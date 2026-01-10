//! Component trait for pure UI elements

use ratatui::{layout::Rect, Frame};

use crate::event::EventKind;

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
///
/// # Example
///
/// ```ignore
/// use tui_dispatch::{Component, EventKind, Frame, Rect};
///
/// struct Counter;
///
/// struct CounterProps {
///     count: i32,
///     is_focused: bool,
/// }
///
/// impl Component<AppAction> for Counter {
///     type Props<'a> = CounterProps;
///
///     fn handle_event(&mut self, event: &EventKind, props: Self::Props<'_>) -> Vec<AppAction> {
///         if !props.is_focused {
///             return vec![];
///         }
///         if let EventKind::Key(key) = event {
///             match key.code {
///                 KeyCode::Up => return vec![AppAction::Increment],
///                 KeyCode::Down => return vec![AppAction::Decrement],
///                 _ => {}
///             }
///         }
///         vec![]
///     }
///
///     fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
///         let text = format!("Count: {}", props.count);
///         frame.render_widget(Paragraph::new(text), area);
///     }
/// }
/// ```
pub trait Component<A> {
    /// Data required to render the component (read-only)
    type Props<'a>;

    /// Handle an event and return actions to dispatch
    ///
    /// Components receive the raw `EventKind` (key press, mouse event, etc.).
    /// Focus state and other context should be passed through `Props`.
    ///
    /// Default implementation returns no actions (render-only components).
    #[allow(unused_variables)]
    fn handle_event(&mut self, event: &EventKind, props: Self::Props<'_>) -> Vec<A> {
        vec![]
    }

    /// Render the component to the frame
    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>);
}
