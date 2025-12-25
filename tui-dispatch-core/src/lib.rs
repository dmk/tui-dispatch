//! Core traits and types for tui-dispatch
//!
//! This crate provides the foundational abstractions for building TUI applications
//! with centralized state management, following a Redux/Elm-inspired architecture.
//!
//! # Core Concepts
//!
//! - **Action**: Events that describe state changes
//! - **Store**: Centralized state container with reducer pattern
//! - **Component**: Pure UI elements that render based on props
//! - **EventBus**: Pub/sub system for event routing
//! - **Keybindings**: Context-aware key mapping
//!
//! # Example
//!
//! ```ignore
//! use tui_dispatch_core::prelude::*;
//!
//! #[derive(Action, Clone, Debug)]
//! enum MyAction {
//!     Increment,
//!     Decrement,
//! }
//!
//! #[derive(Default)]
//! struct AppState {
//!     counter: i32,
//! }
//!
//! fn reducer(state: &mut AppState, action: MyAction) -> bool {
//!     match action {
//!         MyAction::Increment => { state.counter += 1; true }
//!         MyAction::Decrement => { state.counter -= 1; true }
//!     }
//! }
//!
//! let mut store = Store::new(AppState::default(), reducer);
//! store.dispatch(MyAction::Increment);
//! ```

pub mod action;
pub mod bus;
pub mod component;
pub mod event;
pub mod keybindings;
pub mod store;

// Core trait exports
pub use action::Action;
pub use component::Component;

// Event system exports
pub use bus::{process_raw_event, spawn_event_poller, EventBus, RawEvent};
pub use event::{ComponentId, Event, EventContext, EventKind, EventType, NumericComponentId};

// Keybindings exports
pub use keybindings::{format_key_for_display, parse_key_string, BindingContext, Keybindings};

// Store exports
pub use store::{
    ComposedMiddleware, LoggingMiddleware, Middleware, NoopMiddleware, Reducer, Store,
    StoreWithMiddleware,
};

// Re-export ratatui types for convenience
pub use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    Frame,
};

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::action::Action;
    pub use crate::bus::{process_raw_event, spawn_event_poller, EventBus, RawEvent};
    pub use crate::component::Component;
    pub use crate::event::{ComponentId, Event, EventContext, EventKind, EventType, NumericComponentId};
    pub use crate::keybindings::{
        format_key_for_display, parse_key_string, BindingContext, Keybindings,
    };
    pub use crate::store::{
        ComposedMiddleware, LoggingMiddleware, Middleware, NoopMiddleware, Reducer, Store,
        StoreWithMiddleware,
    };

    // Re-export ratatui types
    pub use ratatui::{
        layout::Rect,
        style::{Color, Modifier, Style},
        text::{Line, Span, Text},
        Frame,
    };
}
