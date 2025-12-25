//! Action trait for type-safe state mutations

use std::fmt::Debug;

/// Marker trait for actions that can be dispatched to the store
///
/// Actions represent intents to change state. They should be:
/// - Clone: Actions may be logged, replayed, or sent to multiple handlers
/// - Debug: For debugging and logging
/// - Send + 'static: For async dispatch across threads
///
/// Use `#[derive(Action)]` from `tui-dispatch-macros` to auto-implement this trait.
pub trait Action: Clone + Debug + Send + 'static {
    /// Get the action name for logging and filtering
    fn name(&self) -> &'static str;
}
