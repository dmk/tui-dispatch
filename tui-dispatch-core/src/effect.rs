//! Effect-based state management
//!
//! This module provides an effect-aware store that allows reducers to emit
//! side effects alongside state changes. Effects are declarative descriptions
//! of work to be done, not the work itself.
//!
//! # Overview
//!
//! The traditional reducer returns `bool` (state changed or not):
//! ```ignore
//! fn reducer(state: &mut S, action: A) -> bool
//! ```
//!
//! An effect-aware reducer returns both change status and effects:
//! ```ignore
//! fn reducer(state: &mut S, action: A) -> DispatchResult<E>
//! ```
//!
//! # Example
//!
//! ```ignore
//! use tui_dispatch::{Action, DispatchResult, EffectStore};
//!
//! // Define your effects
//! enum Effect {
//!     FetchData { url: String },
//!     SaveToFile { path: String, data: Vec<u8> },
//!     CopyToClipboard(String),
//! }
//!
//! // Define state and actions
//! struct AppState { loading: bool, data: Option<String> }
//!
//! #[derive(Clone, Debug, Action)]
//! enum AppAction {
//!     LoadData,
//!     DidLoadData(String),
//! }
//!
//! // Reducer emits effects
//! fn reducer(state: &mut AppState, action: AppAction) -> DispatchResult<Effect> {
//!     match action {
//!         AppAction::LoadData => {
//!             state.loading = true;
//!             DispatchResult::changed_with(vec![
//!                 Effect::FetchData { url: "https://api.example.com".into() }
//!             ])
//!         }
//!         AppAction::DidLoadData(data) => {
//!             state.loading = false;
//!             state.data = Some(data);
//!             DispatchResult::changed()
//!         }
//!     }
//! }
//!
//! // Main loop handles effects
//! let mut store = EffectStore::new(AppState::default(), reducer);
//! let result = store.dispatch(AppAction::LoadData);
//!
//! for effect in result.effects {
//!     match effect {
//!         Effect::FetchData { url } => {
//!             // spawn async task
//!         }
//!         // ...
//!     }
//! }
//! ```

use std::marker::PhantomData;

use crate::action::Action;
use crate::store::Middleware;

/// Result of dispatching an action to an effect-aware store.
///
/// Contains both the state change indicator and any effects to be processed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchResult<E> {
    /// Whether the state was modified by this action.
    pub changed: bool,
    /// Effects to be processed after dispatch.
    pub effects: Vec<E>,
}

impl<E> Default for DispatchResult<E> {
    fn default() -> Self {
        Self::unchanged()
    }
}

impl<E> DispatchResult<E> {
    /// Create a result indicating no state change and no effects.
    #[inline]
    pub fn unchanged() -> Self {
        Self {
            changed: false,
            effects: vec![],
        }
    }

    /// Create a result indicating state changed but no effects.
    #[inline]
    pub fn changed() -> Self {
        Self {
            changed: true,
            effects: vec![],
        }
    }

    /// Create a result with a single effect but no state change.
    #[inline]
    pub fn effect(effect: E) -> Self {
        Self {
            changed: false,
            effects: vec![effect],
        }
    }

    /// Create a result with multiple effects but no state change.
    #[inline]
    pub fn effects(effects: Vec<E>) -> Self {
        Self {
            changed: false,
            effects,
        }
    }

    /// Create a result indicating state changed with a single effect.
    #[inline]
    pub fn changed_with(effect: E) -> Self {
        Self {
            changed: true,
            effects: vec![effect],
        }
    }

    /// Create a result indicating state changed with multiple effects.
    #[inline]
    pub fn changed_with_many(effects: Vec<E>) -> Self {
        Self {
            changed: true,
            effects,
        }
    }

    /// Add an effect to this result.
    #[inline]
    pub fn with(mut self, effect: E) -> Self {
        self.effects.push(effect);
        self
    }

    /// Set the changed flag to true.
    #[inline]
    pub fn mark_changed(mut self) -> Self {
        self.changed = true;
        self
    }

    /// Returns true if there are any effects to process.
    #[inline]
    pub fn has_effects(&self) -> bool {
        !self.effects.is_empty()
    }
}

/// A reducer function that can emit effects.
///
/// Takes mutable state and an action, returns whether state changed
/// and any effects to process.
pub type EffectReducer<S, A, E> = fn(&mut S, A) -> DispatchResult<E>;

/// A store that supports effect-emitting reducers.
///
/// Similar to [`Store`](crate::Store), but the reducer returns
/// [`DispatchResult<E>`] instead of `bool`, allowing it to declare
/// side effects alongside state changes.
///
/// # Example
///
/// ```ignore
/// use tui_dispatch::{DispatchResult, EffectStore};
///
/// enum Effect { Log(String) }
/// struct State { count: i32 }
/// enum Action { Increment }
///
/// fn reducer(state: &mut State, action: Action) -> DispatchResult<Effect> {
///     match action {
///         Action::Increment => {
///             state.count += 1;
///             DispatchResult::changed_with(Effect::Log(format!("count is {}", state.count)))
///         }
///     }
/// }
///
/// let mut store = EffectStore::new(State { count: 0 }, reducer);
/// let result = store.dispatch(Action::Increment);
/// assert!(result.changed);
/// assert_eq!(result.effects.len(), 1);
/// ```
pub struct EffectStore<S, A, E> {
    state: S,
    reducer: EffectReducer<S, A, E>,
    _marker: PhantomData<(A, E)>,
}

impl<S, A, E> EffectStore<S, A, E>
where
    A: Action,
{
    /// Create a new effect store with the given initial state and reducer.
    pub fn new(state: S, reducer: EffectReducer<S, A, E>) -> Self {
        Self {
            state,
            reducer,
            _marker: PhantomData,
        }
    }

    /// Get a reference to the current state.
    #[inline]
    pub fn state(&self) -> &S {
        &self.state
    }

    /// Get a mutable reference to the state.
    ///
    /// Use sparingly - prefer dispatching actions for state changes.
    /// This is mainly useful for initialization.
    #[inline]
    pub fn state_mut(&mut self) -> &mut S {
        &mut self.state
    }

    /// Dispatch an action to the store.
    ///
    /// The reducer is called with the current state and action,
    /// returning whether state changed and any effects to process.
    #[inline]
    pub fn dispatch(&mut self, action: A) -> DispatchResult<E> {
        (self.reducer)(&mut self.state, action)
    }
}

/// An effect store with middleware support.
///
/// Wraps an [`EffectStore`] and calls middleware hooks before and after
/// each dispatch. The middleware receives action references and the
/// state change indicator, but not the effects.
///
/// # Example
///
/// ```ignore
/// use tui_dispatch::{DispatchResult, EffectStoreWithMiddleware};
/// use tui_dispatch::debug::ActionLoggerMiddleware;
///
/// let middleware = ActionLoggerMiddleware::with_default_log();
/// let mut store = EffectStoreWithMiddleware::new(
///     State::default(),
///     reducer,
///     middleware,
/// );
///
/// let result = store.dispatch(Action::Something);
/// // Middleware logged the action
/// // result.effects contains any effects to process
/// ```
pub struct EffectStoreWithMiddleware<S, A, E, M>
where
    A: Action,
    M: Middleware<A>,
{
    store: EffectStore<S, A, E>,
    middleware: M,
}

impl<S, A, E, M> EffectStoreWithMiddleware<S, A, E, M>
where
    A: Action,
    M: Middleware<A>,
{
    /// Create a new effect store with middleware.
    pub fn new(state: S, reducer: EffectReducer<S, A, E>, middleware: M) -> Self {
        Self {
            store: EffectStore::new(state, reducer),
            middleware,
        }
    }

    /// Get a reference to the current state.
    #[inline]
    pub fn state(&self) -> &S {
        self.store.state()
    }

    /// Get a mutable reference to the state.
    #[inline]
    pub fn state_mut(&mut self) -> &mut S {
        self.store.state_mut()
    }

    /// Get a reference to the middleware.
    #[inline]
    pub fn middleware(&self) -> &M {
        &self.middleware
    }

    /// Get a mutable reference to the middleware.
    #[inline]
    pub fn middleware_mut(&mut self) -> &mut M {
        &mut self.middleware
    }

    /// Dispatch an action through middleware and store.
    ///
    /// Calls `middleware.before()`, then `store.dispatch()`,
    /// then `middleware.after()` with the state change indicator.
    pub fn dispatch(&mut self, action: A) -> DispatchResult<E> {
        self.middleware.before(&action);
        let result = self.store.dispatch(action.clone());
        self.middleware.after(&action, result.changed);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug)]
    enum TestAction {
        Increment,
        Decrement,
        NoOp,
        TriggerEffect,
    }

    impl Action for TestAction {
        fn name(&self) -> &'static str {
            match self {
                TestAction::Increment => "Increment",
                TestAction::Decrement => "Decrement",
                TestAction::NoOp => "NoOp",
                TestAction::TriggerEffect => "TriggerEffect",
            }
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    enum TestEffect {
        Log(String),
        Save,
    }

    #[derive(Default)]
    struct TestState {
        count: i32,
    }

    fn test_reducer(state: &mut TestState, action: TestAction) -> DispatchResult<TestEffect> {
        match action {
            TestAction::Increment => {
                state.count += 1;
                DispatchResult::changed()
            }
            TestAction::Decrement => {
                state.count -= 1;
                DispatchResult::changed_with(TestEffect::Log(format!("count: {}", state.count)))
            }
            TestAction::NoOp => DispatchResult::unchanged(),
            TestAction::TriggerEffect => {
                DispatchResult::effects(vec![TestEffect::Log("triggered".into()), TestEffect::Save])
            }
        }
    }

    #[test]
    fn test_dispatch_result_builders() {
        let r: DispatchResult<TestEffect> = DispatchResult::unchanged();
        assert!(!r.changed);
        assert!(r.effects.is_empty());

        let r: DispatchResult<TestEffect> = DispatchResult::changed();
        assert!(r.changed);
        assert!(r.effects.is_empty());

        let r = DispatchResult::effect(TestEffect::Save);
        assert!(!r.changed);
        assert_eq!(r.effects, vec![TestEffect::Save]);

        let r = DispatchResult::changed_with(TestEffect::Save);
        assert!(r.changed);
        assert_eq!(r.effects, vec![TestEffect::Save]);

        let r =
            DispatchResult::changed_with_many(vec![TestEffect::Save, TestEffect::Log("x".into())]);
        assert!(r.changed);
        assert_eq!(r.effects.len(), 2);
    }

    #[test]
    fn test_dispatch_result_chaining() {
        let r: DispatchResult<TestEffect> = DispatchResult::unchanged()
            .with(TestEffect::Save)
            .mark_changed();
        assert!(r.changed);
        assert_eq!(r.effects, vec![TestEffect::Save]);
    }

    #[test]
    fn test_effect_store_basic() {
        let mut store = EffectStore::new(TestState::default(), test_reducer);

        assert_eq!(store.state().count, 0);

        let result = store.dispatch(TestAction::Increment);
        assert!(result.changed);
        assert!(result.effects.is_empty());
        assert_eq!(store.state().count, 1);

        let result = store.dispatch(TestAction::NoOp);
        assert!(!result.changed);
        assert_eq!(store.state().count, 1);
    }

    #[test]
    fn test_effect_store_with_effects() {
        let mut store = EffectStore::new(TestState::default(), test_reducer);

        let result = store.dispatch(TestAction::Decrement);
        assert!(result.changed);
        assert_eq!(result.effects.len(), 1);
        assert!(matches!(&result.effects[0], TestEffect::Log(s) if s == "count: -1"));

        let result = store.dispatch(TestAction::TriggerEffect);
        assert!(!result.changed);
        assert_eq!(result.effects.len(), 2);
    }

    #[test]
    fn test_effect_store_state_mut() {
        let mut store = EffectStore::new(TestState::default(), test_reducer);
        store.state_mut().count = 100;
        assert_eq!(store.state().count, 100);
    }

    #[test]
    fn test_has_effects() {
        let r: DispatchResult<TestEffect> = DispatchResult::unchanged();
        assert!(!r.has_effects());

        let r = DispatchResult::effect(TestEffect::Save);
        assert!(r.has_effects());
    }
}
