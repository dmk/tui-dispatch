//! Shared command names understood by built-in interactive components.
//!
//! These constants are plain `&'static str` values so they can be used anywhere
//! command strings are accepted, including keybinding setup and tests.

/// Move to the next item or line.
pub const NEXT: &str = "next";
/// Alias for [`NEXT`] used by directional widgets.
pub const DOWN: &str = "down";
/// Move to the previous item or line.
pub const PREV: &str = "prev";
/// Alias for [`PREV`] used by directional widgets.
pub const UP: &str = "up";
/// Move to the first item or top of content.
pub const FIRST: &str = "first";
/// Alias for [`FIRST`] used by home/end style keymaps.
pub const HOME: &str = "home";
/// Move to the last item or bottom of content.
pub const LAST: &str = "last";
/// Alias for [`LAST`] used by home/end style keymaps.
pub const END: &str = "end";
/// Move or collapse left in hierarchical widgets.
pub const LEFT: &str = "left";
/// Move or expand right in hierarchical widgets.
pub const RIGHT: &str = "right";
/// Toggle the current item.
pub const TOGGLE: &str = "toggle";
/// Select the current item.
pub const SELECT: &str = "select";
/// Confirm the current item.
pub const CONFIRM: &str = "confirm";
/// Move down by one page.
pub const PAGE_DOWN: &str = "page_down";
/// Move up by one page.
pub const PAGE_UP: &str = "page_up";
/// Submit the current value.
pub const SUBMIT: &str = "submit";
/// Cancel the current interaction.
pub const CANCEL: &str = "cancel";

/// Text input command names.
pub mod text_input {
    pub use super::{CANCEL, SUBMIT};

    /// Move one character backward.
    pub const MOVE_BACKWARD: &str = "move_backward";
    /// Move one character forward.
    pub const MOVE_FORWARD: &str = "move_forward";
    /// Move one word backward.
    pub const MOVE_WORD_BACKWARD: &str = "move_word_backward";
    /// Move one word forward.
    pub const MOVE_WORD_FORWARD: &str = "move_word_forward";
    /// Move to the start of the input.
    pub const MOVE_HOME: &str = "move_home";
    /// Move to the end of the input.
    pub const MOVE_END: &str = "move_end";
    /// Delete one character before the cursor.
    pub const DELETE_BACKWARD: &str = "delete_backward";
    /// Delete one character at the cursor.
    pub const DELETE_FORWARD: &str = "delete_forward";
    /// Delete one word before the cursor.
    pub const DELETE_WORD_BACKWARD: &str = "delete_word_backward";
    /// Delete one word at the cursor.
    pub const DELETE_WORD_FORWARD: &str = "delete_word_forward";

    /// Directional alias for [`MOVE_BACKWARD`].
    pub const MOVE_LEFT: &str = "move_left";
    /// Directional alias for [`MOVE_FORWARD`].
    pub const MOVE_RIGHT: &str = "move_right";
    /// Directional alias for [`MOVE_WORD_BACKWARD`].
    pub const MOVE_WORD_LEFT: &str = "move_word_left";
    /// Directional alias for [`MOVE_WORD_FORWARD`].
    pub const MOVE_WORD_RIGHT: &str = "move_word_right";
    /// Directional alias for [`DELETE_BACKWARD`].
    pub const DELETE_LEFT: &str = "delete_left";
    /// Directional alias for [`DELETE_FORWARD`].
    pub const DELETE_RIGHT: &str = "delete_right";
    /// Directional alias for [`DELETE_WORD_BACKWARD`].
    pub const DELETE_WORD_LEFT: &str = "delete_word_left";
    /// Directional alias for [`DELETE_WORD_FORWARD`].
    pub const DELETE_WORD_RIGHT: &str = "delete_word_right";
}
