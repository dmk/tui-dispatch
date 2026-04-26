//! Pre-built UI components for tui-dispatch
//!
//! This crate provides reusable TUI components that integrate with tui-dispatch patterns.
//! Components implement the `Component<A>` trait and emit actions via callback functions
//! passed through Props.
//!
//! # Components
//!
//! - [`SelectList`] - Scrollable selection list with keyboard navigation
//! - [`ScrollView`] - Scrollable view for pre-wrapped lines
//! - [`StatusBar`] - Left/center/right status bar with hints
//! - [`TextInput`] - Single-line text input with cursor
//! - [`TreeView`] - Hierarchical tree view with selection
//! - [`Modal`] - Overlay with dimmed background
//!
//! # Styling
//!
//! All components use unified styling objects:
//! - [`SelectListStyle`] - Styling for SelectList (base, selection, scrollbar)
//! - [`ScrollViewStyle`] - Styling for ScrollView (base, scrollbar)
//! - [`StatusBarStyle`] - Styling for StatusBar (base, hints, separators)
//! - [`TextInputStyle`] - Styling for TextInput (base, placeholder, cursor)
//! - [`TreeViewStyle`] - Styling for TreeView (base, selection, branches)
//! - [`ModalStyle`] - Styling for Modal (base, dimming)
//!
//! Common types are available in the [`style`] module.
//!
//! # Example
//!
//! ```ignore
//! use tui_dispatch_components::{
//!     Line, SelectList, SelectListBehavior, SelectListProps, SelectListStyle,
//! };
//! use std::rc::Rc;
//!
//! // In your render function:
//! let items: Vec<Line> = state.items.iter().map(|s| Line::raw(s)).collect();
//! let render_item = |item: &Line| item.clone();
//! let mut list = SelectList::default();
//! list.render(frame, area, SelectListProps {
//!     items: &items,
//!     count: items.len(),
//!     selected: state.selected,
//!     is_focused: state.focus == Focus::List,
//!     style: SelectListStyle::default(),
//!     behavior: SelectListBehavior::default(),
//!     on_select: Rc::new(|i| Action::Select(i)),
//!     render_item: &render_item,
//! });
//! ```

#[cfg(target_arch = "wasm32")]
extern crate tinycrossterm as crossterm;

mod host;
mod modal;
mod runtime;
mod scroll_view;
mod select_list;
mod status_bar;
pub mod style;
mod text_input;
mod tree_view;

pub use host::{
    ComponentDebugEntry, ComponentDebugState, ComponentHost, ComponentInput, HostLifecycleError,
    InteractiveComponent, Mounted, MountedComponentInfo, PropsFactory,
};
pub use modal::{centered_rect, Modal, ModalBehavior, ModalCloseCallback, ModalProps, ModalStyle};
pub use ratatui::text::Line;
pub use runtime::{ComponentHostRuntime, HostedRuntime, HostedRuntimeParts, RuntimeHostExt};
pub use scroll_view::{
    LinesScroller, ScrollView, ScrollViewBehavior, ScrollViewCallback, ScrollViewProps,
    ScrollViewRenderProps, ScrollViewStyle, VisibleRange,
};
pub use select_list::{
    SelectList, SelectListBehavior, SelectListCallback, SelectListProps, SelectListRenderProps,
    SelectListStyle,
};
pub use status_bar::{
    StatusBar, StatusBarHint, StatusBarItem, StatusBarProps, StatusBarSection, StatusBarStyle,
};
pub use style::{
    highlight_substring, BaseStyle, BorderStyle, Color, ComponentStyle, Modifier, Padding,
    ScrollbarStyle, SelectionStyle, Style,
};
pub use text_input::{
    TextInput, TextInputCallback, TextInputCursorCallback, TextInputProps, TextInputRenderProps,
    TextInputStyle,
};
pub use tree_view::{
    TreeBranchMode, TreeBranchStyle, TreeNode, TreeNodeRender, TreeSelectCallback,
    TreeToggleCallback, TreeView, TreeViewBehavior, TreeViewProps, TreeViewRenderProps,
    TreeViewStyle,
};

/// Prelude for convenient imports
pub mod prelude {
    pub use crate::{
        centered_rect, BaseStyle, BorderStyle, ComponentStyle, LinesScroller, Modal, ModalBehavior,
        ModalCloseCallback, ModalProps, ModalStyle, Padding, ScrollView, ScrollViewBehavior,
        ScrollViewCallback, ScrollViewProps, ScrollViewRenderProps, ScrollViewStyle,
        ScrollbarStyle, SelectList, SelectListBehavior, SelectListCallback, SelectListProps,
        SelectListRenderProps, SelectListStyle, SelectionStyle, StatusBar, StatusBarHint,
        StatusBarItem, StatusBarProps, StatusBarSection, StatusBarStyle, TextInput,
        TextInputCallback, TextInputCursorCallback, TextInputProps, TextInputRenderProps,
        TextInputStyle, TreeBranchMode, TreeBranchStyle, TreeNode, TreeNodeRender,
        TreeSelectCallback, TreeToggleCallback, TreeView, TreeViewBehavior, TreeViewProps,
        TreeViewRenderProps, TreeViewStyle, VisibleRange,
    };

    pub use crate::{
        ComponentDebugEntry, ComponentDebugState, ComponentHost, ComponentInput,
        HostLifecycleError, HostedRuntime, InteractiveComponent, Mounted, MountedComponentInfo,
        PropsFactory, RuntimeHostExt,
    };

    pub use ratatui::style::{Color, Modifier, Style};
    pub use ratatui::text::Line;
}
