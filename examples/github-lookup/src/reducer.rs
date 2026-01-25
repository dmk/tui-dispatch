//! Reducer - pure function: (state, action) -> DispatchResult
//!
//! The reducer is the single place where state mutations happen.
//! It returns a DispatchResult indicating:
//! - Whether the state changed (triggers re-render)
//! - Any effects to execute (async operations)

use tui_dispatch::DispatchResult;

use crate::action::Action;
use crate::effect::Effect;
use crate::state::AppState;

/// The reducer handles all state transitions
pub fn reducer(state: &mut AppState, action: Action) -> DispatchResult<Effect> {
    match action {
        Action::QueryChange(query) => {
            state.query = query;
            DispatchResult::changed()
        }

        Action::UserFetch(username) => {
            let username = username.trim().to_string();
            if username.is_empty() {
                return DispatchResult::unchanged();
            }

            // Set loading state and emit fetch effect
            state.is_loading = true;
            state.error = None;
            state.user = None;
            DispatchResult::changed_with(Effect::FetchUser { username })
        }

        Action::UserDidLoad(user) => {
            state.user = Some(user);
            state.is_loading = false;
            state.error = None;
            DispatchResult::changed()
        }

        Action::UserDidError(msg) => {
            state.is_loading = false;
            state.error = Some(msg);
            state.user = None;
            DispatchResult::changed()
        }

        Action::Clear => {
            state.query.clear();
            state.user = None;
            state.error = None;
            state.is_loading = false;
            DispatchResult::changed()
        }

        Action::Quit => {
            // Quit is handled in main loop
            DispatchResult::unchanged()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::GitHubUser;

    fn mock_user() -> GitHubUser {
        GitHubUser {
            login: "octocat".into(),
            name: Some("The Octocat".into()),
            bio: Some("GitHub mascot".into()),
            public_repos: 8,
            followers: 1000,
            following: 9,
            avatar_url: "https://example.com/avatar.png".into(),
        }
    }

    #[test]
    fn test_user_fetch_sets_loading_and_emits_effect() {
        let mut state = AppState::default();

        let result = reducer(&mut state, Action::UserFetch("octocat".into()));

        assert!(result.changed);
        assert!(state.is_loading);
        assert!(state.error.is_none());
        assert_eq!(result.effects.len(), 1);
        assert!(matches!(
            &result.effects[0],
            Effect::FetchUser { username } if username == "octocat"
        ));
    }

    #[test]
    fn test_empty_username_does_not_fetch() {
        let mut state = AppState::default();

        let result = reducer(&mut state, Action::UserFetch("  ".into()));

        assert!(!result.changed);
        assert!(!state.is_loading);
        assert!(result.effects.is_empty());
    }

    #[test]
    fn test_user_did_load_updates_state() {
        let mut state = AppState {
            is_loading: true,
            ..Default::default()
        };

        let result = reducer(&mut state, Action::UserDidLoad(mock_user()));

        assert!(result.changed);
        assert!(!state.is_loading);
        assert!(state.user.is_some());
        assert_eq!(state.user.as_ref().unwrap().login, "octocat");
    }

    #[test]
    fn test_user_did_error_sets_error() {
        let mut state = AppState {
            is_loading: true,
            ..Default::default()
        };

        let result = reducer(&mut state, Action::UserDidError("Not found".into()));

        assert!(result.changed);
        assert!(!state.is_loading);
        assert!(state.user.is_none());
        assert_eq!(state.error, Some("Not found".into()));
    }

    #[test]
    fn test_clear_resets_state() {
        let mut state = AppState {
            query: "octocat".into(),
            user: Some(mock_user()),
            error: Some("old error".into()),
            is_loading: true,
        };

        let result = reducer(&mut state, Action::Clear);

        assert!(result.changed);
        assert!(state.query.is_empty());
        assert!(state.user.is_none());
        assert!(state.error.is_none());
        assert!(!state.is_loading);
    }
}
