//! Event types for the pub/sub system

use crossterm::event::{KeyEvent, KeyModifiers, MouseEvent};
use ratatui::layout::Rect;
use std::collections::HashMap;

/// Unique identifier for components
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComponentId(pub u32);

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
    /// Scroll event with position and delta
    Scroll { column: u16, row: u16, delta: isize },
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
}

/// Context passed with every event
#[derive(Debug, Clone)]
pub struct EventContext {
    /// Currently focused component
    pub focused_component: Option<ComponentId>,
    /// Current mouse position (if known)
    pub mouse_position: Option<(u16, u16)>,
    /// Active key modifiers
    pub modifiers: KeyModifiers,
    /// Component areas for hit-testing
    pub component_areas: HashMap<ComponentId, Rect>,
    /// Whether a modal is currently open
    pub is_modal_open: bool,
    /// The active modal (if any)
    pub active_modal: Option<ComponentId>,
}

impl Default for EventContext {
    fn default() -> Self {
        Self {
            focused_component: None,
            mouse_position: None,
            modifiers: KeyModifiers::NONE,
            component_areas: HashMap::new(),
            is_modal_open: false,
            active_modal: None,
        }
    }
}

impl EventContext {
    /// Create a new event context
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if a component is focused
    pub fn is_focused(&self, component: ComponentId) -> bool {
        self.focused_component == Some(component)
    }

    /// Check if a point is within a component's area
    pub fn point_in_component(&self, component: ComponentId, x: u16, y: u16) -> bool {
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

    /// Get the component at a given point
    pub fn component_at(&self, x: u16, y: u16) -> Option<ComponentId> {
        if let Some(modal) = self.active_modal {
            if self.point_in_component(modal, x, y) {
                return Some(modal);
            }
        }

        for (&id, area) in &self.component_areas {
            if Some(id) != self.active_modal
                && x >= area.x
                && x < area.x.saturating_add(area.width)
                && y >= area.y
                && y < area.y.saturating_add(area.height)
            {
                return Some(id);
            }
        }
        None
    }

    /// Update the area for a component
    pub fn set_component_area(&mut self, component: ComponentId, area: Rect) {
        self.component_areas.insert(component, area);
    }

    /// Set the focused component
    pub fn set_focus(&mut self, component: Option<ComponentId>) {
        self.focused_component = component;
    }

    /// Set modal state
    pub fn set_modal(&mut self, modal: Option<ComponentId>) {
        self.active_modal = modal;
        self.is_modal_open = modal.is_some();
        if let Some(m) = modal {
            self.focused_component = Some(m);
        }
    }
}

/// An event with its context
#[derive(Debug, Clone)]
pub struct Event {
    /// The event payload
    pub kind: EventKind,
    /// Context at the time of the event
    pub context: EventContext,
}

impl Event {
    /// Create a new event
    pub fn new(kind: EventKind, context: EventContext) -> Self {
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
