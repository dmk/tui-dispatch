//! Centralized state store with reducer pattern

use crate::Action;
use std::marker::PhantomData;

/// A reducer function that handles actions and mutates state
///
/// Returns `true` if the state changed and a re-render is needed.
pub type Reducer<S, A> = fn(&mut S, A) -> bool;

/// Compose a reducer by routing actions to focused handlers.
///
/// # When to Use
///
/// For most reducers, a flat `match` is simpler and clearer. Use this macro when:
/// - Your reducer exceeds ~500 lines and splitting improves organization
/// - You have **context-aware routing** (e.g., vim normal vs command mode)
/// - Handlers live in **separate modules** and you want clean composition
///
/// # Syntax
///
/// ```text
/// reducer_compose!(state, action, {
///     // Arms are tried in order, first match wins
///     category "name" => handler,      // Route by action category
///     Action::Specific => handler,     // Route by pattern match
///     _ => fallback_handler,           // Catch-all (required last)
/// })
///
/// // With context (e.g., for modal/mode-aware routing):
/// reducer_compose!(state, action, context, {
///     context Mode::Command => handle_command,  // Route by context value
///     category "nav" => handle_nav,
///     _ => handle_default,
/// })
/// ```
///
/// # Arm Types
///
/// **`category "name" => handler`** - Routes actions where
/// `ActionCategory::category(&action) == Some("name")`. Requires
/// `#[action(infer_categories)]` on your action enum.
///
/// **`context Value => handler`** - Routes when the context expression equals
/// `Value`. Only available in the 4-argument form.
///
/// **`Pattern => handler`** - Standard pattern match (e.g., `Action::Quit`,
/// `Action::Input(_)`).
///
/// **`_ => handler`** - Catch-all fallback. Must be last.
///
/// # Handler Signature
///
/// All handlers must have the same signature:
/// ```text
/// fn handler(state: &mut S, action: A) -> R
/// ```
/// Where `R` is typically `bool` or `ReducerResult<E>`.
///
/// # Category Inference
///
/// With `#[action(infer_categories)]`, categories are inferred from action
/// names by taking the prefix before the verb:
///
/// | Action | Verb | Category |
/// |--------|------|----------|
/// | `NavScrollUp` | Scroll | `"nav"` |
/// | `SearchQuerySubmit` | Submit | `"search_query"` |
/// | `WeatherDidLoad` | Did | `"weather"` |
/// | `Quit` | (none) | `None` |
///
/// For predictable categories, use explicit `#[category = "name"]` attributes.
///
/// # Example
///
/// ```ignore
/// fn reducer(state: &mut AppState, action: Action, mode: Mode) -> bool {
///     reducer_compose!(state, action, mode, {
///         // Command mode gets priority
///         context Mode::Command => handle_command,
///         // Then route by category
///         category "nav" => handle_navigation,
///         category "search" => handle_search,
///         // Specific actions
///         Action::Quit => |_, _| false,
///         // Everything else
///         _ => handle_ui,
///     })
/// }
///
/// fn handle_navigation(state: &mut AppState, action: Action) -> bool {
///     match action {
///         Action::NavUp => { state.cursor -= 1; true }
///         Action::NavDown => { state.cursor += 1; true }
///         _ => false,
///     }
/// }
/// ```
#[macro_export]
macro_rules! reducer_compose {
    // 3-argument form must come first to prevent $context:expr from matching the braces
    ($state:expr, $action:expr, { $($arms:tt)+ }) => {{
        let __state = $state;
        let __action_input = $action;
        let __context = ();
        $crate::reducer_compose!(@accum __state, __action_input, __context; () $($arms)+)
    }};
    ($state:expr, $action:expr, $context:expr, { $($arms:tt)+ }) => {{
        let __state = $state;
        let __action_input = $action;
        let __context = $context;
        $crate::reducer_compose!(@accum __state, __action_input, __context; () $($arms)+)
    }};
    (@accum $state:ident, $action:ident, $context:ident; ($($out:tt)*) category $category:expr => $handler:expr, $($rest:tt)+) => {
        $crate::reducer_compose!(
            @accum $state, $action, $context;
            (
                $($out)*
                __action if $crate::ActionCategory::category(&__action) == Some($category) => {
                    ($handler)($state, __action)
                },
            )
            $($rest)+
        )
    };
    (@accum $state:ident, $action:ident, $context:ident; ($($out:tt)*) context $context_value:expr => $handler:expr, $($rest:tt)+) => {
        $crate::reducer_compose!(
            @accum $state, $action, $context;
            (
                $($out)*
                __action if $context == $context_value => {
                    ($handler)($state, __action)
                },
            )
            $($rest)+
        )
    };
    (@accum $state:ident, $action:ident, $context:ident; ($($out:tt)*) _ => $handler:expr, $($rest:tt)+) => {
        $crate::reducer_compose!(
            @accum $state, $action, $context;
            (
                $($out)*
                __action => {
                    ($handler)($state, __action)
                },
            )
            $($rest)+
        )
    };
    (@accum $state:ident, $action:ident, $context:ident; ($($out:tt)*) $pattern:pat $(if $guard:expr)? => $handler:expr, $($rest:tt)+) => {
        $crate::reducer_compose!(
            @accum $state, $action, $context;
            (
                $($out)*
                __action @ $pattern $(if $guard)? => {
                    ($handler)($state, __action)
                },
            )
            $($rest)+
        )
    };
    (@accum $state:ident, $action:ident, $context:ident; ($($out:tt)*) category $category:expr => $handler:expr $(,)?) => {
        match $action {
            $($out)*
            __action if $crate::ActionCategory::category(&__action) == Some($category) => {
                ($handler)($state, __action)
            }
        }
    };
    (@accum $state:ident, $action:ident, $context:ident; ($($out:tt)*) context $context_value:expr => $handler:expr $(,)?) => {
        match $action {
            $($out)*
            __action if $context == $context_value => {
                ($handler)($state, __action)
            }
        }
    };
    (@accum $state:ident, $action:ident, $context:ident; ($($out:tt)*) _ => $handler:expr $(,)?) => {
        match $action {
            $($out)*
            __action => {
                ($handler)($state, __action)
            }
        }
    };
    (@accum $state:ident, $action:ident, $context:ident; ($($out:tt)*) $pattern:pat $(if $guard:expr)? => $handler:expr $(,)?) => {
        match $action {
            $($out)*
            __action @ $pattern $(if $guard)? => {
                ($handler)($state, __action)
            }
        }
    };
}

/// Centralized state store with Redux-like reducer pattern
///
/// The store holds the application state and provides a single point
/// for state mutations through the `dispatch` method.
///
/// # Type Parameters
/// * `S` - The application state type
/// * `A` - The action type (must implement `Action`)
///
/// # Example
/// ```
/// use tui_dispatch_core::{Action, Store};
///
/// #[derive(Clone, Debug)]
/// enum MyAction { Increment, Decrement }
///
/// impl Action for MyAction {
///     fn name(&self) -> &'static str {
///         match self {
///             MyAction::Increment => "Increment",
///             MyAction::Decrement => "Decrement",
///         }
///     }
/// }
///
/// #[derive(Default)]
/// struct AppState { counter: i32 }
///
/// fn reducer(state: &mut AppState, action: MyAction) -> bool {
///     match action {
///         MyAction::Increment => { state.counter += 1; true }
///         MyAction::Decrement => { state.counter -= 1; true }
///     }
/// }
///
/// let mut store = Store::new(AppState::default(), reducer);
/// store.dispatch(MyAction::Increment);
/// assert_eq!(store.state().counter, 1);
/// ```
pub struct Store<S, A: Action> {
    state: S,
    reducer: Reducer<S, A>,
    _marker: PhantomData<A>,
}

impl<S, A: Action> Store<S, A> {
    /// Create a new store with initial state and reducer
    pub fn new(state: S, reducer: Reducer<S, A>) -> Self {
        Self {
            state,
            reducer,
            _marker: PhantomData,
        }
    }

    /// Dispatch an action to the store
    ///
    /// The reducer will be called with the current state and action.
    /// Returns `true` if the state changed and a re-render is needed.
    pub fn dispatch(&mut self, action: A) -> bool {
        (self.reducer)(&mut self.state, action)
    }

    /// Get a reference to the current state
    pub fn state(&self) -> &S {
        &self.state
    }

    /// Get a mutable reference to the state
    ///
    /// Use this sparingly - prefer dispatching actions for state changes.
    /// This is useful for initializing state or for cases where the
    /// action pattern doesn't fit well.
    pub fn state_mut(&mut self) -> &mut S {
        &mut self.state
    }
}

/// Store with middleware support
///
/// Wraps a `Store` and allows middleware to intercept actions
/// before and after they are processed by the reducer.
pub struct StoreWithMiddleware<S, A: Action, M: Middleware<S, A>> {
    store: Store<S, A>,
    middleware: M,
    dispatch_depth: usize,
}

impl<S, A: Action, M: Middleware<S, A>> StoreWithMiddleware<S, A, M> {
    /// Create a new store with middleware
    pub fn new(state: S, reducer: Reducer<S, A>, middleware: M) -> Self {
        Self {
            store: Store::new(state, reducer),
            middleware,
            dispatch_depth: 0,
        }
    }

    /// Dispatch an action through middleware and store
    ///
    /// The action passes through `middleware.before()` (which can cancel it),
    /// then the reducer, then `middleware.after()` (which can inject follow-up actions).
    /// Injected actions go through the full pipeline recursively.
    pub fn dispatch(&mut self, action: A) -> bool {
        self.dispatch_depth += 1;
        assert!(
            self.dispatch_depth <= MAX_DISPATCH_DEPTH,
            "middleware dispatch depth exceeded {MAX_DISPATCH_DEPTH} — likely infinite injection loop"
        );

        if self.middleware.before(&action, &self.store.state) {
            let mut changed = self.store.dispatch(action.clone());
            let injected = self.middleware.after(&action, changed, &self.store.state);
            for a in injected {
                changed |= self.dispatch(a);
            }
            self.dispatch_depth -= 1;
            changed
        } else {
            self.dispatch_depth -= 1;
            false
        }
    }

    /// Get a reference to the current state
    pub fn state(&self) -> &S {
        self.store.state()
    }

    /// Get a mutable reference to the state
    pub fn state_mut(&mut self) -> &mut S {
        self.store.state_mut()
    }

    /// Get a reference to the middleware
    pub fn middleware(&self) -> &M {
        &self.middleware
    }

    /// Get a mutable reference to the middleware
    pub fn middleware_mut(&mut self) -> &mut M {
        &mut self.middleware
    }
}

/// Maximum dispatch depth before panicking to prevent infinite middleware injection loops.
pub(crate) const MAX_DISPATCH_DEPTH: usize = 16;

/// Middleware trait for intercepting actions
///
/// Implement this trait to add logging, persistence, throttling, or other
/// cross-cutting concerns to your store. Middleware can:
///
/// - **Observe**: inspect actions and state (logging, analytics, persistence)
/// - **Cancel**: return `false` from `before()` to prevent the action from reaching the reducer
/// - **Inject**: return follow-up actions from `after()` that are dispatched through the full pipeline
///
/// # Cancel
///
/// Return `false` from `before()` to cancel the action — the reducer is never called and
/// `after()` is not invoked. Useful for throttling, validation, and auth guards.
///
/// # Inject
///
/// Return actions from `after()` to trigger follow-up dispatches. Injected actions go through
/// the full middleware + reducer pipeline. A recursion depth limit prevents infinite loops.
///
/// Useful for cascading behavior: moving a card to "Done" triggers a notification,
/// without the move reducer knowing about notifications.
pub trait Middleware<S, A: Action> {
    /// Called before the action is dispatched to the reducer.
    ///
    /// Return `true` to proceed with dispatch, `false` to cancel.
    fn before(&mut self, action: &A, state: &S) -> bool;

    /// Called after the action is processed by the reducer.
    ///
    /// Return any follow-up actions to dispatch through the full pipeline.
    fn after(&mut self, action: &A, state_changed: bool, state: &S) -> Vec<A>;
}

/// A no-op middleware that does nothing
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopMiddleware;

impl<S, A: Action> Middleware<S, A> for NoopMiddleware {
    fn before(&mut self, _action: &A, _state: &S) -> bool {
        true
    }
    fn after(&mut self, _action: &A, _state_changed: bool, _state: &S) -> Vec<A> {
        vec![]
    }
}

/// Middleware that logs actions via the `tracing` crate.
///
/// Requires the `tracing` feature.
#[cfg(feature = "tracing")]
#[derive(Debug, Clone, Default)]
pub struct LoggingMiddleware {
    /// Whether to log before dispatch
    pub log_before: bool,
    /// Whether to log after dispatch
    pub log_after: bool,
}

#[cfg(feature = "tracing")]
impl LoggingMiddleware {
    /// Create a new logging middleware with default settings (log after only)
    pub fn new() -> Self {
        Self {
            log_before: false,
            log_after: true,
        }
    }

    /// Create a logging middleware that logs both before and after
    pub fn verbose() -> Self {
        Self {
            log_before: true,
            log_after: true,
        }
    }
}

#[cfg(feature = "tracing")]
impl<S, A: Action> Middleware<S, A> for LoggingMiddleware {
    fn before(&mut self, action: &A, _state: &S) -> bool {
        if self.log_before {
            tracing::debug!(action = %action.name(), "Dispatching action");
        }
        true
    }

    fn after(&mut self, action: &A, state_changed: bool, _state: &S) -> Vec<A> {
        if self.log_after {
            tracing::debug!(
                action = %action.name(),
                state_changed = state_changed,
                "Action processed"
            );
        }
        vec![]
    }
}

/// Compose multiple middleware into a single middleware
pub struct ComposedMiddleware<S, A: Action> {
    middlewares: Vec<Box<dyn Middleware<S, A>>>,
}

impl<S, A: Action> std::fmt::Debug for ComposedMiddleware<S, A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComposedMiddleware")
            .field("middlewares_count", &self.middlewares.len())
            .finish()
    }
}

impl<S, A: Action> Default for ComposedMiddleware<S, A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S, A: Action> ComposedMiddleware<S, A> {
    /// Create a new composed middleware
    pub fn new() -> Self {
        Self {
            middlewares: Vec::new(),
        }
    }

    /// Add a middleware to the composition
    pub fn add<M: Middleware<S, A> + 'static>(&mut self, middleware: M) {
        self.middlewares.push(Box::new(middleware));
    }
}

impl<S, A: Action> Middleware<S, A> for ComposedMiddleware<S, A> {
    fn before(&mut self, action: &A, state: &S) -> bool {
        for middleware in &mut self.middlewares {
            if !middleware.before(action, state) {
                return false;
            }
        }
        true
    }

    fn after(&mut self, action: &A, state_changed: bool, state: &S) -> Vec<A> {
        let mut injected = Vec::new();
        // Call in reverse order for proper nesting
        for middleware in self.middlewares.iter_mut().rev() {
            injected.extend(middleware.after(action, state_changed, state));
        }
        injected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ActionCategory;

    #[derive(Default)]
    struct TestState {
        counter: i32,
    }

    #[derive(Clone, Debug)]
    enum TestAction {
        Increment,
        Decrement,
        NoOp,
    }

    impl Action for TestAction {
        fn name(&self) -> &'static str {
            match self {
                TestAction::Increment => "Increment",
                TestAction::Decrement => "Decrement",
                TestAction::NoOp => "NoOp",
            }
        }
    }

    fn test_reducer(state: &mut TestState, action: TestAction) -> bool {
        match action {
            TestAction::Increment => {
                state.counter += 1;
                true
            }
            TestAction::Decrement => {
                state.counter -= 1;
                true
            }
            TestAction::NoOp => false,
        }
    }

    #[test]
    fn test_store_dispatch() {
        let mut store = Store::new(TestState::default(), test_reducer);

        assert!(store.dispatch(TestAction::Increment));
        assert_eq!(store.state().counter, 1);

        assert!(store.dispatch(TestAction::Increment));
        assert_eq!(store.state().counter, 2);

        assert!(store.dispatch(TestAction::Decrement));
        assert_eq!(store.state().counter, 1);
    }

    #[test]
    fn test_store_noop() {
        let mut store = Store::new(TestState::default(), test_reducer);

        assert!(!store.dispatch(TestAction::NoOp));
        assert_eq!(store.state().counter, 0);
    }

    #[test]
    fn test_store_state_mut() {
        let mut store = Store::new(TestState::default(), test_reducer);

        store.state_mut().counter = 100;
        assert_eq!(store.state().counter, 100);
    }

    #[derive(Default)]
    struct CountingMiddleware {
        before_count: usize,
        after_count: usize,
    }

    impl<S, A: Action> Middleware<S, A> for CountingMiddleware {
        fn before(&mut self, _action: &A, _state: &S) -> bool {
            self.before_count += 1;
            true
        }

        fn after(&mut self, _action: &A, _state_changed: bool, _state: &S) -> Vec<A> {
            self.after_count += 1;
            vec![]
        }
    }

    #[test]
    fn test_store_with_middleware() {
        let mut store = StoreWithMiddleware::new(
            TestState::default(),
            test_reducer,
            CountingMiddleware::default(),
        );

        store.dispatch(TestAction::Increment);
        store.dispatch(TestAction::Increment);

        assert_eq!(store.middleware().before_count, 2);
        assert_eq!(store.middleware().after_count, 2);
        assert_eq!(store.state().counter, 2);
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    enum ComposeContext {
        Default,
        Command,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    enum ComposeCategory {
        Nav,
        Search,
        Uncategorized,
    }

    #[derive(Clone, Debug)]
    enum ComposeAction {
        NavUp,
        Search,
        Other,
    }

    impl Action for ComposeAction {
        fn name(&self) -> &'static str {
            match self {
                ComposeAction::NavUp => "NavUp",
                ComposeAction::Search => "Search",
                ComposeAction::Other => "Other",
            }
        }
    }

    impl ActionCategory for ComposeAction {
        type Category = ComposeCategory;

        fn category(&self) -> Option<&'static str> {
            match self {
                ComposeAction::NavUp => Some("nav"),
                ComposeAction::Search => Some("search"),
                ComposeAction::Other => None,
            }
        }

        fn category_enum(&self) -> Self::Category {
            match self {
                ComposeAction::NavUp => ComposeCategory::Nav,
                ComposeAction::Search => ComposeCategory::Search,
                ComposeAction::Other => ComposeCategory::Uncategorized,
            }
        }
    }

    fn handle_nav(state: &mut usize, _action: ComposeAction) -> &'static str {
        *state += 1;
        "nav"
    }

    fn handle_command(state: &mut usize, _action: ComposeAction) -> &'static str {
        *state += 10;
        "command"
    }

    fn handle_search(state: &mut usize, _action: ComposeAction) -> &'static str {
        *state += 100;
        "search"
    }

    fn handle_default(state: &mut usize, _action: ComposeAction) -> &'static str {
        *state += 1000;
        "default"
    }

    fn composed_reducer(
        state: &mut usize,
        action: ComposeAction,
        context: ComposeContext,
    ) -> &'static str {
        crate::reducer_compose!(state, action, context, {
            category "nav" => handle_nav,
            context ComposeContext::Command => handle_command,
            ComposeAction::Search => handle_search,
            _ => handle_default,
        })
    }

    #[test]
    fn test_reducer_compose_routes_category() {
        let mut state = 0;
        let result = composed_reducer(&mut state, ComposeAction::NavUp, ComposeContext::Command);
        assert_eq!(result, "nav");
        assert_eq!(state, 1);
    }

    #[test]
    fn test_reducer_compose_routes_context() {
        let mut state = 0;
        let result = composed_reducer(&mut state, ComposeAction::Other, ComposeContext::Command);
        assert_eq!(result, "command");
        assert_eq!(state, 10);
    }

    #[test]
    fn test_reducer_compose_routes_pattern() {
        let mut state = 0;
        let result = composed_reducer(&mut state, ComposeAction::Search, ComposeContext::Default);
        assert_eq!(result, "search");
        assert_eq!(state, 100);
    }

    #[test]
    fn test_reducer_compose_routes_fallback() {
        let mut state = 0;
        let result = composed_reducer(&mut state, ComposeAction::Other, ComposeContext::Default);
        assert_eq!(result, "default");
        assert_eq!(state, 1000);
    }

    // Test 3-argument form (no context)
    fn composed_reducer_no_context(state: &mut usize, action: ComposeAction) -> &'static str {
        crate::reducer_compose!(state, action, {
            category "nav" => handle_nav,
            ComposeAction::Search => handle_search,
            _ => handle_default,
        })
    }

    #[test]
    fn test_reducer_compose_3arg_category() {
        let mut state = 0;
        let result = composed_reducer_no_context(&mut state, ComposeAction::NavUp);
        assert_eq!(result, "nav");
        assert_eq!(state, 1);
    }

    #[test]
    fn test_reducer_compose_3arg_pattern() {
        let mut state = 0;
        let result = composed_reducer_no_context(&mut state, ComposeAction::Search);
        assert_eq!(result, "search");
        assert_eq!(state, 100);
    }

    #[test]
    fn test_reducer_compose_3arg_fallback() {
        let mut state = 0;
        let result = composed_reducer_no_context(&mut state, ComposeAction::Other);
        assert_eq!(result, "default");
        assert_eq!(state, 1000);
    }
}
