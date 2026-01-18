use crate::cli::DebugCliArgs;
use crate::debug::{glob_match, ActionLoggerConfig, DebugLayer, DebugState};
use crate::replay::ReplayItem;
use crate::snapshot::{ActionSnapshot, SnapshotError, StateSnapshot};
use ratatui::backend::{Backend, TestBackend};
use ratatui::layout::{Rect, Size};
use ratatui::Terminal;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::cell::RefCell;
use std::error::Error;
use std::fmt;
use std::future::Future;
use std::io;
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;
use tui_dispatch_core::runtime::{
    EffectContext, EffectRuntime, EffectStoreLike, EventOutcome, RenderContext,
};
use tui_dispatch_core::store::{ComposedMiddleware, Middleware};
use tui_dispatch_core::testing::RenderHarness;
use tui_dispatch_core::{Action, ActionParams, EventKind};

/// Records actions for --debug-actions-out snapshots with optional filtering.
#[derive(Clone)]
pub struct DebugActionRecorder<A> {
    actions: Rc<RefCell<Vec<A>>>,
    filter: ActionLoggerConfig,
}

impl<A> DebugActionRecorder<A> {
    pub fn new(filter: ActionLoggerConfig) -> Self {
        Self {
            actions: Rc::new(RefCell::new(Vec::new())),
            filter,
        }
    }

    pub fn actions(&self) -> Vec<A>
    where
        A: Clone,
    {
        self.actions.borrow().clone()
    }
}

impl<A: Action> Middleware<A> for DebugActionRecorder<A> {
    fn before(&mut self, action: &A) {
        if self.filter.should_log(action.name()) {
            self.actions.borrow_mut().push(action.clone());
        }
    }

    fn after(&mut self, _action: &A, _state_changed: bool) {}
}

/// Output from a debug-aware app run.
pub struct DebugRunOutput<S> {
    state: S,
    render_output: Option<String>,
}

impl<S> DebugRunOutput<S> {
    pub fn new(state: S, render_output: Option<String>) -> Self {
        Self {
            state,
            render_output,
        }
    }

    pub fn state(&self) -> &S {
        &self.state
    }

    pub fn into_state(self) -> S {
        self.state
    }

    pub fn render_output(&self) -> Option<&str> {
        self.render_output.as_deref()
    }

    pub fn take_render_output(self) -> Option<String> {
        self.render_output
    }

    pub fn write_render_output(&self) -> io::Result<()> {
        if let Some(output) = self.render_output.as_ref() {
            let mut stdout = io::stdout();
            stdout.write_all(output.as_bytes())?;
            stdout.flush()?;
        }
        Ok(())
    }
}

pub type DebugSessionResult<T> = Result<T, DebugSessionError>;

#[derive(Debug)]
pub enum DebugSessionError {
    Snapshot(SnapshotError),
    Fallback(Box<dyn Error + Send + Sync>),
    MissingActionRecorder { path: PathBuf },
}

impl DebugSessionError {
    fn fallback<E>(error: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self::Fallback(Box::new(error))
    }
}

impl fmt::Display for DebugSessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Snapshot(error) => write!(f, "snapshot error: {error:?}"),
            Self::Fallback(error) => write!(f, "fallback error: {error}"),
            Self::MissingActionRecorder { path } => write!(
                f,
                "debug actions out requested but no recorder attached: {}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for DebugSessionError {}

/// Error from replay with await markers.
#[derive(Debug)]
pub enum ReplayError {
    /// Timeout waiting for action to be dispatched.
    Timeout { pattern: String },
    /// Broadcast channel closed unexpectedly.
    ChannelClosed,
}

impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timeout { pattern } => {
                write!(f, "timeout waiting for action matching '{pattern}'")
            }
            Self::ChannelClosed => write!(f, "action broadcast channel closed"),
        }
    }
}

impl std::error::Error for ReplayError {}

/// Wait for an action matching one of the patterns to be dispatched.
async fn wait_for_action(
    action_rx: &mut tokio::sync::broadcast::Receiver<String>,
    patterns: &[String],
    timeout: Duration,
) -> Result<(), ReplayError> {
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Err(ReplayError::Timeout {
                pattern: patterns.join(" | "),
            });
        }

        match tokio::time::timeout(remaining, action_rx.recv()).await {
            Ok(Ok(action_name)) => {
                // Check if any pattern matches
                for pattern in patterns {
                    if glob_match(pattern, &action_name) {
                        return Ok(());
                    }
                }
                // Not a match, keep waiting
            }
            Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                return Err(ReplayError::ChannelClosed);
            }
            Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {
                // Missed some messages, but keep waiting
                continue;
            }
            Err(_) => {
                // Timeout
                return Err(ReplayError::Timeout {
                    pattern: patterns.join(" | "),
                });
            }
        }
    }
}

/// Helper for wiring debug CLI flags into an app runtime.
#[derive(Debug)]
pub struct DebugSession {
    args: DebugCliArgs,
}

impl DebugSession {
    pub fn new(args: DebugCliArgs) -> Self {
        Self { args }
    }

    pub fn args(&self) -> &DebugCliArgs {
        &self.args
    }

    pub fn enabled(&self) -> bool {
        self.args.enabled
    }

    pub fn render_once(&self) -> bool {
        self.args.render_once
    }

    pub fn use_alt_screen(&self) -> bool {
        !self.args.render_once
    }

    pub fn action_filter(&self) -> ActionLoggerConfig {
        self.args.action_filter()
    }

    pub fn auto_fetch(&self) -> bool {
        self.args.auto_fetch()
    }

    pub fn load_state_or_else<S, F, E>(&self, fallback: F) -> DebugSessionResult<S>
    where
        S: DeserializeOwned,
        F: FnOnce() -> Result<S, E>,
        E: Error + Send + Sync + 'static,
    {
        if let Some(path) = self.args.state_in.as_ref() {
            StateSnapshot::load_json(path)
                .map(|snapshot| snapshot.into_state())
                .map_err(DebugSessionError::Snapshot)
        } else {
            fallback().map_err(DebugSessionError::fallback)
        }
    }

    pub async fn load_state_or_else_async<S, F, Fut, E>(&self, fallback: F) -> DebugSessionResult<S>
    where
        S: DeserializeOwned,
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<S, E>>,
        E: Error + Send + Sync + 'static,
    {
        if let Some(path) = self.args.state_in.as_ref() {
            StateSnapshot::load_json(path)
                .map(|snapshot| snapshot.into_state())
                .map_err(DebugSessionError::Snapshot)
        } else {
            fallback().await.map_err(DebugSessionError::fallback)
        }
    }

    pub fn load_state_or<S, F>(&self, fallback: F) -> DebugSessionResult<S>
    where
        S: DeserializeOwned,
        F: FnOnce() -> S,
    {
        self.load_state_or_else(|| Ok::<S, std::convert::Infallible>(fallback()))
    }

    /// Load replay items from `--debug-actions-in`.
    ///
    /// This auto-detects the format: either a simple `Vec<A>` or `Vec<ReplayItem<A>>`.
    /// Both formats are supported for backwards compatibility.
    pub fn load_replay_items<A>(&self) -> DebugSessionResult<Vec<ReplayItem<A>>>
    where
        A: DeserializeOwned,
    {
        let Some(path) = self.args.actions_in.as_ref() else {
            return Ok(Vec::new());
        };

        let contents =
            std::fs::read_to_string(path).map_err(|e| DebugSessionError::Snapshot(e.into()))?;

        // Try parsing as Vec<ReplayItem<A>> first (supports await markers)
        if let Ok(items) = serde_json::from_str::<Vec<ReplayItem<A>>>(&contents) {
            return Ok(items);
        }

        // Fall back to Vec<A> (simple action list, wrap in ReplayItem::Action)
        let actions: Vec<A> = serde_json::from_str(&contents)
            .map_err(|e| DebugSessionError::Snapshot(SnapshotError::Json(e)))?;
        Ok(actions.into_iter().map(ReplayItem::Action).collect())
    }

    /// Load actions from `--debug-actions-in` (legacy API, ignores await markers).
    #[deprecated(note = "Use load_replay_items instead")]
    pub fn load_actions<A>(&self) -> DebugSessionResult<Vec<A>>
    where
        A: DeserializeOwned,
    {
        self.load_replay_items().map(|items| {
            items
                .into_iter()
                .filter_map(|item| item.into_action())
                .collect()
        })
    }

    pub fn action_recorder<A: Action>(&self) -> Option<DebugActionRecorder<A>> {
        self.args
            .actions_out
            .as_ref()
            .map(|_| DebugActionRecorder::new(self.action_filter()))
    }

    pub fn middleware_with_recorder<A: Action>(
        &self,
    ) -> (ComposedMiddleware<A>, Option<DebugActionRecorder<A>>) {
        let mut middleware = ComposedMiddleware::new();
        let recorder = self.action_recorder();
        if let Some(recorder) = recorder.clone() {
            middleware.add(recorder);
        }
        (middleware, recorder)
    }

    pub fn save_actions<A>(
        &self,
        recorder: Option<&DebugActionRecorder<A>>,
    ) -> DebugSessionResult<()>
    where
        A: Clone + Serialize,
    {
        let Some(path) = self.args.actions_out.as_ref() else {
            return Ok(());
        };
        let Some(recorder) = recorder else {
            return Err(DebugSessionError::MissingActionRecorder {
                path: path.to_path_buf(),
            });
        };
        ActionSnapshot::new(recorder.actions())
            .save_json(path)
            .map_err(DebugSessionError::Snapshot)
    }

    /// Save JSON schema for the state type if `--debug-state-schema-out` was set.
    #[cfg(feature = "json-schema")]
    pub fn save_state_schema<S>(&self) -> DebugSessionResult<()>
    where
        S: crate::JsonSchema,
    {
        if let Some(path) = self.args.state_schema_out.as_ref() {
            crate::save_schema::<S, _>(path).map_err(DebugSessionError::Snapshot)
        } else {
            Ok(())
        }
    }

    /// Save JSON schema for replay items (actions + await markers).
    ///
    /// This generates a schema for `Vec<ReplayItem<A>>` which includes:
    /// - All action variants from `A`
    /// - `_await` and `_await_any` markers for async coordination
    /// - An `$defs.awaitable_actions` list of Did* action names
    #[cfg(feature = "json-schema")]
    pub fn save_actions_schema<A>(&self) -> DebugSessionResult<()>
    where
        A: crate::JsonSchema,
    {
        if let Some(path) = self.args.actions_schema_out.as_ref() {
            crate::save_replay_schema::<A, _>(path).map_err(DebugSessionError::Snapshot)
        } else {
            Ok(())
        }
    }

    /// Get the replay timeout duration from CLI args.
    pub fn replay_timeout(&self) -> Duration {
        Duration::from_secs(self.args.replay_timeout)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn run_effect_app<B, S, A, E, St, FInit, FRender, FEvent, FQuit, FEffect, R>(
        &self,
        terminal: &mut Terminal<B>,
        mut store: St,
        debug_layer: DebugLayer<A>,
        replay_items: Vec<ReplayItem<A>>,
        auto_action: Option<A>,
        quit_action: Option<A>,
        init_runtime: FInit,
        mut render: FRender,
        mut map_event: FEvent,
        mut should_quit: FQuit,
        mut handle_effect: FEffect,
    ) -> io::Result<DebugRunOutput<S>>
    where
        B: Backend,
        S: Clone + DebugState + Serialize + 'static,
        A: Action + ActionParams,
        St: EffectStoreLike<S, A, E>,
        FInit: FnOnce(&mut EffectRuntime<S, A, E, St>),
        FRender: FnMut(&mut ratatui::Frame, Rect, &S, RenderContext),
        FEvent: FnMut(&EventKind, &S) -> R,
        R: Into<EventOutcome<A>>,
        FQuit: FnMut(&A) -> bool,
        FEffect: FnMut(E, &mut EffectContext<A>),
    {
        let size = terminal.size().unwrap_or_else(|_| Size::new(80, 24));
        let width = size.width.max(1);
        let height = size.height.max(1);
        let auto_action = auto_action;

        // Check if replay items contain any await markers
        let has_awaits = replay_items.iter().any(|item| item.is_await());
        let replay_timeout = self.replay_timeout();

        if self.args.render_once {
            let final_state = if has_awaits {
                // Need runtime for effects when replay has await markers
                let runtime = EffectRuntime::from_store(store);
                let mut action_rx = runtime.subscribe_actions();
                let action_tx = runtime.action_tx();

                // Spawn replay task that processes items with await support
                let replay_items_clone = replay_items;
                let auto_action_clone = auto_action.clone();
                let auto_fetch = self.auto_fetch();
                let replay_handle = tokio::spawn(async move {
                    for item in replay_items_clone {
                        match item {
                            ReplayItem::Action(action) => {
                                let _ = action_tx.send(action);
                            }
                            ReplayItem::AwaitOne { _await: pattern } => {
                                wait_for_action(&mut action_rx, &[pattern], replay_timeout).await?;
                            }
                            ReplayItem::AwaitAny {
                                _await_any: patterns,
                            } => {
                                wait_for_action(&mut action_rx, &patterns, replay_timeout).await?;
                            }
                        }
                    }
                    if auto_fetch {
                        if let Some(action) = auto_action_clone {
                            let _ = action_tx.send(action);
                        }
                    }
                    Ok::<(), ReplayError>(())
                });

                let mut runtime = runtime;
                init_runtime(&mut runtime);

                let quit_action = quit_action.ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "replay with await markers requires a quit action",
                    )
                })?;

                // Send quit after replay completes
                let action_tx = runtime.action_tx();
                let quit = quit_action.clone();
                tokio::spawn(async move {
                    // Wait for replay to complete
                    let _ = replay_handle.await;
                    // Small delay to let final effects settle
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    let _ = action_tx.send(quit);
                });

                let backend = TestBackend::new(width, height);
                let mut test_terminal = Terminal::new(backend)?;
                runtime
                    .run(
                        &mut test_terminal,
                        |_frame, _area, _state, _ctx| {},
                        |_event, _state| EventOutcome::<A>::ignored(),
                        |action| should_quit(action),
                        |effect, ctx| handle_effect(effect, ctx),
                    )
                    .await?;

                runtime.state().clone()
            } else {
                // Simple dispatch mode (no effects, ignores awaits)
                for item in replay_items {
                    if let ReplayItem::Action(action) = item {
                        let _ = store.dispatch(action);
                    }
                }
                if self.auto_fetch() {
                    if let Some(action) = auto_action.clone() {
                        let _ = store.dispatch(action);
                    }
                }
                store.state().clone()
            };

            let mut harness = RenderHarness::new(width, height);
            let output = harness.render_to_string_plain(|frame| {
                render(frame, frame.area(), &final_state, RenderContext::default());
            });

            return Ok(DebugRunOutput::new(final_state, Some(output)));
        }

        // Normal interactive mode - just enqueue actions (ignore awaits)
        let debug_layer = debug_layer
            .with_state_snapshots::<S>()
            .active(self.args.enabled);
        let mut runtime = EffectRuntime::from_store(store).with_debug(debug_layer);
        init_runtime(&mut runtime);

        for item in replay_items {
            if let ReplayItem::Action(action) = item {
                runtime.enqueue(action);
            }
        }
        if self.auto_fetch() {
            if let Some(action) = auto_action {
                runtime.enqueue(action);
            }
        }

        let result = runtime
            .run(
                terminal,
                |frame, area, state, render_ctx| {
                    render(frame, area, state, render_ctx);
                },
                |event, state| map_event(event, state),
                |action| should_quit(action),
                |effect, ctx| handle_effect(effect, ctx),
            )
            .await;

        match result {
            Ok(()) => Ok(DebugRunOutput::new(runtime.state().clone(), None)),
            Err(err) => Err(err),
        }
    }
}
