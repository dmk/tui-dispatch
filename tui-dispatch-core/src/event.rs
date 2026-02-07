//! Event types for the pub/sub system

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use ratatui::layout::Rect;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;

/// Trait for user-defined component identifiers
///
/// Implement this trait for your own component ID enum, or use `#[derive(ComponentId)]`
/// from `tui-dispatch-macros` to auto-generate the implementation.
///
/// # Example
/// ```ignore
/// #[derive(ComponentId, Clone, Copy, PartialEq, Eq, Hash, Debug)]
/// pub enum MyComponentId {
///     Sidebar,
///     MainContent,
///     StatusBar,
/// }
/// ```
pub trait ComponentId: Clone + Copy + Eq + Hash + Debug {
    /// Get the component name as a string (for debugging/logging)
    fn name(&self) -> &'static str;
}

/// A simple numeric component ID for basic use cases
///
/// Use this if you don't need named components, or use your own enum
/// with `#[derive(ComponentId)]` for named components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NumericComponentId(pub u32);

impl ComponentId for NumericComponentId {
    fn name(&self) -> &'static str {
        "component"
    }
}

/// Event types that components can subscribe to
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventType {
    /// Keyboard events
    Key,
    /// Mouse click/drag events
    Mouse,
    /// Scroll wheel events
    Scroll,
    /// Terminal resize events
    Resize,
    /// Periodic tick for animations
    Tick,
    /// Global events delivered to all components
    Global,
}

/// The actual event payload
#[derive(Debug, Clone)]
pub enum EventKind {
    /// Keyboard event
    Key(KeyEvent),
    /// Mouse event
    Mouse(MouseEvent),
    /// Scroll event with position, delta, and modifiers
    Scroll {
        column: u16,
        row: u16,
        delta: isize,
        modifiers: KeyModifiers,
    },
    /// Terminal resize
    Resize(u16, u16),
    /// Periodic tick
    Tick,
}

impl EventKind {
    /// Get the event type for this event kind
    pub fn event_type(&self) -> EventType {
        match self {
            EventKind::Key(_) => EventType::Key,
            EventKind::Mouse(_) => EventType::Mouse,
            EventKind::Scroll { .. } => EventType::Scroll,
            EventKind::Resize(_, _) => EventType::Resize,
            EventKind::Tick => EventType::Tick,
        }
    }

    /// Check if this is a global event (should be delivered to all components)
    pub fn is_global(&self) -> bool {
        match self {
            EventKind::Key(key) => {
                use crossterm::event::KeyCode;
                matches!(key.code, KeyCode::Esc)
                    || (key.modifiers.contains(KeyModifiers::CONTROL)
                        && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('q')))
            }
            EventKind::Resize(_, _) => true,
            _ => false,
        }
    }

    /// Check if this is a broadcast event (delivered to all subscribers, never consumed)
    pub fn is_broadcast(&self) -> bool {
        matches!(self, EventKind::Resize(..) | EventKind::Tick)
    }
}

/// Policy for determining which events are treated as global.
///
/// Global events bypass modal blocking and are delivered to global subscribers
/// even when a modal is active. Resize events are always treated as global
/// regardless of this policy.
///
/// By default, Esc, Ctrl+C, and Ctrl+Q are global keys. Use this to customize
/// that behavior — for example, if your app uses Esc for "close modal" and
/// doesn't want it treated as global.
///
/// # Example
///
/// ```ignore
/// use tui_dispatch::{EventBus, GlobalKeyPolicy};
///
/// // Remove Esc from global keys (keep only Ctrl+C, Ctrl+Q)
/// let bus = EventBus::new()
///     .with_global_key_policy(GlobalKeyPolicy::without_esc());
///
/// // Custom set of global keys
/// let bus = EventBus::new()
///     .with_global_key_policy(GlobalKeyPolicy::keys(vec![
///         (KeyCode::Char('c'), KeyModifiers::CONTROL),
///     ]));
///
/// // No key events are global (only Resize remains global)
/// let bus = EventBus::new()
///     .with_global_key_policy(GlobalKeyPolicy::none());
/// ```
pub enum GlobalKeyPolicy {
    /// Default: Esc, Ctrl+C, Ctrl+Q are global. Same as `EventKind::is_global()`.
    Default,
    /// Only these specific key combinations are global.
    /// Each entry is `(KeyCode, required_modifiers)`.
    Keys(Vec<(KeyCode, KeyModifiers)>),
    /// Custom predicate function.
    Custom(Box<dyn Fn(&EventKind) -> bool + Send + Sync>),
}

impl Default for GlobalKeyPolicy {
    fn default() -> Self {
        Self::Default
    }
}

impl GlobalKeyPolicy {
    /// No key events are global (only Resize remains global).
    pub fn none() -> Self {
        Self::Keys(vec![])
    }

    /// Default without Esc — only Ctrl+C and Ctrl+Q are global.
    ///
    /// Useful for apps that use Esc to close modals/dialogs.
    pub fn without_esc() -> Self {
        Self::Keys(vec![
            (KeyCode::Char('c'), KeyModifiers::CONTROL),
            (KeyCode::Char('q'), KeyModifiers::CONTROL),
        ])
    }

    /// Only these specific key combinations are global.
    ///
    /// Each entry is `(KeyCode, required_modifiers)` — the key matches if
    /// its code equals the given code and its modifiers contain the
    /// required modifiers.
    pub fn keys(keys: Vec<(KeyCode, KeyModifiers)>) -> Self {
        Self::Keys(keys)
    }

    /// Custom predicate for full control over global classification.
    ///
    /// Resize events are still always global regardless of the predicate.
    pub fn custom(f: impl Fn(&EventKind) -> bool + Send + Sync + 'static) -> Self {
        Self::Custom(Box::new(f))
    }

    /// Check whether an event is global under this policy.
    ///
    /// Resize events always return true regardless of the policy.
    pub fn is_global(&self, event: &EventKind) -> bool {
        // Resize is always global
        if matches!(event, EventKind::Resize(..)) {
            return true;
        }
        match self {
            Self::Default => event.is_global(),
            Self::Keys(keys) => {
                if let EventKind::Key(key) = event {
                    keys.iter()
                        .any(|(code, mods)| key.code == *code && key.modifiers.contains(*mods))
                } else {
                    false
                }
            }
            Self::Custom(f) => f(event),
        }
    }
}

/// Context passed with every event
///
/// Generic over the component ID type `C` which must implement `ComponentId`.
#[derive(Debug, Clone)]
pub struct EventContext<C: ComponentId> {
    /// Current mouse position (if known)
    pub mouse_position: Option<(u16, u16)>,
    /// Active key modifiers
    pub modifiers: KeyModifiers,
    /// Component areas for hit-testing
    pub component_areas: HashMap<C, Rect>,
}

impl<C: ComponentId> Default for EventContext<C> {
    fn default() -> Self {
        Self {
            mouse_position: None,
            modifiers: KeyModifiers::NONE,
            component_areas: HashMap::new(),
        }
    }
}

impl<C: ComponentId> EventContext<C> {
    /// Create a new event context
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if a point is within a component's area
    pub fn point_in_component(&self, component: C, x: u16, y: u16) -> bool {
        self.component_areas
            .get(&component)
            .map(|area| {
                x >= area.x
                    && x < area.x.saturating_add(area.width)
                    && y >= area.y
                    && y < area.y.saturating_add(area.height)
            })
            .unwrap_or(false)
    }

    /// Get the first component containing a point
    ///
    /// The ordering is based on the internal map iteration order and should be
    /// treated as undefined. Prefer routing through `EventBus` when ordering
    /// matters.
    pub fn component_at(&self, x: u16, y: u16) -> Option<C> {
        self.component_areas
            .iter()
            .find(|(_, area)| {
                x >= area.x
                    && x < area.x.saturating_add(area.width)
                    && y >= area.y
                    && y < area.y.saturating_add(area.height)
            })
            .map(|(id, _)| *id)
    }

    /// Update the area for a component
    pub fn set_component_area(&mut self, component: C, area: Rect) {
        self.component_areas.insert(component, area);
    }
}

/// An event with its context
///
/// Generic over the component ID type `C` which must implement `ComponentId`.
#[derive(Debug, Clone)]
pub struct Event<C: ComponentId> {
    /// The event payload
    pub kind: EventKind,
    /// Context at the time of the event
    pub context: EventContext<C>,
}

impl<C: ComponentId> Event<C> {
    /// Create a new event
    pub fn new(kind: EventKind, context: EventContext<C>) -> Self {
        Self { kind, context }
    }

    /// Get the event type
    pub fn event_type(&self) -> EventType {
        self.kind.event_type()
    }

    /// Check if this is a global event
    pub fn is_global(&self) -> bool {
        self.kind.is_global()
    }
}
