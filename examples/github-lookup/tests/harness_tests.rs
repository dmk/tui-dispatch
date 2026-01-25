//! Tests using EffectStoreTestHarness
//!
//! These tests demonstrate how to test the async/effects pattern
//! without actually making HTTP requests.

use tui_dispatch::testing::*;

use github_lookup::{
    action::Action,
    effect::Effect,
    reducer::reducer,
    state::{AppState, GitHubUser},
};

/// Helper to create mock user data
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
fn test_user_fetch_flow() {
    let mut harness = EffectStoreTestHarness::new(AppState::default(), reducer);

    // Update query
    harness.dispatch_collect(Action::QueryChange("octocat".into()));
    harness.assert_state(|s| s.query == "octocat");

    // Trigger fetch - should set loading and emit effect
    harness.dispatch_collect(Action::UserFetch("octocat".into()));
    harness.assert_state(|s| s.is_loading);
    harness.assert_state(|s| s.user.is_none());

    // Verify effect was emitted
    let effects = harness.drain_effects();
    effects.effects_count(1);
    effects.effects_first_matches(
        |e| matches!(e, Effect::FetchUser { username } if username == "octocat"),
    );
}

#[test]
fn test_user_load_success() {
    let mut harness = EffectStoreTestHarness::new(AppState::default(), reducer);

    // Start fetch
    harness.dispatch_collect(Action::UserFetch("octocat".into()));
    harness.assert_state(|s| s.is_loading);

    // Simulate async completion
    harness.complete_action(Action::UserDidLoad(mock_user()));
    let (changed, total) = harness.process_emitted();

    assert_eq!(total, 1);
    assert_eq!(changed, 1);

    harness.assert_state(|s| !s.is_loading);
    harness.assert_state(|s| s.user.is_some());
    harness.assert_state(|s| s.user.as_ref().unwrap().login == "octocat");
    harness.assert_state(|s| s.error.is_none());
}

#[test]
fn test_user_load_error() {
    let mut harness = EffectStoreTestHarness::new(AppState::default(), reducer);

    // Start fetch
    harness.dispatch_collect(Action::UserFetch("nonexistent".into()));

    // Simulate error
    harness.complete_action(Action::UserDidError("User 'nonexistent' not found".into()));
    harness.process_emitted();

    harness.assert_state(|s| !s.is_loading);
    harness.assert_state(|s| s.user.is_none());
    harness.assert_state(|s| s.error.is_some());
    harness.assert_state(|s| s.error.as_ref().unwrap().contains("not found"));
}

#[test]
fn test_clear_resets_state() {
    let mut harness = EffectStoreTestHarness::new(AppState::default(), reducer);

    // Load a user first
    harness.dispatch_collect(Action::UserFetch("octocat".into()));
    harness.complete_action(Action::UserDidLoad(mock_user()));
    harness.process_emitted();

    harness.assert_state(|s| s.user.is_some());

    // Clear
    harness.dispatch_collect(Action::Clear);

    harness.assert_state(|s| s.query.is_empty());
    harness.assert_state(|s| s.user.is_none());
    harness.assert_state(|s| s.error.is_none());
}

#[test]
fn test_empty_username_not_fetched() {
    let mut harness = EffectStoreTestHarness::new(AppState::default(), reducer);

    // Try to fetch empty username
    harness.dispatch_collect(Action::UserFetch("   ".into()));

    // Should not be loading and no effects
    harness.assert_state(|s| !s.is_loading);
    let effects = harness.drain_effects();
    effects.effects_empty();
}

#[test]
fn test_effect_assertions() {
    let mut harness = EffectStoreTestHarness::new(AppState::default(), reducer);

    // Initially no effects
    let effects = harness.drain_effects();
    effects.effects_empty();

    // After fetch, should have exactly one effect
    harness.dispatch_collect(Action::UserFetch("rust-lang".into()));

    let effects = harness.drain_effects();
    effects.effects_not_empty();
    effects.effects_count(1);
    effects.effects_all_match(|e| matches!(e, Effect::FetchUser { .. }));
}
