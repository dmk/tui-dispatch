//! Effects - side effects declared by the reducer
//!
//! Effects are returned from the reducer and handled by the main loop.
//! This keeps the reducer pure while making async operations explicit.

/// Side effects that can be triggered by actions
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Fetch user data for the given username
    FetchUser { username: String },
}
