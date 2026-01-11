//! Pre-built UI components for tui-dispatch
//!
//! This crate provides reusable TUI components that integrate with tui-dispatch patterns.
//! Components implement the `Component<A>` trait and emit actions via callback functions
//! passed through Props.
//!
//! # Components
//!
//! - [`SelectList`] - Scrollable selection list with keyboard navigation
//! - [`TextInput`] - Single-line text input with cursor
//! - [`Modal`] - Overlay with dimmed background snapshot
//!
//! # Styling
//!
//! All components use unified styling objects:
//! - [`ListStyle`] - Styling for SelectList (border, padding, selection)
//! - [`InputStyle`] - Styling for TextInput (border, padding, colors)
//! - [`ModalStyle`] - Styling for Modal (dim factor, background, border)
//!
//! Common types are available in the [`style`] module.
//!
//! # Example
//!
//! ```ignore
//! use tui_dispatch_components::{SelectList, SelectListProps, ListStyle, ListBehavior, Line};
//!
//! // In your render function:
//! let items: Vec<Line> = state.items.iter().map(|s| Line::raw(s)).collect();
//! let mut list = SelectList::default();
//! list.render(frame, area, SelectListProps {
//!     items: &items,
//!     count: items.len(),
//!     selected: state.selected,
//!     is_focused: state.focus == Focus::List,
//!     style: ListStyle::default(),
//!     behavior: ListBehavior::default(),
//!     on_select: |i| Action::Select(i),
//! });
//! ```

mod modal;
mod select_list;
pub mod style;
mod text_input;

pub use modal::{centered_rect, render_modal, ModalStyle};
pub use ratatui::text::Line;
pub use select_list::{ListBehavior, ListStyle, SelectList, SelectListProps};
pub use style::{
    highlight_substring, BorderStyle, Color, ComponentStyle, Modifier, Padding, SelectionStyle,
    Style,
};
pub use text_input::{InputStyle, TextInput, TextInputProps};

/// Prelude for convenient imports
pub mod prelude {
    pub use crate::{
        centered_rect, render_modal, BorderStyle, ComponentStyle, InputStyle, ListBehavior,
        ListStyle, ModalStyle, Padding, SelectList, SelectListProps, SelectionStyle, TextInput,
        TextInputProps,
    };
    pub use ratatui::style::{Color, Modifier, Style};
    pub use ratatui::text::Line;
}
