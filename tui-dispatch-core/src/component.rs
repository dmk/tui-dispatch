//! Component trait for pure UI elements

use ratatui::{layout::Rect, Frame};

use crate::event::{Event, EventType};
use crate::Action;

/// A pure UI component that renders based on props and emits actions
///
/// Components follow these rules:
/// 1. Props contain ALL read-only data needed for rendering
/// 2. `handle_event` returns actions, never mutates external state
/// 3. `render` is a pure function of props (plus internal UI state like scroll position)
///
/// Internal UI state (scroll position, selection highlight) can be stored in `&mut self`,
/// but data mutations must go through actions.
pub trait Component {
    /// Data required to render the component (read-only)
    type Props<'a>;

    /// Event types this component wants to receive
    ///
    /// Return the event types this component should be subscribed to.
    /// Global events are always delivered regardless of this.
    fn subscriptions(&self) -> Vec<EventType> {
        vec![]
    }

    /// Handle an event and return actions to dispatch
    ///
    /// The event includes context about focus, mouse position, etc.
    /// Components should check `event.context.focused_component` or
    /// `event.context.is_focused(id)` before handling non-global events.
    fn handle_event(
        &mut self,
        event: &Event,
        props: Self::Props<'_>,
    ) -> Vec<impl Action>;

    /// Render the component to the frame
    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>);

    /// Get the last rendered area (for hit-testing and focus management)
    fn area(&self) -> Option<Rect> {
        None
    }
}
