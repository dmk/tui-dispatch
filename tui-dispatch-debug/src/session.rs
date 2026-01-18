use crate::cli::DebugCliArgs;
use crate::debug::{ActionLoggerConfig, DebugLayer, DebugState};
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

    pub fn render_wait(&self) -> u64 {
        self.args.render_wait
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
            StateSnapshot::load_ron(path)
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
            StateSnapshot::load_ron(path)
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

    pub fn load_actions<A>(&self) -> DebugSessionResult<Vec<A>>
    where
        A: DeserializeOwned,
    {
        if let Some(path) = self.args.actions_in.as_ref() {
            ActionSnapshot::load_ron(path)
                .map(|snapshot| snapshot.into_actions())
                .map_err(DebugSessionError::Snapshot)
        } else {
            Ok(Vec::new())
        }
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
            .save_ron(path)
            .map_err(DebugSessionError::Snapshot)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn run_effect_app<B, S, A, E, St, FInit, FRender, FEvent, FQuit, FEffect, R>(
        &self,
        terminal: &mut Terminal<B>,
        mut store: St,
        debug_layer: DebugLayer<A>,
        replay_actions: Vec<A>,
        auto_action: Option<A>,
        render_wait_quit_action: Option<A>,
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

        if self.args.render_once {
            let final_state = if self.args.render_wait > 0 {
                let mut runtime = EffectRuntime::from_store(store);
                init_runtime(&mut runtime);

                for action in replay_actions {
                    runtime.enqueue(action);
                }
                if self.auto_fetch() {
                    if let Some(action) = auto_action.clone() {
                        runtime.enqueue(action);
                    }
                }

                let Some(quit_action) = render_wait_quit_action else {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "debug render wait requires a quit action",
                    ));
                };
                let wait = self.args.render_wait;
                let action_tx = runtime.action_tx();
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(wait)).await;
                    let _ = action_tx.send(quit_action);
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
                for action in replay_actions {
                    let _ = store.dispatch(action);
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

        let debug_layer = debug_layer
            .with_state_snapshots::<S>()
            .active(self.args.enabled);
        let mut runtime = EffectRuntime::from_store(store).with_debug(debug_layer);
        init_runtime(&mut runtime);

        for action in replay_actions {
            runtime.enqueue(action);
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
