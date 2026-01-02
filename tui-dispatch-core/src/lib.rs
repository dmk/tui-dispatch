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
//! # Basic Example
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
//!
//! # Async Handler Pattern
//!
//! For applications with async operations (API calls, file I/O, etc.), use a two-phase
//! action pattern:
//!
//! 1. **Intent actions** trigger async work (e.g., `FetchData`)
//! 2. **Result actions** carry the outcome back (e.g., `DidFetchData`, `DidFetchError`)
//!
//! ```ignore
//! use tokio::sync::mpsc;
//!
//! #[derive(Action, Clone, Debug)]
//! #[action(infer_categories)]
//! enum Action {
//!     // Intent: triggers async fetch
//!     DataFetch { id: String },
//!     // Result: async operation completed
//!     DataDidLoad { id: String, payload: Vec<u8> },
//!     DataDidError { id: String, error: String },
//! }
//!
//! // Async handler spawns a task and sends result back via channel
//! fn handle_async(action: &Action, tx: mpsc::UnboundedSender<Action>) {
//!     match action {
//!         Action::DataFetch { id } => {
//!             let id = id.clone();
//!             let tx = tx.clone();
//!             tokio::spawn(async move {
//!                 match fetch_from_api(&id).await {
//!                     Ok(payload) => tx.send(Action::DataDidLoad { id, payload }),
//!                     Err(e) => tx.send(Action::DataDidError { id, error: e.to_string() }),
//!                 }
//!             });
//!         }
//!         _ => {}
//!     }
//! }
//!
//! // Main loop receives actions from both events and async completions
//! loop {
//!     tokio::select! {
//!         Some(action) = action_rx.recv() => {
//!             handle_async(&action, action_tx.clone());
//!             store.dispatch(action);
//!         }
//!         // ... event handling
//!     }
//! }
//! ```
//!
//! The `Did*` naming convention clearly identifies result actions. With `#[action(infer_categories)]`,
//! these are automatically grouped (e.g., `DataFetch` and `DataDidLoad` both get category `"data"`).

pub mod action;
pub mod bus;
pub mod component;
pub mod debug;
pub mod effect;
pub mod event;
pub mod features;
pub mod keybindings;
pub mod store;
#[cfg(feature = "subscriptions")]
pub mod subscriptions;
#[cfg(feature = "tasks")]
pub mod tasks;
pub mod testing;

// Core trait exports
pub use action::{Action, ActionCategory, ActionSummary};
pub use component::Component;
pub use features::{DynamicFeatures, FeatureFlags};

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

// Effect exports
pub use effect::{DispatchResult, EffectReducer, EffectStore, EffectStoreWithMiddleware};

// Task exports (requires "tasks" feature)
#[cfg(feature = "tasks")]
pub use tasks::{TaskKey, TaskManager};

// Subscription exports (requires "subscriptions" feature)
#[cfg(feature = "subscriptions")]
pub use subscriptions::{SubKey, Subscriptions};

// Re-export ratatui types for convenience
pub use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    Frame,
};

// Testing exports
pub use testing::{
    alt_key, buffer_rect_to_string_plain, buffer_to_string, buffer_to_string_plain, char_key,
    ctrl_key, into_event, key, key_event, key_events, keys, ActionAssertions, ActionAssertionsEq,
    RenderHarness, TestHarness,
};

#[cfg(feature = "testing-time")]
pub use testing::{advance_time, pause_time, resume_time};

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::action::{Action, ActionCategory};
    pub use crate::bus::{process_raw_event, spawn_event_poller, EventBus, RawEvent};
    pub use crate::component::Component;
    pub use crate::effect::{
        DispatchResult, EffectReducer, EffectStore, EffectStoreWithMiddleware,
    };
    pub use crate::event::{
        ComponentId, Event, EventContext, EventKind, EventType, NumericComponentId,
    };
    pub use crate::features::{DynamicFeatures, FeatureFlags};
    pub use crate::keybindings::{
        format_key_for_display, parse_key_string, BindingContext, Keybindings,
    };
    pub use crate::store::{
        ComposedMiddleware, LoggingMiddleware, Middleware, NoopMiddleware, Reducer, Store,
        StoreWithMiddleware,
    };
    #[cfg(feature = "subscriptions")]
    pub use crate::subscriptions::{SubKey, Subscriptions};
    #[cfg(feature = "tasks")]
    pub use crate::tasks::{TaskKey, TaskManager};

    // Re-export ratatui types
    pub use ratatui::{
        layout::Rect,
        style::{Color, Modifier, Style},
        text::{Line, Span, Text},
        Frame,
    };
}
