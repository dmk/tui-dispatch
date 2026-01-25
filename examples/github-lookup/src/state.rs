//! Application state - single source of truth

use serde::{Deserialize, Serialize};

/// GitHub user data from the API
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GitHubUser {
    pub login: String,
    pub name: Option<String>,
    pub bio: Option<String>,
    pub public_repos: u32,
    pub followers: u32,
    pub following: u32,
    pub avatar_url: String,
}

/// Application state
#[derive(Clone, Debug, Default)]
pub struct AppState {
    /// Current search query
    pub query: String,

    /// Loaded user data (None = not yet fetched)
    pub user: Option<GitHubUser>,

    /// Loading state for async operations
    pub is_loading: bool,

    /// Error message (if last fetch failed)
    pub error: Option<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }
}
