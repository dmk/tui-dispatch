# Introduction

**tui-dispatch** is a centralized state management framework for Rust TUI applications, inspired by Redux and Elm.

## The Core Idea

State mutations happen in one place (reducers), making apps predictable and testable:

```
Event → Action → Reducer → State Change → Render
```

1. Terminal events (keypresses, mouse clicks) are converted to **actions**
2. Actions are dispatched to the **store**
3. The **reducer** updates state based on the action
4. The UI re-renders with the new state

## When to Use tui-dispatch

**Good fit:**
- Apps with shared state across multiple UI components
- Apps with async operations (API calls, file I/O)
- Apps where you want clear separation between UI and logic
- Apps that need debugging tools (action logging, state inspection)

**Overkill for:**
- Simple single-screen apps with minimal state
- Apps where state is naturally local to each widget

## Two Modes of Operation

### Simple Mode (bool reducer)

For apps without async side effects. The reducer returns `true` if state changed:

```rust
fn reducer(state: &mut AppState, action: AppAction) -> bool {
    match action {
        AppAction::Increment => { state.count += 1; true }
        AppAction::Quit => false,
    }
}
```

Use `DispatchRuntime` or `Store` directly. See the [Counter example](./examples/counter.md).

### Effect Mode (DispatchResult reducer)

For apps with async operations. The reducer returns state change status AND effects to execute:

```rust
fn reducer(state: &mut AppState, action: AppAction) -> DispatchResult<Effect> {
    match action {
        AppAction::Fetch => {
            state.is_loading = true;
            DispatchResult::changed_with(Effect::FetchData)
        }
        AppAction::DidLoad(data) => {
            state.data = Some(data);
            state.is_loading = false;
            DispatchResult::changed()
        }
    }
}
```

Use `EffectRuntime` or `EffectStore`. See the [Weather example](./examples/weather.md).

## Data Flow

```
Terminal Input
      │
      ▼
map_event(event) ──► Action
                        │
                        ▼
                 store.dispatch(action)
                        │
                        ▼
                 reducer(state, action)
                        │
                        ▼
            ┌───────────┴───────────┐
            │                       │
            ▼                       ▼
     state changed?          effects to run?
            │                       │
            ▼                       ▼
        render()              handle_effect()
                                    │
                                    ▼
                            spawn async task
                                    │
                                    ▼
                         action_tx.send(DidAction)
                                    │
                                    └──► back to dispatch
```

## Crate Structure

```
tui-dispatch/
├── tui-dispatch/        # Re-exports + prelude
├── tui-dispatch-core/   # Core: Store, EffectStore, Runtime, Component
└── tui-dispatch-macros/ # #[derive(Action)], #[derive(DebugState)]
```

Optional companion crate:
```
tui-dispatch-components/ # Reusable UI components: SelectList, TextInput, etc.
```

## Next Steps

- **New to tui-dispatch?** Start with [Getting Started](./getting-started.md)
- **Need terminology?** Check the [Glossary](./glossary.md)
- **Want to see code?** Browse the [Examples](./examples/README.md)

## Real-World Usage

tui-dispatch is used in production by [memtui](https://github.com/dmk/memtui), a TUI for Redis, Memcached, and etcd.
