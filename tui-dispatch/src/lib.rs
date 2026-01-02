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
//! enum MyAction {
//!     NextItem,
//!     PrevItem,
//! }
//!
//! #[derive(ComponentId, Clone, Copy, PartialEq, Eq, Hash, Debug)]
//! enum MyComponentId {
//!     List,
//!     Detail,
//! }
//! ```

// Re-export everything from core
pub use tui_dispatch_core::*;

// Re-export derive macros
pub use tui_dispatch_macros::{Action, BindingContext, ComponentId, DebugState, FeatureFlags};

/// Prelude for convenient imports
pub mod prelude {
    // Traits
    pub use tui_dispatch_core::{Action, ActionCategory, BindingContext, Component, ComponentId};

    // Event system
    pub use tui_dispatch_core::{
        process_raw_event, spawn_event_poller, Event, EventBus, EventContext, EventKind, EventType,
        NumericComponentId, RawEvent,
    };

    // Keybindings
    pub use tui_dispatch_core::{format_key_for_display, parse_key_string, Keybindings};

    // Store
    pub use tui_dispatch_core::{
        ComposedMiddleware, LoggingMiddleware, Middleware, NoopMiddleware, Reducer, Store,
        StoreWithMiddleware,
    };

    // Effects
    pub use tui_dispatch_core::{
        DispatchResult, EffectReducer, EffectStore, EffectStoreWithMiddleware,
    };

    // Tasks (requires "tasks" feature)
    #[cfg(feature = "tasks")]
    pub use tui_dispatch_core::{TaskKey, TaskManager};

    // Subscriptions (requires "subscriptions" feature)
    #[cfg(feature = "subscriptions")]
    pub use tui_dispatch_core::{SubKey, Subscriptions};

    // Debug
    pub use tui_dispatch_core::debug::{
        ActionLoggerConfig, ActionLoggerMiddleware, DebugFreeze, DebugOverlay, DebugTableBuilder,
    };

    // Derive macros
    pub use tui_dispatch_macros::{Action, BindingContext, ComponentId, DebugState, FeatureFlags};

    // Ratatui re-exports
    pub use tui_dispatch_core::{Color, Frame, Line, Modifier, Rect, Span, Style, Text};
}
