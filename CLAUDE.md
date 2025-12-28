# Agent Notes

tui-dispatch is a centralized state management framework for Rust TUI apps, inspired by Redux/Elm.

## Workspace Structure

- `tui-dispatch`: Re-export crate with prelude
- `tui-dispatch-core`: Core traits and types (Store, EventBus, Component, testing, debug)
- `tui-dispatch-macros`: Derive macros (Action, ComponentId, BindingContext)
- `examples/weather`: Full app demo with async API calls
- `examples/markdown-preview`: Debug overlay demo

## Core Architecture

```
Event → Component::handle_event() → Action → Store::dispatch() → reducer() → state mutation
                                      ↓
                                 async handler → tokio::spawn → Did* action → back to store
```

**Key traits:**
- `Action`: State mutation descriptor (derive with `#[derive(Action)]`)
- `Component`: Pure UI with `handle_event() → Vec<Action>` and `render()`
- `Store`: State container with reducer pattern
- `EventBus`: Pub/sub for event routing with focus management

## Async Handler Pattern

Split handlers into sync (immediate state changes) and async (spawn task → send `Did*` result):

```rust
// Intent action triggers async work
Action::DataFetch { id } => {
    tokio::spawn(async move {
        match api_call().await {
            Ok(data) => tx.send(Action::DataDidLoad { id, data }),
            Err(e) => tx.send(Action::DataDidError { id, error: e.to_string() }),
        }
    });
}
// Result action updates state in reducer
Action::DataDidLoad { id, data } => { state.data.insert(id, data); true }
```

## After Meaningful Changes

Run the full verification suite before committing:

```bash
make verify
```

This runs: fmt-check, check, clippy, and all tests.
