//! Event bus for dispatching events to subscribed components

use crate::event::{ComponentId, Event, EventContext, EventKind, EventType};
use crate::Action;
use crossterm::event::{self, KeyModifiers, MouseEventKind};
use std::collections::{HashMap, HashSet};
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

/// Event bus that manages subscriptions and dispatches events
///
/// Generic over:
/// - `A`: The action type (must implement `Action`)
/// - `C`: The component ID type (must implement `ComponentId`)
pub struct EventBus<A: Action, C: ComponentId> {
    /// Subscriptions: event type -> set of component IDs
    subscriptions: HashMap<EventType, HashSet<C>>,
    /// Current event context (focus, areas, etc.)
    context: EventContext<C>,
    /// Channel for sending actions
    action_tx: mpsc::UnboundedSender<A>,
}

impl<A: Action, C: ComponentId> EventBus<A, C> {
    /// Create a new event bus
    pub fn new(action_tx: mpsc::UnboundedSender<A>) -> Self {
        Self {
            subscriptions: HashMap::new(),
            context: EventContext::default(),
            action_tx,
        }
    }

    /// Subscribe a component to an event type
    pub fn subscribe(&mut self, component: C, event_type: EventType) {
        self.subscriptions
            .entry(event_type)
            .or_default()
            .insert(component);
    }

    /// Subscribe a component to multiple event types
    pub fn subscribe_many(&mut self, component: C, event_types: &[EventType]) {
        for &event_type in event_types {
            self.subscribe(component, event_type);
        }
    }

    /// Unsubscribe a component from an event type
    pub fn unsubscribe(&mut self, component: C, event_type: EventType) {
        if let Some(subscribers) = self.subscriptions.get_mut(&event_type) {
            subscribers.remove(&component);
        }
    }

    /// Unsubscribe a component from all event types
    pub fn unsubscribe_all(&mut self, component: C) {
        for subscribers in self.subscriptions.values_mut() {
            subscribers.remove(&component);
        }
    }

    /// Get subscribers for an event type
    pub fn get_subscribers(&self, event_type: EventType) -> Vec<C> {
        self.subscriptions
            .get(&event_type)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Get all subscribers that should receive an event
    pub fn get_event_subscribers(&self, event: &Event<C>) -> Vec<C> {
        let mut subscribers = HashSet::new();

        // If it's a global event, include Global subscribers
        if event.is_global() {
            if let Some(global_subs) = self.subscriptions.get(&EventType::Global) {
                subscribers.extend(global_subs.iter().copied());
            }
        }

        // Add type-specific subscribers
        if let Some(type_subs) = self.subscriptions.get(&event.event_type()) {
            subscribers.extend(type_subs.iter().copied());
        }

        subscribers.into_iter().collect()
    }

    /// Get mutable reference to context
    pub fn context_mut(&mut self) -> &mut EventContext<C> {
        &mut self.context
    }

    /// Get reference to context
    pub fn context(&self) -> &EventContext<C> {
        &self.context
    }

    /// Create an event with current context
    pub fn create_event(&self, kind: EventKind) -> Event<C> {
        Event::new(kind, self.context.clone())
    }

    /// Get the action sender
    pub fn action_tx(&self) -> &mpsc::UnboundedSender<A> {
        &self.action_tx
    }

    /// Send an action through the bus
    pub fn send(&self, action: A) -> Result<(), mpsc::error::SendError<A>> {
        self.action_tx.send(action)
    }

    /// Update context from mouse position
    pub fn update_mouse_position(&mut self, x: u16, y: u16) {
        self.context.mouse_position = Some((x, y));
    }

    /// Update modifiers from key event
    pub fn update_modifiers(&mut self, modifiers: KeyModifiers) {
        self.context.modifiers = modifiers;
    }
}

/// Spawn the event polling task with cancellation support
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

/// Process a raw event into an EventKind
pub fn process_raw_event(raw: RawEvent) -> EventKind {
    match raw {
        RawEvent::Key(key) => EventKind::Key(key),
        RawEvent::Mouse(mouse) => match mouse.kind {
            MouseEventKind::ScrollDown => EventKind::Scroll {
                column: mouse.column,
                row: mouse.row,
                delta: 1,
            },
            MouseEventKind::ScrollUp => EventKind::Scroll {
                column: mouse.column,
                row: mouse.row,
                delta: -1,
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

    #[derive(Clone, Debug)]
    enum TestAction {
        Test,
    }

    impl Action for TestAction {
        fn name(&self) -> &'static str {
            "Test"
        }
    }

    #[test]
    fn test_subscribe_unsubscribe() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut bus: EventBus<TestAction, NumericComponentId> = EventBus::new(tx);

        let component = NumericComponentId(1);
        bus.subscribe(component, EventType::Key);

        assert_eq!(bus.get_subscribers(EventType::Key), vec![component]);

        bus.unsubscribe(component, EventType::Key);
        assert!(bus.get_subscribers(EventType::Key).is_empty());
    }

    #[test]
    fn test_subscribe_many() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut bus: EventBus<TestAction, NumericComponentId> = EventBus::new(tx);

        let component = NumericComponentId(1);
        bus.subscribe_many(component, &[EventType::Key, EventType::Mouse]);

        assert_eq!(bus.get_subscribers(EventType::Key), vec![component]);
        assert_eq!(bus.get_subscribers(EventType::Mouse), vec![component]);
    }

    #[test]
    fn test_unsubscribe_all() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut bus: EventBus<TestAction, NumericComponentId> = EventBus::new(tx);

        let component = NumericComponentId(1);
        bus.subscribe_many(component, &[EventType::Key, EventType::Mouse, EventType::Scroll]);

        bus.unsubscribe_all(component);

        assert!(bus.get_subscribers(EventType::Key).is_empty());
        assert!(bus.get_subscribers(EventType::Mouse).is_empty());
        assert!(bus.get_subscribers(EventType::Scroll).is_empty());
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
            EventKind::Scroll { column, row, delta } => {
                assert_eq!(column, 10);
                assert_eq!(row, 20);
                assert_eq!(delta, 1);
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
