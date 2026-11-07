//! Optional dispatch observation, independent of rendering and serialization.

use crate::{Action, DispatchError};

/// Result at an individual action's dispatch boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DispatchOutcome {
    /// The reducer ran (including reducers reporting no state change).
    Applied,
    /// Middleware cancelled the action before reduction.
    Cancelled,
    /// Dispatch limits rejected the action before middleware or reduction.
    Rejected(DispatchError),
}

/// Metadata for one action, before synchronous middleware injections run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchObservation {
    /// Zero for a root action; increases for each level of middleware injection.
    pub depth: usize,
    /// Whether this action's reducer reported a change, excluding child actions.
    pub changed: bool,
    /// How this action was processed.
    pub outcome: DispatchOutcome,
}

/// A passive observer of dispatches on a store.
///
/// The callback receives state immediately after reduction (or unchanged state
/// for cancellation/rejection), before `Middleware::after` and child dispatches.
/// Panics propagate; observers must not panic. Reducer panics are not observed.
/// No callbacks are made for direct `state_mut` edits.
pub trait DispatchObserver<S, A: Action>: Send + Sync {
    /// Whether observation is active. Disabled observers need not clone actions.
    fn is_enabled(&self) -> bool {
        true
    }

    /// Inspect one dispatch boundary. State and action are borrowed, not cloned.
    fn observe(&mut self, action: &A, state: &S, observation: DispatchObservation);
}
