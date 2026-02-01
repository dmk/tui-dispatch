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

use crate::bus::{process_raw_event, spawn_event_poller, EventBus, EventRoutingState, RawEvent};
use crate::effect::{DispatchResult, EffectStore, EffectStoreWithMiddleware};
use crate::event::{ComponentId, EventContext, EventKind};
use crate::keybindings::Keybindings;
use crate::store::{Middleware, Reducer, Store, StoreWithMiddleware};
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

/// Result of mapping an event into actions plus an optional render hint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventOutcome<A> {
    /// Actions to enqueue.
    pub actions: Vec<A>,
    /// Whether to force a re-render.
    pub needs_render: bool,
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

impl<A> EventOutcome<A> {
    /// No actions and no render.
    pub fn ignored() -> Self {
        Self {
            actions: Vec::new(),
            needs_render: false,
        }
    }

    /// No actions, but request a render.
    pub fn needs_render() -> Self {
        Self {
            actions: Vec::new(),
            needs_render: true,
        }
    }

    /// Wrap a single action.
    pub fn action(action: A) -> Self {
        Self {
            actions: vec![action],
            needs_render: false,
        }
    }

    /// Wrap multiple actions.
    pub fn actions<I>(actions: I) -> Self
    where
        I: IntoIterator<Item = A>,
    {
        Self {
            actions: actions.into_iter().collect(),
            needs_render: false,
        }
    }

    /// Mark that a render is needed.
    pub fn with_render(mut self) -> Self {
        self.needs_render = true;
        self
    }
}

impl<A> Default for EventOutcome<A> {
    fn default() -> Self {
        Self::ignored()
    }
}

impl<A> From<A> for EventOutcome<A> {
    fn from(action: A) -> Self {
        Self::action(action)
    }
}

impl<A> From<Vec<A>> for EventOutcome<A> {
    fn from(actions: Vec<A>) -> Self {
        Self {
            actions,
            needs_render: false,
        }
    }
}

impl<A> From<Option<A>> for EventOutcome<A> {
    fn from(action: Option<A>) -> Self {
        match action {
            Some(action) => Self::action(action),
            None => Self::ignored(),
        }
    }
}

impl<A> EventOutcome<A> {
    /// Create from any iterator of actions
    ///
    /// Useful for converting `Component::handle_event` results which return
    /// `impl IntoIterator<Item = A>`.
    pub fn from_actions(iter: impl IntoIterator<Item = A>) -> Self {
        Self {
            actions: iter.into_iter().collect(),
            needs_render: false,
        }
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

/// Store interface used by `DispatchRuntime`.
pub trait DispatchStore<S, A: Action> {
    /// Dispatch an action and return whether the state changed.
    fn dispatch(&mut self, action: A) -> bool;
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

impl<S, A: Action, M: Middleware<A>> DispatchStore<S, A> for StoreWithMiddleware<S, A, M> {
    fn dispatch(&mut self, action: A) -> bool {
        StoreWithMiddleware::dispatch(self, action)
    }

    fn state(&self) -> &S {
        StoreWithMiddleware::state(self)
    }
}

/// Effect store interface used by `EffectRuntime`.
pub trait EffectStoreLike<S, A: Action, E> {
    /// Dispatch an action and return state changes plus effects.
    fn dispatch(&mut self, action: A) -> DispatchResult<E>;
    /// Get the current state.
    fn state(&self) -> &S;
}

impl<S, A: Action, E> EffectStoreLike<S, A, E> for EffectStore<S, A, E> {
    fn dispatch(&mut self, action: A) -> DispatchResult<E> {
        EffectStore::dispatch(self, action)
    }

    fn state(&self) -> &S {
        EffectStore::state(self)
    }
}

impl<S, A: Action, E, M: Middleware<A>> EffectStoreLike<S, A, E>
    for EffectStoreWithMiddleware<S, A, E, M>
{
    fn dispatch(&mut self, action: A) -> DispatchResult<E> {
        EffectStoreWithMiddleware::dispatch(self, action)
    }

    fn state(&self) -> &S {
        EffectStoreWithMiddleware::state(self)
    }
}

/// Runtime helper for simple stores (no effects).
pub struct DispatchRuntime<S, A: Action, St: DispatchStore<S, A> = Store<S, A>> {
    store: St,
    action_tx: mpsc::UnboundedSender<A>,
    action_rx: mpsc::UnboundedReceiver<A>,
    poller_config: PollerConfig,
    #[cfg(feature = "debug")]
    debug: Option<Box<dyn DebugAdapter<S, A>>>,
    should_render: bool,
    _state: std::marker::PhantomData<S>,
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
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        Self {
            store,
            action_tx,
            action_rx,
            poller_config: PollerConfig::default(),
            #[cfg(feature = "debug")]
            debug: None,
            should_render: true,
            _state: std::marker::PhantomData,
        }
    }

    /// Attach a debug layer.
    #[cfg(feature = "debug")]
    pub fn with_debug<D>(mut self, debug: D) -> Self
    where
        D: DebugAdapter<S, A>,
    {
        self.debug = Some(Box::new(debug));
        self
    }

    /// Configure event polling behavior.
    pub fn with_event_poller(mut self, config: PollerConfig) -> Self {
        self.poller_config = config;
        self
    }

    /// Send an action into the runtime queue.
    pub fn enqueue(&self, action: A) {
        let _ = self.action_tx.send(action);
    }

    /// Clone the action sender.
    pub fn action_tx(&self) -> mpsc::UnboundedSender<A> {
        self.action_tx.clone()
    }

    /// Access the current state.
    pub fn state(&self) -> &S {
        self.store.state()
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
        let (event_tx, mut event_rx) = mpsc::unbounded_channel::<RawEvent>();
        let cancel_token = CancellationToken::new();
        let _handle = spawn_event_poller(
            event_tx,
            self.poller_config.poll_timeout,
            self.poller_config.loop_sleep,
            cancel_token.clone(),
        );

        loop {
            if self.should_render {
                let state = self.store.state();
                let render_ctx = RenderContext {
                    debug_enabled: {
                        #[cfg(feature = "debug")]
                        {
                            self.debug
                                .as_ref()
                                .map(|debug| debug.is_enabled())
                                .unwrap_or(false)
                        }
                        #[cfg(not(feature = "debug"))]
                        {
                            false
                        }
                    },
                };
                terminal.draw(|frame| {
                    #[cfg(feature = "debug")]
                    if let Some(debug) = self.debug.as_mut() {
                        let mut render_fn =
                            |f: &mut Frame, area: Rect, state: &S, ctx: RenderContext| {
                                render(f, area, state, ctx);
                            };
                        debug.render(frame, state, render_ctx, &mut render_fn);
                    } else {
                        render(frame, frame.area(), state, render_ctx);
                    }

                    #[cfg(not(feature = "debug"))]
                    {
                        render(frame, frame.area(), state, render_ctx);
                    }
                })?;
                self.should_render = false;
            }

            tokio::select! {
                Some(raw_event) = event_rx.recv() => {
                    let event = process_raw_event(raw_event);

                    #[cfg(feature = "debug")]
                    if let Some(debug) = self.debug.as_mut() {
                        if let Some(needs_render) =
                            debug.handle_event(&event, self.store.state(), &self.action_tx)
                        {
                            self.should_render = needs_render;
                            continue;
                        }
                    }

                    let outcome: EventOutcome<A> = map_event(&event, self.store.state()).into();
                    if outcome.needs_render {
                        self.should_render = true;
                    }
                    for action in outcome.actions {
                        let _ = self.action_tx.send(action);
                    }
                }

                Some(action) = self.action_rx.recv() => {
                    if should_quit(&action) {
                        break;
                    }

                    #[cfg(feature = "debug")]
                    if let Some(debug) = self.debug.as_mut() {
                        debug.log_action(&action);
                    }

                    self.should_render = self.store.dispatch(action);
                }

                else => {
                    break;
                }
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
        let (event_tx, mut event_rx) = mpsc::unbounded_channel::<RawEvent>();
        let cancel_token = CancellationToken::new();
        let _handle = spawn_event_poller(
            event_tx,
            self.poller_config.poll_timeout,
            self.poller_config.loop_sleep,
            cancel_token.clone(),
        );

        loop {
            if self.should_render {
                let state = self.store.state();
                let render_ctx = RenderContext {
                    debug_enabled: {
                        #[cfg(feature = "debug")]
                        {
                            self.debug
                                .as_ref()
                                .map(|debug| debug.is_enabled())
                                .unwrap_or(false)
                        }
                        #[cfg(not(feature = "debug"))]
                        {
                            false
                        }
                    },
                };
                terminal.draw(|frame| {
                    #[cfg(feature = "debug")]
                    if let Some(debug) = self.debug.as_mut() {
                        let mut render_fn =
                            |f: &mut Frame, area: Rect, state: &S, ctx: RenderContext| {
                                render(f, area, state, ctx, bus.context_mut());
                            };
                        debug.render(frame, state, render_ctx, &mut render_fn);
                    } else {
                        render(frame, frame.area(), state, render_ctx, bus.context_mut());
                    }

                    #[cfg(not(feature = "debug"))]
                    {
                        render(frame, frame.area(), state, render_ctx, bus.context_mut());
                    }
                })?;
                self.should_render = false;
            }

            tokio::select! {
                Some(raw_event) = event_rx.recv() => {
                    let event = process_raw_event(raw_event);

                    #[cfg(feature = "debug")]
                    if let Some(debug) = self.debug.as_mut() {
                        if let Some(needs_render) =
                            debug.handle_event(&event, self.store.state(), &self.action_tx)
                        {
                            self.should_render = needs_render;
                            continue;
                        }
                    }

                    let outcome = bus.handle_event(&event, self.store.state(), keybindings);
                    if outcome.needs_render {
                        self.should_render = true;
                    }
                    for action in outcome.actions {
                        let _ = self.action_tx.send(action);
                    }
                }

                Some(action) = self.action_rx.recv() => {
                    if should_quit(&action) {
                        break;
                    }

                    #[cfg(feature = "debug")]
                    if let Some(debug) = self.debug.as_mut() {
                        debug.log_action(&action);
                    }

                    self.should_render = self.store.dispatch(action);
                }

                else => {
                    break;
                }
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
pub struct EffectRuntime<S, A: Action, E, St: EffectStoreLike<S, A, E> = EffectStore<S, A, E>> {
    store: St,
    action_tx: mpsc::UnboundedSender<A>,
    action_rx: mpsc::UnboundedReceiver<A>,
    poller_config: PollerConfig,
    #[cfg(feature = "debug")]
    debug: Option<Box<dyn DebugAdapter<S, A>>>,
    should_render: bool,
    #[cfg(feature = "tasks")]
    tasks: TaskManager<A>,
    #[cfg(feature = "subscriptions")]
    subscriptions: Subscriptions<A>,
    /// Broadcasts action names when dispatched (for replay await functionality).
    action_broadcast: tokio::sync::broadcast::Sender<String>,
    _state: std::marker::PhantomData<S>,
    _effect: std::marker::PhantomData<E>,
}

impl<S: 'static, A: Action, E> EffectRuntime<S, A, E, EffectStore<S, A, E>> {
    /// Create a runtime from state + effect reducer.
    pub fn new(state: S, reducer: crate::effect::EffectReducer<S, A, E>) -> Self {
        Self::from_store(EffectStore::new(state, reducer))
    }
}

impl<S: 'static, A: Action, E, St: EffectStoreLike<S, A, E>> EffectRuntime<S, A, E, St> {
    /// Create a runtime from an existing effect store.
    pub fn from_store(store: St) -> Self {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        let (action_broadcast, _) = tokio::sync::broadcast::channel(64);

        #[cfg(feature = "tasks")]
        let tasks = TaskManager::new(action_tx.clone());
        #[cfg(feature = "subscriptions")]
        let subscriptions = Subscriptions::new(action_tx.clone());

        Self {
            store,
            action_tx,
            action_rx,
            poller_config: PollerConfig::default(),
            #[cfg(feature = "debug")]
            debug: None,
            should_render: true,
            #[cfg(feature = "tasks")]
            tasks,
            #[cfg(feature = "subscriptions")]
            subscriptions,
            action_broadcast,
            _state: std::marker::PhantomData,
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
        self.debug = Some(Box::new(debug));
        self
    }

    /// Configure event polling behavior.
    pub fn with_event_poller(mut self, config: PollerConfig) -> Self {
        self.poller_config = config;
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
        let _ = self.action_tx.send(action);
    }

    /// Clone the action sender.
    pub fn action_tx(&self) -> mpsc::UnboundedSender<A> {
        self.action_tx.clone()
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
            action_tx: &self.action_tx,
            tasks: &mut self.tasks,
            subscriptions: &mut self.subscriptions,
        }
    }

    #[cfg(all(feature = "tasks", not(feature = "subscriptions")))]
    fn effect_context(&mut self) -> EffectContext<'_, A> {
        EffectContext {
            action_tx: &self.action_tx,
            tasks: &mut self.tasks,
        }
    }

    #[cfg(all(not(feature = "tasks"), feature = "subscriptions"))]
    fn effect_context(&mut self) -> EffectContext<'_, A> {
        EffectContext {
            action_tx: &self.action_tx,
            subscriptions: &mut self.subscriptions,
        }
    }

    #[cfg(all(not(feature = "tasks"), not(feature = "subscriptions")))]
    fn effect_context(&mut self) -> EffectContext<'_, A> {
        EffectContext {
            action_tx: &self.action_tx,
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
        let (event_tx, mut event_rx) = mpsc::unbounded_channel::<RawEvent>();
        let cancel_token = CancellationToken::new();
        let _handle = spawn_event_poller(
            event_tx,
            self.poller_config.poll_timeout,
            self.poller_config.loop_sleep,
            cancel_token.clone(),
        );

        loop {
            if self.should_render {
                let state = self.store.state();
                let render_ctx = RenderContext {
                    debug_enabled: {
                        #[cfg(feature = "debug")]
                        {
                            self.debug
                                .as_ref()
                                .map(|debug| debug.is_enabled())
                                .unwrap_or(false)
                        }
                        #[cfg(not(feature = "debug"))]
                        {
                            false
                        }
                    },
                };
                terminal.draw(|frame| {
                    #[cfg(feature = "debug")]
                    if let Some(debug) = self.debug.as_mut() {
                        let mut render_fn =
                            |f: &mut Frame, area: Rect, state: &S, ctx: RenderContext| {
                                render(f, area, state, ctx);
                            };
                        debug.render(frame, state, render_ctx, &mut render_fn);
                    } else {
                        render(frame, frame.area(), state, render_ctx);
                    }

                    #[cfg(not(feature = "debug"))]
                    {
                        render(frame, frame.area(), state, render_ctx);
                    }
                })?;
                self.should_render = false;
            }

            tokio::select! {
                Some(raw_event) = event_rx.recv() => {
                    let event = process_raw_event(raw_event);

                    #[cfg(feature = "debug")]
                    if let Some(debug) = self.debug.as_mut() {
                        if let Some(needs_render) =
                            debug.handle_event(&event, self.store.state(), &self.action_tx)
                        {
                            self.should_render = needs_render;
                            continue;
                        }
                    }

                    let outcome: EventOutcome<A> = map_event(&event, self.store.state()).into();
                    if outcome.needs_render {
                        self.should_render = true;
                    }
                    for action in outcome.actions {
                        let _ = self.action_tx.send(action);
                    }
                }

                Some(action) = self.action_rx.recv() => {
                    if should_quit(&action) {
                        break;
                    }

                    #[cfg(feature = "debug")]
                    if let Some(debug) = self.debug.as_mut() {
                        debug.log_action(&action);
                    }

                    // Broadcast action name for replay await functionality
                    if self.action_broadcast.receiver_count() > 0 {
                        let _ = self.action_broadcast.send(action.name().to_string());
                    }

                    let result = self.store.dispatch(action);
                    if result.has_effects() {
                        let mut ctx = self.effect_context();
                        for effect in result.effects {
                            handle_effect(effect, &mut ctx);
                        }
                    }
                    self.should_render = result.changed;
                }

                else => {
                    break;
                }
            }
        }

        cancel_token.cancel();
        #[cfg(feature = "subscriptions")]
        self.subscriptions.cancel_all();
        #[cfg(feature = "tasks")]
        self.tasks.cancel_all();

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
        let (event_tx, mut event_rx) = mpsc::unbounded_channel::<RawEvent>();
        let cancel_token = CancellationToken::new();
        let _handle = spawn_event_poller(
            event_tx,
            self.poller_config.poll_timeout,
            self.poller_config.loop_sleep,
            cancel_token.clone(),
        );

        loop {
            if self.should_render {
                let state = self.store.state();
                let render_ctx = RenderContext {
                    debug_enabled: {
                        #[cfg(feature = "debug")]
                        {
                            self.debug
                                .as_ref()
                                .map(|debug| debug.is_enabled())
                                .unwrap_or(false)
                        }
                        #[cfg(not(feature = "debug"))]
                        {
                            false
                        }
                    },
                };
                terminal.draw(|frame| {
                    #[cfg(feature = "debug")]
                    if let Some(debug) = self.debug.as_mut() {
                        let mut render_fn =
                            |f: &mut Frame, area: Rect, state: &S, ctx: RenderContext| {
                                render(f, area, state, ctx, bus.context_mut());
                            };
                        debug.render(frame, state, render_ctx, &mut render_fn);
                    } else {
                        render(frame, frame.area(), state, render_ctx, bus.context_mut());
                    }

                    #[cfg(not(feature = "debug"))]
                    {
                        render(frame, frame.area(), state, render_ctx, bus.context_mut());
                    }
                })?;
                self.should_render = false;
            }

            tokio::select! {
                Some(raw_event) = event_rx.recv() => {
                    let event = process_raw_event(raw_event);

                    #[cfg(feature = "debug")]
                    if let Some(debug) = self.debug.as_mut() {
                        if let Some(needs_render) =
                            debug.handle_event(&event, self.store.state(), &self.action_tx)
                        {
                            self.should_render = needs_render;
                            continue;
                        }
                    }

                    let outcome = bus.handle_event(&event, self.store.state(), keybindings);
                    if outcome.needs_render {
                        self.should_render = true;
                    }
                    for action in outcome.actions {
                        let _ = self.action_tx.send(action);
                    }
                }

                Some(action) = self.action_rx.recv() => {
                    if should_quit(&action) {
                        break;
                    }

                    #[cfg(feature = "debug")]
                    if let Some(debug) = self.debug.as_mut() {
                        debug.log_action(&action);
                    }

                    // Broadcast action name for replay await functionality
                    if self.action_broadcast.receiver_count() > 0 {
                        let _ = self.action_broadcast.send(action.name().to_string());
                    }

                    let result = self.store.dispatch(action);
                    if result.has_effects() {
                        let mut ctx = self.effect_context();
                        for effect in result.effects {
                            handle_effect(effect, &mut ctx);
                        }
                    }
                    self.should_render = result.changed;
                }

                else => {
                    break;
                }
            }
        }

        cancel_token.cancel();
        #[cfg(feature = "subscriptions")]
        self.subscriptions.cancel_all();
        #[cfg(feature = "tasks")]
        self.tasks.cancel_all();

        Ok(())
    }
}
