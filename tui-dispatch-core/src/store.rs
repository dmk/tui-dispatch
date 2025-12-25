//! Centralized state store with reducer pattern

use crate::Action;
use std::marker::PhantomData;

/// A reducer function that handles actions and mutates state
///
/// Returns `true` if the state changed and a re-render is needed.
pub type Reducer<S, A> = fn(&mut S, A) -> bool;

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
/// ```ignore
/// #[derive(Default)]
/// struct AppState {
///     counter: i32,
/// }
///
/// #[derive(Action, Clone, Debug)]
/// enum MyAction {
///     Increment,
///     Decrement,
/// }
///
/// fn reducer(state: &mut AppState, action: MyAction) -> bool {
///     match action {
///         MyAction::Increment => {
///             state.counter += 1;
///             true
///         }
///         MyAction::Decrement => {
///             state.counter -= 1;
///             true
///         }
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
pub struct StoreWithMiddleware<S, A: Action, M: Middleware<A>> {
    store: Store<S, A>,
    middleware: M,
}

impl<S, A: Action, M: Middleware<A>> StoreWithMiddleware<S, A, M> {
    /// Create a new store with middleware
    pub fn new(state: S, reducer: Reducer<S, A>, middleware: M) -> Self {
        Self {
            store: Store::new(state, reducer),
            middleware,
        }
    }

    /// Dispatch an action through middleware and store
    pub fn dispatch(&mut self, action: A) -> bool {
        self.middleware.before(&action);
        let changed = self.store.dispatch(action.clone());
        self.middleware.after(&action, changed);
        changed
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

/// Middleware trait for intercepting actions
///
/// Implement this trait to add logging, persistence, or other
/// cross-cutting concerns to your store.
pub trait Middleware<A: Action> {
    /// Called before the action is dispatched to the reducer
    fn before(&mut self, action: &A);

    /// Called after the action is processed by the reducer
    fn after(&mut self, action: &A, state_changed: bool);
}

/// A no-op middleware that does nothing
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopMiddleware;

impl<A: Action> Middleware<A> for NoopMiddleware {
    fn before(&mut self, _action: &A) {}
    fn after(&mut self, _action: &A, _state_changed: bool) {}
}

/// Middleware that logs actions (for debugging)
#[derive(Debug, Clone, Default)]
pub struct LoggingMiddleware {
    /// Whether to log before dispatch
    pub log_before: bool,
    /// Whether to log after dispatch
    pub log_after: bool,
}

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

impl<A: Action> Middleware<A> for LoggingMiddleware {
    fn before(&mut self, action: &A) {
        if self.log_before {
            tracing::debug!(action = %action.name(), "Dispatching action");
        }
    }

    fn after(&mut self, action: &A, state_changed: bool) {
        if self.log_after {
            tracing::debug!(
                action = %action.name(),
                state_changed = state_changed,
                "Action processed"
            );
        }
    }
}

/// Compose multiple middleware into a single middleware
pub struct ComposedMiddleware<A: Action> {
    middlewares: Vec<Box<dyn Middleware<A>>>,
}

impl<A: Action> std::fmt::Debug for ComposedMiddleware<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComposedMiddleware")
            .field("middlewares_count", &self.middlewares.len())
            .finish()
    }
}

impl<A: Action> Default for ComposedMiddleware<A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A: Action> ComposedMiddleware<A> {
    /// Create a new composed middleware
    pub fn new() -> Self {
        Self {
            middlewares: Vec::new(),
        }
    }

    /// Add a middleware to the composition
    pub fn add<M: Middleware<A> + 'static>(&mut self, middleware: M) {
        self.middlewares.push(Box::new(middleware));
    }
}

impl<A: Action> Middleware<A> for ComposedMiddleware<A> {
    fn before(&mut self, action: &A) {
        for middleware in &mut self.middlewares {
            middleware.before(action);
        }
    }

    fn after(&mut self, action: &A, state_changed: bool) {
        // Call in reverse order for proper nesting
        for middleware in self.middlewares.iter_mut().rev() {
            middleware.after(action, state_changed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    impl<A: Action> Middleware<A> for CountingMiddleware {
        fn before(&mut self, _action: &A) {
            self.before_count += 1;
        }

        fn after(&mut self, _action: &A, _state_changed: bool) {
            self.after_count += 1;
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
}
