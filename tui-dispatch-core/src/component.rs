//! Component trait for pure UI elements

use ratatui::{layout::Rect, Frame};

use crate::event::{EventKind, EventType};
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
///
/// # Focus and Context
///
/// Components receive `EventKind` (the raw event) rather than the full `Event` with context.
/// Focus information and other context should be passed through `Props`. This keeps components
/// decoupled from the specific ComponentId type used by the application.
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
    /// Components receive the raw `EventKind` (key press, mouse event, etc.).
    /// Focus state and other context should be passed through `Props`.
    fn handle_event(
        &mut self,
        event: &EventKind,
        props: Self::Props<'_>,
    ) -> Vec<impl Action>;

    /// Render the component to the frame
    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>);

    /// Get the last rendered area (for hit-testing and focus management)
    fn area(&self) -> Option<Rect> {
        None
    }
}
