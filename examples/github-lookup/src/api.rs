//! GitHub API client
//!
//! This module handles the async HTTP requests to GitHub's API.
//! It's called from the effect handler, not from the reducer.

use serde::Deserialize;

use crate::state::GitHubUser;

/// Raw API response from GitHub
#[derive(Debug, Deserialize)]
struct GitHubApiResponse {
    login: String,
    name: Option<String>,
    bio: Option<String>,
    public_repos: u32,
    followers: u32,
    following: u32,
    avatar_url: String,
}

/// Fetch a GitHub user by username
///
/// # Returns
/// `Ok(GitHubUser)` on success, `Err(String)` with error message on failure.
pub async fn fetch_user(username: &str) -> Result<GitHubUser, String> {
    let url = format!("https://api.github.com/users/{}", username);

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("User-Agent", "tui-dispatch-example")
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(format!("User '{}' not found", username));
    }

    if !response.status().is_success() {
        return Err(format!("GitHub API error: {}", response.status()));
    }

    let data: GitHubApiResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(GitHubUser {
        login: data.login,
        name: data.name,
        bio: data.bio,
        public_repos: data.public_repos,
        followers: data.followers,
        following: data.following,
        avatar_url: data.avatar_url,
    })
}
