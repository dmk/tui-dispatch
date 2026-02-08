//! Effect-based state management
//!
//! This module provides an effect-aware store that allows reducers to emit
//! side effects alongside state changes. Effects are declarative descriptions
//! of work to be done, not the work itself.
//!
//! # Overview
//!
//! The traditional reducer returns `bool` (state changed or not):
//! ```text
//! fn reducer(state: &mut S, action: A) -> bool
//! ```
//!
//! An effect-aware reducer returns both change status and effects:
//! ```text
//! fn reducer(state: &mut S, action: A) -> ReducerResult<E>
//! ```
//!
//! # Example
//!
//! ```
//! use tui_dispatch_core::{Action, ReducerResult, EffectStore};
//!
//! // Define your effects
//! enum Effect {
//!     FetchData { url: String },
//!     CopyToClipboard(String),
//! }
//!
//! // Define state and actions
//! struct AppState { loading: bool, data: Option<String> }
//!
//! #[derive(Clone, Debug)]
//! enum AppAction { LoadData, DidLoadData(String) }
//!
//! impl Action for AppAction {
//!     fn name(&self) -> &'static str {
//!         match self {
//!             AppAction::LoadData => "LoadData",
//!             AppAction::DidLoadData(_) => "DidLoadData",
//!         }
//!     }
//! }
//!
//! // Reducer emits effects
//! fn reducer(state: &mut AppState, action: AppAction) -> ReducerResult<Effect> {
//!     match action {
//!         AppAction::LoadData => {
//!             state.loading = true;
//!             ReducerResult::changed_with(
//!                 Effect::FetchData { url: "https://api.example.com".into() }
//!             )
//!         }
//!         AppAction::DidLoadData(data) => {
//!             state.loading = false;
//!             state.data = Some(data);
//!             ReducerResult::changed()
//!         }
//!     }
//! }
//!
//! // Main loop handles effects
//! let mut store = EffectStore::new(
//!     AppState { loading: false, data: None },
//!     reducer,
//! );
//! let result = store.dispatch(AppAction::LoadData);
//! assert!(result.changed);
//!
//! for effect in result.effects {
//!     match effect {
//!         Effect::FetchData { url } => { /* spawn async task */ }
//!         _ => {}
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
pub struct ReducerResult<E> {
    /// Whether the state was modified by this action.
    pub changed: bool,
    /// Effects to be processed after dispatch.
    pub effects: Vec<E>,
}

impl<E> Default for ReducerResult<E> {
    fn default() -> Self {
        Self::unchanged()
    }
}

impl<E> ReducerResult<E> {
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
pub type EffectReducer<S, A, E> = fn(&mut S, A) -> ReducerResult<E>;

/// A store that supports effect-emitting reducers.
///
/// Similar to [`Store`](crate::Store), but the reducer returns
/// [`ReducerResult<E>`] instead of `bool`, allowing it to declare
/// side effects alongside state changes.
///
/// # Example
///
/// ```
/// use tui_dispatch_core::{Action, ReducerResult, EffectStore};
///
/// #[derive(Clone, Debug)]
/// enum MyAction { Increment }
///
/// impl Action for MyAction {
///     fn name(&self) -> &'static str { "Increment" }
/// }
///
/// enum Effect { Log(String) }
/// struct State { count: i32 }
///
/// fn reducer(state: &mut State, action: MyAction) -> ReducerResult<Effect> {
///     match action {
///         MyAction::Increment => {
///             state.count += 1;
///             ReducerResult::changed_with(Effect::Log(format!("count is {}", state.count)))
///         }
///     }
/// }
///
/// let mut store = EffectStore::new(State { count: 0 }, reducer);
/// let result = store.dispatch(MyAction::Increment);
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
    pub fn dispatch(&mut self, action: A) -> ReducerResult<E> {
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
/// use tui_dispatch::{ReducerResult, EffectStoreWithMiddleware};
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
    M: Middleware<S, A>,
{
    store: EffectStore<S, A, E>,
    middleware: M,
    dispatch_depth: usize,
}

impl<S, A, E, M> EffectStoreWithMiddleware<S, A, E, M>
where
    A: Action,
    M: Middleware<S, A>,
{
    /// Create a new effect store with middleware.
    pub fn new(state: S, reducer: EffectReducer<S, A, E>, middleware: M) -> Self {
        Self {
            store: EffectStore::new(state, reducer),
            middleware,
            dispatch_depth: 0,
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
    /// The action passes through `middleware.before()` (which can cancel it),
    /// then the reducer, then `middleware.after()` (which can inject follow-up actions).
    /// Injected actions go through the full pipeline recursively; their effects
    /// are merged into the returned result.
    pub fn dispatch(&mut self, action: A) -> ReducerResult<E> {
        use crate::store::MAX_DISPATCH_DEPTH;

        self.dispatch_depth += 1;
        assert!(
            self.dispatch_depth <= MAX_DISPATCH_DEPTH,
            "middleware dispatch depth exceeded {MAX_DISPATCH_DEPTH} — likely infinite injection loop"
        );

        if !self.middleware.before(&action, &self.store.state) {
            self.dispatch_depth -= 1;
            return ReducerResult::unchanged();
        }

        let mut result = self.store.dispatch(action.clone());
        let injected = self
            .middleware
            .after(&action, result.changed, &self.store.state);

        for a in injected {
            let sub = self.dispatch(a);
            result.changed |= sub.changed;
            result.effects.extend(sub.effects);
        }

        self.dispatch_depth -= 1;
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

    fn test_reducer(state: &mut TestState, action: TestAction) -> ReducerResult<TestEffect> {
        match action {
            TestAction::Increment => {
                state.count += 1;
                ReducerResult::changed()
            }
            TestAction::Decrement => {
                state.count -= 1;
                ReducerResult::changed_with(TestEffect::Log(format!("count: {}", state.count)))
            }
            TestAction::NoOp => ReducerResult::unchanged(),
            TestAction::TriggerEffect => {
                ReducerResult::effects(vec![TestEffect::Log("triggered".into()), TestEffect::Save])
            }
        }
    }

    #[test]
    fn test_dispatch_result_builders() {
        let r: ReducerResult<TestEffect> = ReducerResult::unchanged();
        assert!(!r.changed);
        assert!(r.effects.is_empty());

        let r: ReducerResult<TestEffect> = ReducerResult::changed();
        assert!(r.changed);
        assert!(r.effects.is_empty());

        let r = ReducerResult::effect(TestEffect::Save);
        assert!(!r.changed);
        assert_eq!(r.effects, vec![TestEffect::Save]);

        let r = ReducerResult::changed_with(TestEffect::Save);
        assert!(r.changed);
        assert_eq!(r.effects, vec![TestEffect::Save]);

        let r =
            ReducerResult::changed_with_many(vec![TestEffect::Save, TestEffect::Log("x".into())]);
        assert!(r.changed);
        assert_eq!(r.effects.len(), 2);
    }

    #[test]
    fn test_dispatch_result_chaining() {
        let r: ReducerResult<TestEffect> = ReducerResult::unchanged()
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
        let r: ReducerResult<TestEffect> = ReducerResult::unchanged();
        assert!(!r.has_effects());

        let r = ReducerResult::effect(TestEffect::Save);
        assert!(r.has_effects());
    }
}
