//! tui-dispatch: Centralized state management for Rust TUI apps
//!
//! Like Redux/Elm, but for terminals. Components are pure functions of state,
//! and all state mutations happen through dispatched actions.
//!
//! # Quick Start
//!
//! ```
//! use tui_dispatch::prelude::*;
//!
//! // Define your actions
//! #[derive(Action, Clone, Debug)]
//! enum MyAction {
//!     Increment,
//!     Decrement,
//!     Quit,
//! }
//!
//! // Define your state
//! #[derive(Default)]
//! struct MyState {
//!     count: i32,
//! }
//!
//! // Write a reducer
//! fn reducer(state: &mut MyState, action: MyAction) -> bool {
//!     match action {
//!         MyAction::Increment => { state.count += 1; true }
//!         MyAction::Decrement => { state.count -= 1; true }
//!         MyAction::Quit => false,
//!     }
//! }
//!
//! // Create a store
//! let mut store = Store::new(MyState::default(), reducer);
//! store.dispatch(MyAction::Increment);
//! assert_eq!(store.state().count, 1);
//! ```
//!
//! # With Effects
//!
//! For async operations, use `EffectStore` with `ReducerResult`:
//!
//! ```
//! use tui_dispatch::prelude::*;
//!
//! #[derive(Action, Clone, Debug)]
//! enum Action {
//!     Fetch,
//!     DidLoad(String),
//! }
//!
//! enum Effect {
//!     FetchData,
//! }
//!
//! #[derive(Default)]
//! struct State {
//!     data: Option<String>,
//!     loading: bool,
//! }
//!
//! fn reducer(state: &mut State, action: Action) -> ReducerResult<Effect> {
//!     match action {
//!         Action::Fetch => {
//!             state.loading = true;
//!             ReducerResult::changed_with(Effect::FetchData)
//!         }
//!         Action::DidLoad(data) => {
//!             state.data = Some(data);
//!             state.loading = false;
//!             ReducerResult::changed()
//!         }
//!     }
//! }
//!
//! let mut store = EffectStore::new(State::default(), reducer);
//! let result = store.dispatch(Action::Fetch);
//! assert!(result.changed);
//! assert_eq!(result.effects.len(), 1);
//! ```
//!
//! See the [documentation](https://docs.rs/tui-dispatch) for full guides.

// Re-export everything from core
pub use tui_dispatch_core::reducer_compose;
pub use tui_dispatch_core::*;

// Debug utilities
pub use tui_dispatch_debug::debug;

// Re-export derive macros
pub use tui_dispatch_macros::{Action, BindingContext, ComponentId, DebugState, FeatureFlags};

/// Prelude for convenient imports
pub mod prelude {
    // Traits
    pub use tui_dispatch_core::{
        Action, ActionCategory, ActionParams, BindingContext, Component, ComponentId,
    };

    // Event system
    pub use tui_dispatch_core::{
        process_raw_event, spawn_event_poller, DefaultBindingContext, Event, EventBus,
        EventContext, EventHandler, EventKind, EventRoutingState, EventType, GlobalKeyPolicy,
        HandlerResponse, NumericComponentId, RawEvent, RouteTarget, RoutedEvent, SimpleEventBus,
    };

    // Keybindings
    pub use tui_dispatch_core::{format_key_for_display, parse_key_string, Keybindings};

    // Store
    pub use tui_dispatch_core::reducer_compose;
    #[cfg(feature = "tracing")]
    pub use tui_dispatch_core::LoggingMiddleware;
    pub use tui_dispatch_core::{
        ComposedMiddleware, Middleware, NoopMiddleware, Reducer, Store, StoreWithMiddleware,
    };

    // Effects and state types
    pub use tui_dispatch_core::{
        DataResource, EffectReducer, EffectStore, EffectStoreWithMiddleware, ReducerResult,
    };

    // Runtime helpers
    pub use tui_dispatch_core::{
        DispatchRuntime, DispatchStore, EffectContext, EffectRuntime, EffectStoreLike,
        EventOutcome, PollerConfig, RenderContext,
    };

    // Tasks (requires "tasks" feature)
    #[cfg(feature = "tasks")]
    pub use tui_dispatch_core::{TaskKey, TaskManager, TaskPauseHandle};

    // Subscriptions (requires "subscriptions" feature)
    #[cfg(feature = "subscriptions")]
    pub use tui_dispatch_core::{SubKey, SubPauseHandle, Subscriptions};

    // Debug
    pub use tui_dispatch_debug::debug::{
        ActionLoggerConfig, ActionLoggerMiddleware, DebugFreeze, DebugOverlay, DebugTableBuilder,
    };

    // Derive macros
    pub use tui_dispatch_macros::{Action, BindingContext, ComponentId, DebugState, FeatureFlags};

    // Ratatui re-exports
    pub use tui_dispatch_core::{Color, Frame, Line, Modifier, Rect, Span, Style, Text};
}
