//! Actions - messages that describe state changes
//!
//! Naming convention:
//! - `UserFetch`: Intent action - user wants to fetch data
//! - `UserDidLoad`: Result action - async operation succeeded
//! - `UserDidError`: Result action - async operation failed

use crate::state::GitHubUser;

/// Application actions
#[derive(tui_dispatch::Action, Clone, Debug, PartialEq)]
pub enum Action {
    /// Update the search query text
    QueryChange(String),

    /// Intent: Request user data fetch (triggers async task)
    UserFetch(String),

    /// Result: User data loaded successfully
    UserDidLoad(GitHubUser),

    /// Result: User fetch failed
    UserDidError(String),

    /// Clear the current user and error
    Clear,

    /// Exit the application
    Quit,
}
