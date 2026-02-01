//! Event bus for dispatching events to registered handlers

use crate::event::{ComponentId, EventContext, EventKind, EventType};
use crate::keybindings::Keybindings;
use crate::runtime::EventOutcome;
use crate::{Action, BindingContext};
use crossterm::event::{self, KeyEventKind, MouseEventKind};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

/// Raw event from crossterm before processing
#[derive(Debug)]
pub enum RawEvent {
    Key(crossterm::event::KeyEvent),
    Mouse(crossterm::event::MouseEvent),
    Resize(u16, u16),
}

/// State accessors used by the event bus for routing decisions.
pub trait EventRoutingState<Id: ComponentId, Ctx: BindingContext> {
    fn focused(&self) -> Option<Id>;
    fn modal(&self) -> Option<Id>;
    fn binding_context(&self, id: Id) -> Ctx;
    fn default_context(&self) -> Ctx;
}

/// Where a handler is being routed in the current event flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteTarget<Id: ComponentId> {
    Modal(Id),
    Focused(Id),
    Hovered(Id),
    Subscriber(Id),
    Global,
}

/// Routing plan that determines the order components receive events.
///
/// Priority: Modal → Hovered → Focused → Subscribers → Global subscribers
struct RoutingPlan<Id: ComponentId> {
    targets: Vec<(RouteTarget<Id>, Id)>,
    modal_blocks: bool,
}

impl<Id: ComponentId> RoutingPlan<Id> {
    /// Build a routing plan for an event.
    fn build<S, Ctx>(
        event: &EventKind,
        state: &S,
        subscribers: &[Id],
        global_subscribers: &[Id],
        hovered: Option<Id>,
    ) -> Self
    where
        S: EventRoutingState<Id, Ctx>,
        Ctx: BindingContext,
    {
        let is_broadcast = event.is_broadcast();
        let modal = state.modal();
        let focused = state.focused();

        let mut targets = Vec::with_capacity(
            subscribers.len()
                + if event.is_global() {
                    global_subscribers.len()
                } else {
                    0
                }
                + if modal.is_some() { 1 } else { 0 }
                + if hovered.is_some() { 1 } else { 0 }
                + if focused.is_some() { 1 } else { 0 },
        );

        // Modal gets first priority
        if let Some(id) = modal {
            targets.push((RouteTarget::Modal(id), id));
        }

        // Modal blocks non-broadcast events from reaching other components
        let modal_blocks = modal.is_some() && !is_broadcast;

        if !modal_blocks {
            // Hovered component (mouse events only)
            if let Some(id) = hovered {
                targets.push((RouteTarget::Hovered(id), id));
            }

            // Focused component
            if let Some(id) = focused {
                targets.push((RouteTarget::Focused(id), id));
            }

            // Type-specific subscribers
            for &id in subscribers {
                targets.push((RouteTarget::Subscriber(id), id));
            }
        }

        // Global subscribers (for global events, even when modal is active)
        if event.is_global() {
            for &id in global_subscribers {
                targets.push((RouteTarget::Subscriber(id), id));
            }
        }

        Self {
            targets,
            modal_blocks,
        }
    }

    fn iter(&self) -> impl Iterator<Item = (RouteTarget<Id>, Id)> + '_ {
        self.targets.iter().copied()
    }
}

/// Routed event passed to handlers.
#[derive(Debug, Clone)]
pub struct RoutedEvent<'a, Id: ComponentId, Ctx: BindingContext> {
    pub kind: EventKind,
    pub command: Option<&'a str>,
    pub binding_ctx: Ctx,
    pub target: RouteTarget<Id>,
    pub context: &'a EventContext<Id>,
}

/// Response returned by an event handler.
#[derive(Debug, Clone)]
pub struct HandlerResponse<A> {
    pub actions: Vec<A>,
    pub consumed: bool,
    pub needs_render: bool,
}

impl<A> HandlerResponse<A> {
    pub fn ignored() -> Self {
        Self {
            actions: Vec::new(),
            consumed: false,
            needs_render: false,
        }
    }

    pub fn action(action: A) -> Self {
        Self {
            actions: vec![action],
            consumed: true,
            needs_render: false,
        }
    }

    /// Multiple actions, event consumed.
    pub fn actions<I>(actions: I) -> Self
    where
        I: IntoIterator<Item = A>,
    {
        Self {
            actions: actions.into_iter().collect(),
            consumed: true,
            needs_render: false,
        }
    }

    /// Multiple actions, event NOT consumed (passthrough to other handlers).
    pub fn actions_passthrough<I>(actions: I) -> Self
    where
        I: IntoIterator<Item = A>,
    {
        Self {
            actions: actions.into_iter().collect(),
            consumed: false,
            needs_render: false,
        }
    }

    pub fn with_render(mut self) -> Self {
        self.needs_render = true;
        self
    }

    pub fn with_consumed(mut self, consumed: bool) -> Self {
        self.consumed = consumed;
        self
    }
}

/// Trait for event handlers registered with the bus.
pub trait EventHandler<S, A, Id: ComponentId, Ctx: BindingContext>: 'static {
    fn handle(&mut self, event: RoutedEvent<'_, Id, Ctx>, state: &S) -> HandlerResponse<A>;
}

impl<S, A, Id, Ctx, F> EventHandler<S, A, Id, Ctx> for F
where
    Id: ComponentId,
    Ctx: BindingContext,
    F: for<'a> FnMut(RoutedEvent<'a, Id, Ctx>, &S) -> HandlerResponse<A> + 'static,
{
    fn handle(&mut self, event: RoutedEvent<'_, Id, Ctx>, state: &S) -> HandlerResponse<A> {
        (self)(event, state)
    }
}

type HandlerFn<S, A, Id, Ctx> =
    dyn for<'a, 'ctx> FnMut(RoutedEvent<'ctx, Id, Ctx>, &'a S) -> HandlerResponse<A>;

/// Event bus that manages subscriptions and dispatches events.
pub struct EventBus<S, A: Action, Id: ComponentId, Ctx: BindingContext> {
    handlers: HashMap<Id, Box<HandlerFn<S, A, Id, Ctx>>>,
    global_handlers: Vec<Box<HandlerFn<S, A, Id, Ctx>>>,
    subscriptions: HashMap<EventType, Vec<Id>>,
    global_subscribers: Vec<Id>,
    registration_order: Vec<Id>,
    context: EventContext<Id>,
}

/// Default binding context for apps that don't need custom contexts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct DefaultBindingContext;

impl BindingContext for DefaultBindingContext {
    fn name(&self) -> &'static str {
        "default"
    }

    fn from_name(name: &str) -> Option<Self> {
        (name == "default").then_some(Self)
    }

    fn all() -> &'static [Self] {
        &[Self]
    }
}

/// Simplified EventBus for apps that don't need custom binding contexts.
///
/// Use this when you don't need context-specific keybindings:
/// ```ignore
/// let bus: SimpleEventBus<AppState, AppAction, ComponentId> = SimpleEventBus::new();
/// ```
pub type SimpleEventBus<S, A, Id> = EventBus<S, A, Id, DefaultBindingContext>;

impl<S: 'static, A, Id, Ctx> Default for EventBus<S, A, Id, Ctx>
where
    A: Action,
    Id: ComponentId + 'static,
    Ctx: BindingContext + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<S: 'static, A, Id, Ctx> EventBus<S, A, Id, Ctx>
where
    A: Action,
    Id: ComponentId + 'static,
    Ctx: BindingContext + 'static,
{
    /// Create a new event bus.
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            global_handlers: Vec::new(),
            subscriptions: HashMap::new(),
            global_subscribers: Vec::new(),
            registration_order: Vec::new(),
            context: EventContext::default(),
        }
    }

    /// Register a handler for a component ID.
    pub fn register<F>(&mut self, component: Id, handler: F)
    where
        F: for<'a, 'ctx> FnMut(RoutedEvent<'ctx, Id, Ctx>, &'a S) -> HandlerResponse<A> + 'static,
    {
        if !self.handlers.contains_key(&component) {
            self.registration_order.push(component);
        }
        self.handlers.insert(component, Box::new(handler));
    }

    /// Register a component handler that implements [`EventHandler`].
    pub fn register_handler<H>(&mut self, component: Id, mut handler: H)
    where
        H: EventHandler<S, A, Id, Ctx> + 'static,
    {
        self.register(component, move |event, state| handler.handle(event, state));
    }

    /// Register a global handler (not tied to a component).
    pub fn register_global<F>(&mut self, handler: F)
    where
        F: for<'a, 'ctx> FnMut(RoutedEvent<'ctx, Id, Ctx>, &'a S) -> HandlerResponse<A> + 'static,
    {
        self.global_handlers.push(Box::new(handler));
    }

    /// Register a global handler that implements [`EventHandler`].
    pub fn register_global_handler<H>(&mut self, mut handler: H)
    where
        H: EventHandler<S, A, Id, Ctx> + 'static,
    {
        self.register_global(move |event, state| handler.handle(event, state));
    }

    /// Unregister a component handler.
    pub fn unregister(&mut self, component: Id) -> Option<Box<HandlerFn<S, A, Id, Ctx>>> {
        self.registration_order.retain(|id| *id != component);
        self.unsubscribe_all(component);
        self.handlers.remove(&component)
    }

    /// Subscribe a component to an event type.
    pub fn subscribe(&mut self, component: Id, event_type: EventType) {
        if event_type == EventType::Global {
            if !self.global_subscribers.contains(&component) {
                self.global_subscribers.push(component);
            }
            return;
        }

        let entry = self.subscriptions.entry(event_type).or_default();
        if !entry.contains(&component) {
            entry.push(component);
        }
    }

    /// Subscribe a component to multiple event types.
    pub fn subscribe_many(&mut self, component: Id, event_types: &[EventType]) {
        for &event_type in event_types {
            self.subscribe(component, event_type);
        }
    }

    /// Unsubscribe a component from an event type.
    pub fn unsubscribe(&mut self, component: Id, event_type: EventType) {
        if event_type == EventType::Global {
            self.global_subscribers.retain(|id| *id != component);
            return;
        }

        if let Some(subscribers) = self.subscriptions.get_mut(&event_type) {
            subscribers.retain(|id| *id != component);
        }
    }

    /// Unsubscribe a component from all event types.
    pub fn unsubscribe_all(&mut self, component: Id) {
        self.global_subscribers.retain(|id| *id != component);
        for subscribers in self.subscriptions.values_mut() {
            subscribers.retain(|id| *id != component);
        }
    }

    /// Get subscribers for an event type.
    pub fn get_subscribers(&self, event_type: EventType) -> Vec<Id> {
        if event_type == EventType::Global {
            return self.global_subscribers.clone();
        }

        self.subscriptions
            .get(&event_type)
            .cloned()
            .unwrap_or_default()
    }

    /// Get mutable reference to context.
    pub fn context_mut(&mut self) -> &mut EventContext<Id> {
        &mut self.context
    }

    /// Get reference to context.
    pub fn context(&self) -> &EventContext<Id> {
        &self.context
    }

    /// Route an event through the bus and collect actions.
    ///
    /// Routing priority: Modal → Hovered → Focused → Subscribers → Global handlers.
    /// Modal blocks non-broadcast events from reaching other components.
    pub fn handle_event(
        &mut self,
        event: &EventKind,
        state: &S,
        keybindings: &Keybindings<Ctx>,
    ) -> EventOutcome<A>
    where
        S: EventRoutingState<Id, Ctx>,
    {
        self.update_context(event);

        let is_broadcast = event.is_broadcast();
        let is_mouse_event = matches!(event, EventKind::Mouse(_) | EventKind::Scroll { .. });
        let key_event = match event {
            EventKind::Key(key)
                if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
            {
                Some(key)
            }
            _ => None,
        };

        // Gather routing inputs
        let subscribers = self
            .subscriptions
            .get(&event.event_type())
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        let global_subscribers = self.global_subscribers.as_slice();
        let hovered = if is_mouse_event {
            self.mouse_position_from_event(event)
                .and_then(|(col, row)| self.hit_test(col, row))
        } else {
            None
        };

        // Build routing plan
        let plan = RoutingPlan::build(event, state, subscribers, global_subscribers, hovered);

        // Prepare binding context for global handlers
        let default_binding_ctx = state.default_context();
        let global_binding_ctx = state
            .modal()
            .or_else(|| state.focused())
            .map(|id| state.binding_context(id))
            .unwrap_or(default_binding_ctx);
        // Dispatch state
        let mut command_cache: HashMap<Ctx, Option<&str>> = HashMap::new();
        let mut actions = Vec::new();
        let mut needs_render = false;
        let mut consumed = false;
        let mut called: Vec<Id> = Vec::with_capacity(plan.targets.len());

        // Route to components
        for (target, id) in plan.iter() {
            if called.contains(&id) {
                continue;
            }
            called.push(id);

            if let Some(handler) = self.handlers.get_mut(&id) {
                let binding_ctx = state.binding_context(id);
                let command = if let Some(key) = key_event {
                    *command_cache
                        .entry(binding_ctx)
                        .or_insert_with(|| keybindings.get_command_ref(key, binding_ctx))
                } else {
                    None
                };
                let response = Self::call_handler(
                    handler.as_mut(),
                    target,
                    event,
                    command,
                    binding_ctx,
                    &self.context,
                    state,
                );

                actions.extend(response.actions);
                needs_render |= response.needs_render;
                consumed |= response.consumed;

                if consumed && !is_broadcast {
                    return EventOutcome {
                        actions,
                        needs_render,
                    };
                }
            }
        }

        // Global handlers (always run last, skip if modal blocks and not global event)
        let should_run_global = !plan.modal_blocks || event.is_global();
        if should_run_global {
            let global_command = if let Some(key) = key_event {
                *command_cache
                    .entry(global_binding_ctx)
                    .or_insert_with(|| keybindings.get_command_ref(key, global_binding_ctx))
            } else {
                None
            };
            for handler in self.global_handlers.iter_mut() {
                let response = Self::call_handler(
                    handler.as_mut(),
                    RouteTarget::Global,
                    event,
                    global_command,
                    global_binding_ctx,
                    &self.context,
                    state,
                );

                actions.extend(response.actions);
                needs_render |= response.needs_render;
                consumed |= response.consumed;

                if consumed && !is_broadcast {
                    break;
                }
            }
        }

        EventOutcome {
            actions,
            needs_render,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn call_handler(
        handler: &mut HandlerFn<S, A, Id, Ctx>,
        target: RouteTarget<Id>,
        event: &EventKind,
        command: Option<&str>,
        binding_ctx: Ctx,
        context: &EventContext<Id>,
        state: &S,
    ) -> HandlerResponse<A> {
        let routed = RoutedEvent {
            kind: event.clone(),
            command,
            binding_ctx,
            target,
            context,
        };
        handler(routed, state)
    }

    fn update_context(&mut self, event: &EventKind) {
        match event {
            EventKind::Key(key) => {
                self.context.modifiers = key.modifiers;
            }
            EventKind::Mouse(mouse) => {
                self.context.mouse_position = Some((mouse.column, mouse.row));
                self.context.modifiers = mouse.modifiers;
            }
            EventKind::Scroll {
                column,
                row,
                modifiers,
                ..
            } => {
                self.context.mouse_position = Some((*column, *row));
                self.context.modifiers = *modifiers;
            }
            EventKind::Resize(width, height) => {
                if let Some((column, row)) = self.context.mouse_position {
                    if column >= *width || row >= *height {
                        self.context.mouse_position = None;
                    }
                }
            }
            EventKind::Tick => {}
        }
    }

    fn mouse_position_from_event(&self, event: &EventKind) -> Option<(u16, u16)> {
        match event {
            EventKind::Mouse(mouse) => Some((mouse.column, mouse.row)),
            EventKind::Scroll { column, row, .. } => Some((*column, *row)),
            _ => None,
        }
    }

    fn hit_test(&self, column: u16, row: u16) -> Option<Id> {
        self.registration_order
            .iter()
            .rev()
            .copied()
            .find(|&id| self.context.point_in_component(id, column, row))
    }
}

/// Spawn the event polling task with cancellation support.
///
/// This spawns an async task that polls for crossterm events and sends them
/// through the provided channel. The task can be cancelled using the token.
///
/// # Arguments
/// * `tx` - Channel to send raw events
/// * `poll_timeout` - Timeout for each poll operation
/// * `loop_sleep` - Sleep duration between poll cycles
/// * `cancel_token` - Token to cancel the polling task
pub fn spawn_event_poller(
    tx: mpsc::UnboundedSender<RawEvent>,
    poll_timeout: Duration,
    loop_sleep: Duration,
    cancel_token: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        const MAX_EVENTS_PER_BATCH: usize = 20;

        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    info!("Event poller cancelled, draining buffer");
                    // Drain any remaining events from crossterm buffer before exiting
                    while event::poll(Duration::ZERO).unwrap_or(false) {
                        let _ = event::read();
                    }
                    break;
                }
                _ = tokio::time::sleep(loop_sleep) => {
                    // Process up to MAX_EVENTS_PER_BATCH events per iteration
                    let mut events_processed = 0;
                    while events_processed < MAX_EVENTS_PER_BATCH
                        && event::poll(poll_timeout).unwrap_or(false)
                    {
                        events_processed += 1;
                        if let Ok(evt) = event::read() {
                            let raw = match evt {
                                event::Event::Key(key) => Some(RawEvent::Key(key)),
                                event::Event::Mouse(mouse) => Some(RawEvent::Mouse(mouse)),
                                event::Event::Resize(w, h) => Some(RawEvent::Resize(w, h)),
                                _ => None,
                            };
                            if let Some(raw) = raw {
                                if tx.send(raw).is_err() {
                                    debug!("Event channel closed, stopping poller");
                                    return;
                                }
                            }
                        }
                    }
                }
            }
        }
    })
}

/// Process a raw event into an EventKind.
pub fn process_raw_event(raw: RawEvent) -> EventKind {
    match raw {
        RawEvent::Key(key) => EventKind::Key(key),
        RawEvent::Mouse(mouse) => match mouse.kind {
            MouseEventKind::ScrollDown => EventKind::Scroll {
                column: mouse.column,
                row: mouse.row,
                delta: 1,
                modifiers: mouse.modifiers,
            },
            MouseEventKind::ScrollUp => EventKind::Scroll {
                column: mouse.column,
                row: mouse.row,
                delta: -1,
                modifiers: mouse.modifiers,
            },
            _ => EventKind::Mouse(mouse),
        },
        RawEvent::Resize(w, h) => EventKind::Resize(w, h),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::NumericComponentId;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum TestAction {
        Log(&'static str),
    }

    impl Action for TestAction {
        fn name(&self) -> &'static str {
            "Log"
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq, Hash)]
    enum TestContext {
        Default,
    }

    impl BindingContext for TestContext {
        fn name(&self) -> &'static str {
            "default"
        }

        fn from_name(name: &str) -> Option<Self> {
            match name {
                "default" => Some(Self::Default),
                _ => None,
            }
        }

        fn all() -> &'static [Self] {
            &[Self::Default]
        }
    }

    #[derive(Default)]
    struct TestState {
        focused: Option<NumericComponentId>,
        modal: Option<NumericComponentId>,
    }

    impl EventRoutingState<NumericComponentId, TestContext> for TestState {
        fn focused(&self) -> Option<NumericComponentId> {
            self.focused
        }

        fn modal(&self) -> Option<NumericComponentId> {
            self.modal
        }

        fn binding_context(&self, _id: NumericComponentId) -> TestContext {
            TestContext::Default
        }

        fn default_context(&self) -> TestContext {
            TestContext::Default
        }
    }

    #[test]
    fn test_subscribe_unsubscribe() {
        let mut bus: EventBus<TestState, TestAction, NumericComponentId, TestContext> =
            EventBus::new();

        let component = NumericComponentId(1);
        bus.subscribe(component, EventType::Key);

        assert_eq!(bus.get_subscribers(EventType::Key), vec![component]);

        bus.unsubscribe(component, EventType::Key);
        assert!(bus.get_subscribers(EventType::Key).is_empty());
    }

    #[test]
    fn test_subscribe_many() {
        let mut bus: EventBus<TestState, TestAction, NumericComponentId, TestContext> =
            EventBus::new();

        let component = NumericComponentId(1);
        bus.subscribe_many(component, &[EventType::Key, EventType::Mouse]);

        assert_eq!(bus.get_subscribers(EventType::Key), vec![component]);
        assert_eq!(bus.get_subscribers(EventType::Mouse), vec![component]);
    }

    #[test]
    fn test_unsubscribe_all() {
        let mut bus: EventBus<TestState, TestAction, NumericComponentId, TestContext> =
            EventBus::new();

        let component = NumericComponentId(1);
        bus.subscribe_many(
            component,
            &[EventType::Key, EventType::Mouse, EventType::Scroll],
        );

        bus.unsubscribe_all(component);

        assert!(bus.get_subscribers(EventType::Key).is_empty());
        assert!(bus.get_subscribers(EventType::Mouse).is_empty());
        assert!(bus.get_subscribers(EventType::Scroll).is_empty());
    }

    #[test]
    fn test_handle_event_routes_modal_only() {
        let mut bus: EventBus<TestState, TestAction, NumericComponentId, TestContext> =
            EventBus::new();

        let focused = NumericComponentId(1);
        let modal = NumericComponentId(2);
        let subscriber = NumericComponentId(3);

        bus.register(focused, |_, _| HandlerResponse {
            actions: vec![TestAction::Log("focused")],
            consumed: false,
            needs_render: false,
        });
        bus.register(modal, |_, _| HandlerResponse {
            actions: vec![TestAction::Log("modal")],
            consumed: false,
            needs_render: false,
        });
        bus.register(subscriber, |_, _| HandlerResponse {
            actions: vec![TestAction::Log("subscriber")],
            consumed: false,
            needs_render: false,
        });
        bus.register_global(|_, _| HandlerResponse {
            actions: vec![TestAction::Log("global")],
            consumed: false,
            needs_render: false,
        });
        bus.subscribe(subscriber, EventType::Key);

        let state = TestState {
            focused: Some(focused),
            modal: Some(modal),
        };
        let keybindings = Keybindings::new();

        let key_event = KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };

        let outcome = bus.handle_event(&EventKind::Key(key_event), &state, &keybindings);
        assert_eq!(outcome.actions, vec![TestAction::Log("modal")]);
    }

    #[test]
    fn test_global_subscribers_only_for_global_events() {
        let mut bus: EventBus<TestState, TestAction, NumericComponentId, TestContext> =
            EventBus::new();

        let global_subscriber = NumericComponentId(1);
        let key_subscriber = NumericComponentId(2);

        bus.register(global_subscriber, |_, _| HandlerResponse {
            actions: vec![TestAction::Log("global")],
            consumed: false,
            needs_render: false,
        });
        bus.register(key_subscriber, |_, _| HandlerResponse {
            actions: vec![TestAction::Log("key")],
            consumed: false,
            needs_render: false,
        });
        bus.subscribe(global_subscriber, EventType::Global);
        bus.subscribe(key_subscriber, EventType::Key);

        let state = TestState::default();
        let keybindings = Keybindings::new();

        let key_event = KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };

        let outcome = bus.handle_event(&EventKind::Key(key_event), &state, &keybindings);
        assert_eq!(outcome.actions, vec![TestAction::Log("key")]);

        let esc_event = KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };

        let outcome = bus.handle_event(&EventKind::Key(esc_event), &state, &keybindings);
        assert_eq!(
            outcome.actions,
            vec![TestAction::Log("key"), TestAction::Log("global")]
        );
    }

    #[test]
    fn test_handle_event_consumes() {
        let mut bus: EventBus<TestState, TestAction, NumericComponentId, TestContext> =
            EventBus::new();

        let focused = NumericComponentId(1);
        let modal = NumericComponentId(2);

        bus.register(focused, |_, _| {
            HandlerResponse::action(TestAction::Log("focused"))
        });
        bus.register(modal, |_, _| {
            HandlerResponse::action(TestAction::Log("modal"))
        });

        let state = TestState {
            focused: Some(focused),
            modal: Some(modal),
        };
        let keybindings = Keybindings::new();

        let key_event = KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };

        let outcome = bus.handle_event(&EventKind::Key(key_event), &state, &keybindings);
        assert_eq!(outcome.actions, vec![TestAction::Log("modal")]);
    }

    #[test]
    fn test_process_raw_event_key() {
        use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

        let key_event = KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };

        let kind = process_raw_event(RawEvent::Key(key_event));
        assert!(matches!(kind, EventKind::Key(_)));
    }

    #[test]
    fn test_process_raw_event_scroll() {
        use crossterm::event::{MouseEvent, MouseEventKind};

        let scroll_down = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 10,
            row: 20,
            modifiers: KeyModifiers::NONE,
        };

        let kind = process_raw_event(RawEvent::Mouse(scroll_down));
        match kind {
            EventKind::Scroll {
                column,
                row,
                delta,
                modifiers,
            } => {
                assert_eq!(column, 10);
                assert_eq!(row, 20);
                assert_eq!(delta, 1);
                assert_eq!(modifiers, KeyModifiers::NONE);
            }
            _ => panic!("Expected Scroll event"),
        }
    }

    #[test]
    fn test_process_raw_event_resize() {
        let kind = process_raw_event(RawEvent::Resize(80, 24));
        assert!(matches!(kind, EventKind::Resize(80, 24)));
    }
}
