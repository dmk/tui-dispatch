//! tui-dispatch: Centralized state management for Rust TUI apps
//!
//! Like Redux/Elm, but for terminals. Components are pure functions of state,
//! and all state mutations happen through dispatched actions.
//!
//! # Example
//! ```ignore
//! use tui_dispatch::prelude::*;
//!
//! #[derive(Action, Clone, Debug)]
//! enum Action {
//!     NextItem,
//!     PrevItem,
//! }
//!
//! struct ItemList;
//!
//! impl Component for ItemList {
//!     type Props<'a> = (&'a [String], usize);
//!
//!     fn handle_event(&mut self, event: &Event, _props: Self::Props<'_>) -> Vec<Action> {
//!         // ...
//!         vec![]
//!     }
//!
//!     fn render(&mut self, f: &mut Frame, area: Rect, props: Self::Props<'_>) {
//!         // ...
//!     }
//! }
//! ```

pub use tui_dispatch_core::*;
pub use tui_dispatch_macros::Action;

pub mod prelude {
    pub use tui_dispatch_core::{
        Action, Component, Event, EventContext, EventKind, EventType,
        Color, Frame, Line, Modifier, Rect, Span, Style, Text,
    };
    pub use tui_dispatch_macros::Action;
}
