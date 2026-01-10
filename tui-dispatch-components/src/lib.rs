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
//! # Example
//!
//! ```ignore
//! use tui_dispatch_components::{SelectList, SelectListProps};
//!
//! // In your render function:
//! let mut list = SelectList::default();
//! list.render(frame, area, SelectListProps {
//!     items: &state.items,
//!     selected: state.selected,
//!     is_focused: state.focus == Focus::List,
//!     show_border: true,
//!     padding_x: 0,
//!     padding_y: 0,
//!     highlight_query: None,
//!     on_select: |i| Action::Select(i),
//! });
//! ```

mod modal;
mod select_list;
mod text_input;

pub use modal::{centered_rect, render_modal, ModalStyle};
pub use select_list::{SelectList, SelectListProps};
pub use text_input::{TextInput, TextInputProps};

/// Prelude for convenient imports
pub mod prelude {
    pub use crate::{
        centered_rect, render_modal, ModalStyle, SelectList, SelectListProps, TextInput,
        TextInputProps,
    };
}
