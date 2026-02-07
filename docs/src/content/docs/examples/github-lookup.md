---
title: GitHub Lookup
description: A GitHub user lookup TUI demonstrating the async/effects pattern
---

A GitHub user lookup TUI demonstrating the async/effects pattern.

**Demonstrates:**
- Effect-based async with `Did*` action pattern
- TaskManager for HTTP request cancellation
- Loading states and error handling
- Text input handling

## Running

```bash
cargo run -p github-lookup-example
```

Type a GitHub username and press Enter to fetch user info. Press Esc to clear, Ctrl+C to quit.

## Key Pattern

This example shows the recommended async flow:

1. User types username, presses Enter
2. `Action::UserFetch(username)` dispatched
3. Reducer sets `is_loading = true` and returns `Effect::FetchUser`
4. Effect handler spawns task via `ctx.tasks().spawn()`
5. Task completes and sends `Action::UserDidLoad` or `Action::UserDidError`
6. Reducer updates state, UI re-renders

```rust
// Intent action triggers async work
Action::UserFetch(username) => {
    state.is_loading = true;
    ReducerResult::changed_with(Effect::FetchUser { username })
}

// Result action updates state
Action::UserDidLoad(user) => {
    state.user = Some(user);
    state.is_loading = false;
    ReducerResult::changed()
}
```

This is the same pattern taught in the [async-fetch tutorial](/tui-dispatch/tutorials/async-fetch/), which walks through building this example from scratch.

## Source Structure

```
examples/github-lookup/
├── src/
│   ├── main.rs      # Terminal setup, EffectRuntime
│   ├── state.rs     # AppState, GitHubUser
│   ├── action.rs    # Action enum (intent + result)
│   ├── effect.rs    # Effect::FetchUser
│   ├── reducer.rs   # State mutations
│   ├── api.rs       # GitHub API client
│   └── ui.rs        # Rendering + key handling
└── Cargo.toml
```
