//! Runtime helpers for tui-dispatch apps.
//!
//! These helpers wrap the common event/action/render loop while keeping
//! the same behavior as the manual wiring shown in the examples.

use std::io;
use std::time::Duration;

use ratatui::backend::Backend;
use ratatui::layout::Rect;
use ratatui::{Frame, Terminal};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::bus::{
    process_raw_event, spawn_event_poller, EventBus, EventOutcome, EventRoutingState, RawEvent,
};
use crate::effect::{EffectStore, EffectStoreWithMiddleware, ReducerResult};
use crate::event::{ComponentId, EventContext, EventKind};
use crate::keybindings::Keybindings;
use crate::store::{DispatchError, Middleware, Reducer, Store, StoreWithMiddleware};
use crate::{Action, BindingContext};

#[cfg(feature = "subscriptions")]
use crate::subscriptions::Subscriptions;
#[cfg(feature = "tasks")]
use crate::tasks::TaskManager;

/// Configuration for the event poller.
#[derive(Debug, Clone, Copy)]
pub struct PollerConfig {
    /// Timeout passed to each `crossterm::event::poll` call.
    pub poll_timeout: Duration,
    /// Sleep between poll cycles.
    pub loop_sleep: Duration,
}

impl Default for PollerConfig {
    fn default() -> Self {
        Self {
            poll_timeout: Duration::from_millis(10),
            loop_sleep: Duration::from_millis(16),
        }
    }
}

/// Context passed to render closures.
#[derive(Debug, Clone, Copy, Default)]
pub struct RenderContext {
    /// Whether the debug overlay is currently active.
    pub debug_enabled: bool,
}

impl RenderContext {
    /// Whether the app should treat input focus as active.
    pub fn is_focused(self) -> bool {
        !self.debug_enabled
    }
}

/// Policy applied by runtimes when `try_dispatch` returns a [`DispatchError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchErrorPolicy {
    /// Keep running without forcing a render.
    Continue,
    /// Keep running and force a render pass.
    ///
    /// The runtime does not persist or log the error automatically;
    /// capture visibility in your error handler closure if needed.
    Render,
    /// Stop the runtime loop gracefully.
    Stop,
}

fn apply_dispatch_error_policy(
    handler: &mut dyn FnMut(&DispatchError) -> DispatchErrorPolicy,
    error: DispatchError,
    should_render: &mut bool,
) -> bool {
    match handler(&error) {
        DispatchErrorPolicy::Continue => false,
        DispatchErrorPolicy::Render => {
            *should_render = true;
            false
        }
        DispatchErrorPolicy::Stop => true,
    }
}

#[cfg(feature = "debug")]
pub trait DebugAdapter<S, A>: 'static {
    fn render(
        &mut self,
        frame: &mut Frame,
        state: &S,
        render_ctx: RenderContext,
        render_fn: &mut dyn FnMut(&mut Frame, Rect, &S, RenderContext),
    );

    fn handle_event(
        &mut self,
        event: &EventKind,
        state: &S,
        action_tx: &mpsc::UnboundedSender<A>,
    ) -> Option<bool>;

    fn log_action(&mut self, action: &A);
    fn is_enabled(&self) -> bool;
}

#[cfg(feature = "debug")]
pub trait DebugHooks<A>: Sized {
    #[cfg(feature = "tasks")]
    fn with_task_manager(self, _tasks: &TaskManager<A>) -> Self {
        self
    }

    #[cfg(feature = "subscriptions")]
    fn with_subscriptions(self, _subscriptions: &Subscriptions<A>) -> Self {
        self
    }
}

// ---------------------------------------------------------------------------
// Shared draw macro — renders a frame with optional debug overlay.
//
// Reaches through `$self.shell` for render context / debug / should_render
// and `$self.store.state()` for state. `$render_call` must be callable as
// `$render_call(frame, area, state, ctx)`. For bus variants, callers wrap
// the user's render closure to include EventContext.
// ---------------------------------------------------------------------------
macro_rules! draw_frame {
    ($self:expr, $terminal:expr, $render_call:expr) => {{
        let render_ctx = $self.shell.render_ctx();
        let state = $self.store.state();
        $terminal.draw(|frame| {
            #[cfg(feature = "debug")]
            if let Some(debug) = $self.shell.debug.as_mut() {
                let mut rf = |f: &mut Frame, area: Rect, s: &S, ctx: RenderContext| {
                    ($render_call)(f, area, s, ctx)
                };
                debug.render(frame, state, render_ctx, &mut rf);
            } else {
                ($render_call)(frame, frame.area(), state, render_ctx);
            }
            #[cfg(not(feature = "debug"))]
            {
                ($render_call)(frame, frame.area(), state, render_ctx);
            }
        })?;
        $self.shell.should_render = false;
    }};
}

// ---------------------------------------------------------------------------
// RuntimeShell — fields and helpers shared by DispatchRuntime and EffectRuntime.
// ---------------------------------------------------------------------------

/// Internal state shared by `DispatchRuntime` and `EffectRuntime`.
///
/// Owns everything that isn't specific to sync-vs-effect dispatch: the
/// action channel, poller config, optional debug hook, dispatch error
/// policy, and the render flag.
pub(crate) struct RuntimeShell<S, A: Action> {
    pub(crate) action_tx: mpsc::UnboundedSender<A>,
    pub(crate) action_rx: mpsc::UnboundedReceiver<A>,
    pub(crate) poller_config: PollerConfig,
    #[cfg(feature = "debug")]
    pub(crate) debug: Option<Box<dyn DebugAdapter<S, A>>>,
    pub(crate) dispatch_error_handler: Box<dyn FnMut(&DispatchError) -> DispatchErrorPolicy>,
    pub(crate) should_render: bool,
    _state: std::marker::PhantomData<S>,
}

impl<S: 'static, A: Action> RuntimeShell<S, A> {
    fn new() -> Self {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        Self {
            action_tx,
            action_rx,
            poller_config: PollerConfig::default(),
            #[cfg(feature = "debug")]
            debug: None,
            dispatch_error_handler: Box::new(|_| DispatchErrorPolicy::Stop),
            should_render: true,
            _state: std::marker::PhantomData,
        }
    }

    fn enqueue(&self, action: A) {
        let _ = self.action_tx.send(action);
    }

    fn action_tx_clone(&self) -> mpsc::UnboundedSender<A> {
        self.action_tx.clone()
    }

    fn render_ctx(&self) -> RenderContext {
        RenderContext {
            debug_enabled: {
                #[cfg(feature = "debug")]
                {
                    self.debug.as_ref().is_some_and(|d| d.is_enabled())
                }
                #[cfg(not(feature = "debug"))]
                {
                    false
                }
            },
        }
    }

    /// Returns `Some(needs_render)` if the debug layer intercepted the event.
    #[allow(unused_variables)]
    fn debug_intercept_event(&mut self, event: &EventKind, state: &S) -> Option<bool> {
        #[cfg(feature = "debug")]
        if let Some(debug) = self.debug.as_mut() {
            return debug.handle_event(event, state, &self.action_tx);
        }
        None
    }

    #[allow(unused_variables)]
    fn debug_log_action(&mut self, action: &A) {
        #[cfg(feature = "debug")]
        if let Some(debug) = self.debug.as_mut() {
            debug.log_action(action);
        }
    }

    fn enqueue_outcome(&mut self, outcome: EventOutcome<A>) {
        if outcome.needs_render {
            self.should_render = true;
        }
        for action in outcome.actions {
            let _ = self.action_tx.send(action);
        }
    }

    fn spawn_poller(&self) -> (mpsc::UnboundedReceiver<RawEvent>, CancellationToken) {
        let (event_tx, event_rx) = mpsc::unbounded_channel::<RawEvent>();
        let cancel_token = CancellationToken::new();
        let _handle = spawn_event_poller(
            event_tx,
            self.poller_config.poll_timeout,
            self.poller_config.loop_sleep,
            cancel_token.clone(),
        );
        (event_rx, cancel_token)
    }

    /// Apply the configured dispatch error policy. Returns `true` when the
    /// loop should stop.
    fn apply_error_policy(&mut self, error: DispatchError) -> bool {
        apply_dispatch_error_policy(
            self.dispatch_error_handler.as_mut(),
            error,
            &mut self.should_render,
        )
    }

    /// Process a single raw event: run the debug intercept first, then the
    /// caller-supplied event→outcome mapping. Updates `should_render` and
    /// forwards any emitted actions through the action channel.
    fn process_event<FMap, R>(&mut self, raw_event: RawEvent, state: &S, map: FMap)
    where
        FMap: FnOnce(&EventKind, &S) -> R,
        R: Into<EventOutcome<A>>,
    {
        let event = process_raw_event(raw_event);
        if let Some(needs_render) = self.debug_intercept_event(&event, state) {
            self.should_render = needs_render;
            return;
        }
        self.enqueue_outcome(map(&event, state).into());
    }
}

/// Store interface used by `DispatchRuntime`.
pub trait DispatchStore<S, A: Action> {
    /// Dispatch an action and return whether the state changed.
    fn dispatch(&mut self, action: A) -> bool;
    /// Dispatch an action and return whether the state changed.
    ///
    /// Default behavior wraps [`Self::dispatch`] in `Ok(...)`.
    fn try_dispatch(&mut self, action: A) -> Result<bool, DispatchError> {
        Ok(self.dispatch(action))
    }
    /// Get the current state.
    fn state(&self) -> &S;
}

impl<S, A: Action> DispatchStore<S, A> for Store<S, A> {
    fn dispatch(&mut self, action: A) -> bool {
        Store::dispatch(self, action)
    }

    fn state(&self) -> &S {
        Store::state(self)
    }
}

impl<S, A: Action, M: Middleware<S, A>> DispatchStore<S, A> for StoreWithMiddleware<S, A, M> {
    fn dispatch(&mut self, action: A) -> bool {
        StoreWithMiddleware::dispatch(self, action)
    }

    fn try_dispatch(&mut self, action: A) -> Result<bool, DispatchError> {
        StoreWithMiddleware::try_dispatch(self, action)
    }

    fn state(&self) -> &S {
        StoreWithMiddleware::state(self)
    }
}

/// Effect store interface used by `EffectRuntime`.
pub trait EffectDispatchStore<S, A: Action, E> {
    /// Dispatch an action and return state changes plus effects.
    fn dispatch(&mut self, action: A) -> ReducerResult<E>;
    /// Dispatch an action and return state changes plus effects.
    ///
    /// Default behavior wraps [`Self::dispatch`] in `Ok(...)`.
    fn try_dispatch(&mut self, action: A) -> Result<ReducerResult<E>, DispatchError> {
        Ok(self.dispatch(action))
    }
    /// Get the current state.
    fn state(&self) -> &S;
}

impl<S, A: Action, E> EffectDispatchStore<S, A, E> for EffectStore<S, A, E> {
    fn dispatch(&mut self, action: A) -> ReducerResult<E> {
        EffectStore::dispatch(self, action)
    }

    fn state(&self) -> &S {
        EffectStore::state(self)
    }
}

impl<S, A: Action, E, M: Middleware<S, A>> EffectDispatchStore<S, A, E>
    for EffectStoreWithMiddleware<S, A, E, M>
{
    fn dispatch(&mut self, action: A) -> ReducerResult<E> {
        EffectStoreWithMiddleware::dispatch(self, action)
    }

    fn try_dispatch(&mut self, action: A) -> Result<ReducerResult<E>, DispatchError> {
        EffectStoreWithMiddleware::try_dispatch(self, action)
    }

    fn state(&self) -> &S {
        EffectStoreWithMiddleware::state(self)
    }
}

/// Runtime helper for simple stores (no effects).
pub struct DispatchRuntime<S, A: Action, St: DispatchStore<S, A> = Store<S, A>> {
    store: St,
    shell: RuntimeShell<S, A>,
}

impl<S: 'static, A: Action> DispatchRuntime<S, A, Store<S, A>> {
    /// Create a runtime from state + reducer.
    pub fn new(state: S, reducer: Reducer<S, A>) -> Self {
        Self::from_store(Store::new(state, reducer))
    }
}

impl<S: 'static, A: Action, St: DispatchStore<S, A>> DispatchRuntime<S, A, St> {
    /// Create a runtime from an existing store.
    pub fn from_store(store: St) -> Self {
        Self {
            store,
            shell: RuntimeShell::new(),
        }
    }

    /// Attach a debug layer.
    #[cfg(feature = "debug")]
    pub fn with_debug<D>(mut self, debug: D) -> Self
    where
        D: DebugAdapter<S, A>,
    {
        self.shell.debug = Some(Box::new(debug));
        self
    }

    /// Configure event polling behavior.
    pub fn with_event_poller(mut self, config: PollerConfig) -> Self {
        self.shell.poller_config = config;
        self
    }

    /// Configure handling for recoverable dispatch errors.
    ///
    /// The handler receives each [`DispatchError`] and selects a
    /// [`DispatchErrorPolicy`]. Runtimes do not log or store errors by default;
    /// do that inside this closure when needed.
    pub fn with_dispatch_error_handler<F>(mut self, handler: F) -> Self
    where
        F: FnMut(&DispatchError) -> DispatchErrorPolicy + 'static,
    {
        self.shell.dispatch_error_handler = Box::new(handler);
        self
    }

    /// Send an action into the runtime queue.
    pub fn enqueue(&self, action: A) {
        self.shell.enqueue(action);
    }

    /// Clone the action sender.
    pub fn action_tx(&self) -> mpsc::UnboundedSender<A> {
        self.shell.action_tx_clone()
    }

    /// Access the current state.
    pub fn state(&self) -> &S {
        self.store.state()
    }

    fn dispatch_action(&mut self, action: A) -> bool {
        match self.store.try_dispatch(action) {
            Ok(changed) => {
                self.shell.should_render = changed;
                false
            }
            Err(error) => self.shell.apply_error_policy(error),
        }
    }

    /// Run the event/action loop until quit.
    pub async fn run<B, FRender, FEvent, FQuit, R>(
        &mut self,
        terminal: &mut Terminal<B>,
        mut render: FRender,
        mut map_event: FEvent,
        mut should_quit: FQuit,
    ) -> io::Result<()>
    where
        B: Backend,
        FRender: FnMut(&mut Frame, Rect, &S, RenderContext),
        FEvent: FnMut(&EventKind, &S) -> R,
        R: Into<EventOutcome<A>>,
        FQuit: FnMut(&A) -> bool,
    {
        let (mut event_rx, cancel_token) = self.shell.spawn_poller();

        loop {
            if self.shell.should_render {
                draw_frame!(self, terminal, render);
            }

            tokio::select! {
                Some(raw_event) = event_rx.recv() => {
                    self.shell.process_event(raw_event, self.store.state(), &mut map_event);
                }

                Some(action) = self.shell.action_rx.recv() => {
                    if should_quit(&action) {
                        break;
                    }
                    self.shell.debug_log_action(&action);
                    if self.dispatch_action(action) {
                        break;
                    }
                }

                else => { break; }
            }
        }

        cancel_token.cancel();
        Ok(())
    }

    /// Run the event/action loop using an EventBus for routing.
    pub async fn run_with_bus<B, FRender, FQuit, Id, Ctx>(
        &mut self,
        terminal: &mut Terminal<B>,
        bus: &mut EventBus<S, A, Id, Ctx>,
        keybindings: &Keybindings<Ctx>,
        mut render: FRender,
        mut should_quit: FQuit,
    ) -> io::Result<()>
    where
        B: Backend,
        Id: ComponentId + 'static,
        Ctx: BindingContext + 'static,
        S: EventRoutingState<Id, Ctx>,
        FRender: FnMut(&mut Frame, Rect, &S, RenderContext, &mut EventContext<Id>),
        FQuit: FnMut(&A) -> bool,
    {
        let (mut event_rx, cancel_token) = self.shell.spawn_poller();

        loop {
            if self.shell.should_render {
                draw_frame!(self, terminal, |f, area, s, ctx| render(
                    f,
                    area,
                    s,
                    ctx,
                    bus.context_mut()
                ));
            }

            tokio::select! {
                Some(raw_event) = event_rx.recv() => {
                    self.shell.process_event(raw_event, self.store.state(), |event, state| {
                        bus.handle_event(event, state, keybindings)
                    });
                }

                Some(action) = self.shell.action_rx.recv() => {
                    if should_quit(&action) {
                        break;
                    }
                    self.shell.debug_log_action(&action);
                    if self.dispatch_action(action) {
                        break;
                    }
                }

                else => { break; }
            }
        }

        cancel_token.cancel();
        Ok(())
    }
}

/// Context passed to effect handlers.
pub struct EffectContext<'a, A: Action> {
    action_tx: &'a mpsc::UnboundedSender<A>,
    #[cfg(feature = "tasks")]
    tasks: &'a mut TaskManager<A>,
    #[cfg(feature = "subscriptions")]
    subscriptions: &'a mut Subscriptions<A>,
}

impl<'a, A: Action> EffectContext<'a, A> {
    /// Send an action directly.
    pub fn emit(&self, action: A) {
        let _ = self.action_tx.send(action);
    }

    /// Access the action sender.
    pub fn action_tx(&self) -> &mpsc::UnboundedSender<A> {
        self.action_tx
    }

    /// Access the task manager.
    #[cfg(feature = "tasks")]
    pub fn tasks(&mut self) -> &mut TaskManager<A> {
        self.tasks
    }

    /// Access subscriptions.
    #[cfg(feature = "subscriptions")]
    pub fn subscriptions(&mut self) -> &mut Subscriptions<A> {
        self.subscriptions
    }
}

/// Runtime helper for effect-based stores.
pub struct EffectRuntime<S, A: Action, E, St: EffectDispatchStore<S, A, E> = EffectStore<S, A, E>> {
    store: St,
    shell: RuntimeShell<S, A>,
    #[cfg(feature = "tasks")]
    tasks: TaskManager<A>,
    #[cfg(feature = "subscriptions")]
    subscriptions: Subscriptions<A>,
    /// Broadcasts action names when dispatched (for replay await functionality).
    action_broadcast: tokio::sync::broadcast::Sender<String>,
    _effect: std::marker::PhantomData<E>,
}

impl<S: 'static, A: Action, E> EffectRuntime<S, A, E, EffectStore<S, A, E>> {
    /// Create a runtime from state + effect reducer.
    pub fn new(state: S, reducer: crate::effect::EffectReducer<S, A, E>) -> Self {
        Self::from_store(EffectStore::new(state, reducer))
    }
}

impl<S: 'static, A: Action, E, St: EffectDispatchStore<S, A, E>> EffectRuntime<S, A, E, St> {
    /// Create a runtime from an existing effect store.
    pub fn from_store(store: St) -> Self {
        let shell = RuntimeShell::new();
        let (action_broadcast, _) = tokio::sync::broadcast::channel(64);

        #[cfg(feature = "tasks")]
        let tasks = TaskManager::new(shell.action_tx.clone());
        #[cfg(feature = "subscriptions")]
        let subscriptions = Subscriptions::new(shell.action_tx.clone());

        Self {
            store,
            shell,
            #[cfg(feature = "tasks")]
            tasks,
            #[cfg(feature = "subscriptions")]
            subscriptions,
            action_broadcast,
            _effect: std::marker::PhantomData,
        }
    }

    /// Attach a debug layer (auto-wires tasks/subscriptions when available).
    #[cfg(feature = "debug")]
    pub fn with_debug<D>(mut self, debug: D) -> Self
    where
        D: DebugAdapter<S, A> + DebugHooks<A>,
    {
        let debug = {
            let debug = debug;
            #[cfg(feature = "tasks")]
            let debug = debug.with_task_manager(&self.tasks);
            #[cfg(feature = "subscriptions")]
            let debug = debug.with_subscriptions(&self.subscriptions);
            debug
        };
        self.shell.debug = Some(Box::new(debug));
        self
    }

    /// Configure event polling behavior.
    pub fn with_event_poller(mut self, config: PollerConfig) -> Self {
        self.shell.poller_config = config;
        self
    }

    /// Configure handling for recoverable dispatch errors.
    ///
    /// The handler receives each [`DispatchError`] and selects a
    /// [`DispatchErrorPolicy`]. Runtimes do not log or store errors by default;
    /// do that inside this closure when needed.
    pub fn with_dispatch_error_handler<F>(mut self, handler: F) -> Self
    where
        F: FnMut(&DispatchError) -> DispatchErrorPolicy + 'static,
    {
        self.shell.dispatch_error_handler = Box::new(handler);
        self
    }

    /// Subscribe to action name broadcasts.
    ///
    /// Returns a receiver that will receive action names (from `action.name()`)
    /// whenever an action is dispatched. Useful for replay await functionality.
    pub fn subscribe_actions(&self) -> tokio::sync::broadcast::Receiver<String> {
        self.action_broadcast.subscribe()
    }

    /// Send an action into the runtime queue.
    pub fn enqueue(&self, action: A) {
        self.shell.enqueue(action);
    }

    /// Clone the action sender.
    pub fn action_tx(&self) -> mpsc::UnboundedSender<A> {
        self.shell.action_tx_clone()
    }

    /// Access the current state.
    pub fn state(&self) -> &S {
        self.store.state()
    }

    /// Access the task manager.
    #[cfg(feature = "tasks")]
    pub fn tasks(&mut self) -> &mut TaskManager<A> {
        &mut self.tasks
    }

    /// Access subscriptions.
    #[cfg(feature = "subscriptions")]
    pub fn subscriptions(&mut self) -> &mut Subscriptions<A> {
        &mut self.subscriptions
    }

    #[cfg(all(feature = "tasks", feature = "subscriptions"))]
    fn effect_context(&mut self) -> EffectContext<'_, A> {
        EffectContext {
            action_tx: &self.shell.action_tx,
            tasks: &mut self.tasks,
            subscriptions: &mut self.subscriptions,
        }
    }

    #[cfg(all(feature = "tasks", not(feature = "subscriptions")))]
    fn effect_context(&mut self) -> EffectContext<'_, A> {
        EffectContext {
            action_tx: &self.shell.action_tx,
            tasks: &mut self.tasks,
        }
    }

    #[cfg(all(not(feature = "tasks"), feature = "subscriptions"))]
    fn effect_context(&mut self) -> EffectContext<'_, A> {
        EffectContext {
            action_tx: &self.shell.action_tx,
            subscriptions: &mut self.subscriptions,
        }
    }

    #[cfg(all(not(feature = "tasks"), not(feature = "subscriptions")))]
    fn effect_context(&mut self) -> EffectContext<'_, A> {
        EffectContext {
            action_tx: &self.shell.action_tx,
        }
    }

    fn broadcast_action(&self, action: &A) {
        if self.action_broadcast.receiver_count() > 0 {
            let _ = self.action_broadcast.send(action.name().to_string());
        }
    }

    fn cleanup(&mut self, cancel_token: CancellationToken) {
        cancel_token.cancel();
        #[cfg(feature = "subscriptions")]
        self.subscriptions.cancel_all();
        #[cfg(feature = "tasks")]
        self.tasks.cancel_all();
    }

    fn dispatch_and_handle_effects(
        &mut self,
        action: A,
        handle_effect: &mut impl FnMut(E, &mut EffectContext<A>),
    ) -> bool {
        self.broadcast_action(&action);
        match self.store.try_dispatch(action) {
            Ok(result) => {
                if result.has_effects() {
                    let mut ctx = self.effect_context();
                    for effect in result.effects {
                        handle_effect(effect, &mut ctx);
                    }
                }
                self.shell.should_render = result.changed;
                false
            }
            Err(error) => self.shell.apply_error_policy(error),
        }
    }

    /// Run the event/action loop until quit.
    pub async fn run<B, FRender, FEvent, FQuit, FEffect, R>(
        &mut self,
        terminal: &mut Terminal<B>,
        mut render: FRender,
        mut map_event: FEvent,
        mut should_quit: FQuit,
        mut handle_effect: FEffect,
    ) -> io::Result<()>
    where
        B: Backend,
        FRender: FnMut(&mut Frame, Rect, &S, RenderContext),
        FEvent: FnMut(&EventKind, &S) -> R,
        R: Into<EventOutcome<A>>,
        FQuit: FnMut(&A) -> bool,
        FEffect: FnMut(E, &mut EffectContext<A>),
    {
        let (mut event_rx, cancel_token) = self.shell.spawn_poller();

        loop {
            if self.shell.should_render {
                draw_frame!(self, terminal, render);
            }

            tokio::select! {
                Some(raw_event) = event_rx.recv() => {
                    self.shell.process_event(raw_event, self.store.state(), &mut map_event);
                }

                Some(action) = self.shell.action_rx.recv() => {
                    if should_quit(&action) {
                        break;
                    }
                    self.shell.debug_log_action(&action);
                    if self.dispatch_and_handle_effects(action, &mut handle_effect) {
                        break;
                    }
                }

                else => { break; }
            }
        }

        self.cleanup(cancel_token);
        Ok(())
    }

    /// Run the event/action loop using an EventBus for routing.
    pub async fn run_with_bus<B, FRender, FQuit, FEffect, Id, Ctx>(
        &mut self,
        terminal: &mut Terminal<B>,
        bus: &mut EventBus<S, A, Id, Ctx>,
        keybindings: &Keybindings<Ctx>,
        mut render: FRender,
        mut should_quit: FQuit,
        mut handle_effect: FEffect,
    ) -> io::Result<()>
    where
        B: Backend,
        Id: ComponentId + 'static,
        Ctx: BindingContext + 'static,
        S: EventRoutingState<Id, Ctx>,
        FRender: FnMut(&mut Frame, Rect, &S, RenderContext, &mut EventContext<Id>),
        FQuit: FnMut(&A) -> bool,
        FEffect: FnMut(E, &mut EffectContext<A>),
    {
        let (mut event_rx, cancel_token) = self.shell.spawn_poller();

        loop {
            if self.shell.should_render {
                draw_frame!(self, terminal, |f, area, s, ctx| render(
                    f,
                    area,
                    s,
                    ctx,
                    bus.context_mut()
                ));
            }

            tokio::select! {
                Some(raw_event) = event_rx.recv() => {
                    self.shell.process_event(raw_event, self.store.state(), |event, state| {
                        bus.handle_event(event, state, keybindings)
                    });
                }

                Some(action) = self.shell.action_rx.recv() => {
                    if should_quit(&action) {
                        break;
                    }
                    self.shell.debug_log_action(&action);
                    if self.dispatch_and_handle_effects(action, &mut handle_effect) {
                        break;
                    }
                }

                else => { break; }
            }
        }

        self.cleanup(cancel_token);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::DispatchLimits;
    use std::collections::VecDeque;

    #[derive(Clone, Debug)]
    enum TestAction {
        Increment,
    }

    impl Action for TestAction {
        fn name(&self) -> &'static str {
            match self {
                TestAction::Increment => "Increment",
            }
        }
    }

    #[derive(Default)]
    struct TestState {
        count: usize,
    }

    fn reducer(state: &mut TestState, _action: TestAction) -> bool {
        state.count += 1;
        true
    }

    fn effect_reducer(state: &mut TestState, _action: TestAction) -> ReducerResult<()> {
        state.count += 1;
        ReducerResult::changed()
    }

    struct LoopMiddleware;

    impl Middleware<TestState, TestAction> for LoopMiddleware {
        fn before(&mut self, _action: &TestAction, _state: &TestState) -> bool {
            true
        }

        fn after(
            &mut self,
            _action: &TestAction,
            _state_changed: bool,
            _state: &TestState,
        ) -> Vec<TestAction> {
            vec![TestAction::Increment]
        }
    }

    struct MockDispatchStore {
        state: TestState,
        queued_results: VecDeque<Result<bool, DispatchError>>,
    }

    impl MockDispatchStore {
        fn from_results(results: impl IntoIterator<Item = Result<bool, DispatchError>>) -> Self {
            Self {
                state: TestState::default(),
                queued_results: results.into_iter().collect(),
            }
        }
    }

    impl DispatchStore<TestState, TestAction> for MockDispatchStore {
        fn dispatch(&mut self, _action: TestAction) -> bool {
            true
        }

        fn try_dispatch(&mut self, _action: TestAction) -> Result<bool, DispatchError> {
            self.queued_results
                .pop_front()
                .expect("test configured with at least one dispatch result")
        }

        fn state(&self) -> &TestState {
            &self.state
        }
    }

    struct MockEffectStore {
        state: TestState,
        queued_results: VecDeque<Result<ReducerResult<()>, DispatchError>>,
    }

    impl MockEffectStore {
        fn from_results(
            results: impl IntoIterator<Item = Result<ReducerResult<()>, DispatchError>>,
        ) -> Self {
            Self {
                state: TestState::default(),
                queued_results: results.into_iter().collect(),
            }
        }
    }

    impl EffectDispatchStore<TestState, TestAction, ()> for MockEffectStore {
        fn dispatch(&mut self, _action: TestAction) -> ReducerResult<()> {
            ReducerResult::changed()
        }

        fn try_dispatch(
            &mut self,
            _action: TestAction,
        ) -> Result<ReducerResult<()>, DispatchError> {
            self.queued_results
                .pop_front()
                .expect("test configured with at least one dispatch result")
        }

        fn state(&self) -> &TestState {
            &self.state
        }
    }

    fn test_error() -> DispatchError {
        DispatchError::DepthExceeded {
            max_depth: 1,
            action: "Increment",
        }
    }

    #[test]
    fn dispatch_runtime_continue_policy_keeps_running_without_render() {
        let store = MockDispatchStore::from_results([Err(test_error())]);
        let mut runtime = DispatchRuntime::from_store(store)
            .with_dispatch_error_handler(|_| DispatchErrorPolicy::Continue);
        runtime.shell.should_render = false;

        let should_stop = runtime.dispatch_action(TestAction::Increment);

        assert!(!should_stop);
        assert!(!runtime.shell.should_render);
    }

    #[test]
    fn dispatch_runtime_render_policy_forces_render() {
        let store = MockDispatchStore::from_results([Err(test_error())]);
        let mut runtime = DispatchRuntime::from_store(store)
            .with_dispatch_error_handler(|_| DispatchErrorPolicy::Render);
        runtime.shell.should_render = false;

        let should_stop = runtime.dispatch_action(TestAction::Increment);

        assert!(!should_stop);
        assert!(runtime.shell.should_render);
    }

    #[test]
    fn dispatch_runtime_stop_policy_breaks_loop() {
        let store = MockDispatchStore::from_results([Err(test_error())]);
        let mut runtime = DispatchRuntime::from_store(store)
            .with_dispatch_error_handler(|_| DispatchErrorPolicy::Stop);
        runtime.shell.should_render = false;

        let should_stop = runtime.dispatch_action(TestAction::Increment);

        assert!(should_stop);
        assert!(!runtime.shell.should_render);
    }

    #[test]
    fn effect_runtime_continue_policy_keeps_running_without_render() {
        let store = MockEffectStore::from_results([Err(test_error())]);
        let mut runtime = EffectRuntime::from_store(store)
            .with_dispatch_error_handler(|_| DispatchErrorPolicy::Continue);
        runtime.shell.should_render = false;

        let should_stop =
            runtime.dispatch_and_handle_effects(TestAction::Increment, &mut |_effect, _ctx| {});

        assert!(!should_stop);
        assert!(!runtime.shell.should_render);
    }

    #[test]
    fn effect_runtime_render_policy_forces_render() {
        let store = MockEffectStore::from_results([Err(test_error())]);
        let mut runtime = EffectRuntime::from_store(store)
            .with_dispatch_error_handler(|_| DispatchErrorPolicy::Render);
        runtime.shell.should_render = false;

        let should_stop =
            runtime.dispatch_and_handle_effects(TestAction::Increment, &mut |_effect, _ctx| {});

        assert!(!should_stop);
        assert!(runtime.shell.should_render);
    }

    #[test]
    fn effect_runtime_stop_policy_breaks_loop() {
        let store = MockEffectStore::from_results([Err(test_error())]);
        let mut runtime = EffectRuntime::from_store(store)
            .with_dispatch_error_handler(|_| DispatchErrorPolicy::Stop);
        runtime.shell.should_render = false;

        let should_stop =
            runtime.dispatch_and_handle_effects(TestAction::Increment, &mut |_effect, _ctx| {});

        assert!(should_stop);
        assert!(!runtime.shell.should_render);
    }

    #[test]
    fn dispatch_runtime_uses_try_dispatch_for_middleware_overflow() {
        let store = StoreWithMiddleware::new(TestState::default(), reducer, LoopMiddleware)
            .with_dispatch_limits(DispatchLimits {
                max_depth: 1,
                max_actions: 100,
            });
        let mut runtime = DispatchRuntime::from_store(store)
            .with_dispatch_error_handler(|_| DispatchErrorPolicy::Stop);
        runtime.shell.should_render = false;

        let should_stop = runtime.dispatch_action(TestAction::Increment);

        assert!(should_stop);
        assert_eq!(runtime.state().count, 1);
    }

    #[test]
    fn effect_runtime_uses_try_dispatch_for_middleware_overflow() {
        let store =
            EffectStoreWithMiddleware::new(TestState::default(), effect_reducer, LoopMiddleware)
                .with_dispatch_limits(DispatchLimits {
                    max_depth: 1,
                    max_actions: 100,
                });
        let mut runtime = EffectRuntime::from_store(store)
            .with_dispatch_error_handler(|_| DispatchErrorPolicy::Stop);
        runtime.shell.should_render = false;

        let should_stop =
            runtime.dispatch_and_handle_effects(TestAction::Increment, &mut |_effect, _ctx| {});

        assert!(should_stop);
        assert_eq!(runtime.state().count, 1);
    }
}
