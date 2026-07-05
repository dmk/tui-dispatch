# Agent Notes

tui-dispatch is a centralized state management framework for Rust TUI apps, inspired by Redux/Elm.

**Read the [Philosophy doc](docs/src/content/docs/getting-started/philosophy.md) first** - it explains why tui-dispatch exists and the principles behind architectural decisions.

## Workspace Structure

- `tui-dispatch`: Re-export crate with prelude
- `tui-dispatch-core`: Core traits and types (Store, EventBus, Component, effects, tasks, keybindings, testing)
- `tui-dispatch-macros`: Derive macros (Action, ComponentId, BindingContext, DebugState, FeatureFlags)
- `tui-dispatch-components`: Pre-built UI components (modal, select list, text input, tree view, scroll view, status bar)
- `tui-dispatch-debug`: Debug overlay, session recording, snapshot/replay, debug CLI
- `tui-dispatch-shared`: Shared utilities
- `examples/counter`: Minimal Store-only example
- `examples/github-lookup`: Async API calls via the effects pattern
- `examples/log-viewer`: Pre-built components + debug tooling on a streaming input
- `examples/md-preview`: Sync extensions - keybindings, feature flags, debug overlay
- `examples/minesweeper`: Middleware cancel/inject patterns

## Core Architecture

```
Event → Component::handle_event() → Action → Store::dispatch() → reducer() → state mutation + effects
                                                                                ↓
                                              effect handler → ctx.tasks().spawn() → Did* action → back to store
```

**Key traits:**
- `Action`: State mutation descriptor (derive with `#[derive(Action)]`)
- `Component`: Pure UI with `handle_event() → Vec<Action>` and `render()`
- `Store`: State container with reducer pattern
- `EventBus`: Pub/sub for event routing with focus management

## Effects Pattern

Reducers stay sync: they mutate state and emit effects. Effect handlers spawn async
work that resolves to a `Did*` action, which flows back through the reducer:

```rust
// Reducer: mutate state, emit effect
Action::UserFetch(username) => {
    state.is_loading = true;
    ReducerResult::changed().with(Effect::FetchUser { username })
}
Action::UserDidLoad(user) => {
    state.user = Some(user);
    ReducerResult::changed()
}

// Effect handler: spawn async task that resolves to a Did* action
fn handle_effect(effect: Effect, ctx: &mut EffectContext<Action>) {
    match effect {
        Effect::FetchUser { username } => {
            ctx.tasks().spawn("user_fetch", async move {
                match api::fetch_user(&username).await {
                    Ok(user) => Action::UserDidLoad(user),
                    Err(e) => Action::UserDidError(e),
                }
            });
        }
    }
}
```

## After Meaningful Changes

Run the full verification suite before committing:

```bash
make verify
```

This runs: fmt-check, check, clippy, all tests (including the feature matrix), and doc build.

## Documentation

New features should ideally be documented in `docs/src/content/docs/` (Astro Starlight). Keep docs concise.
