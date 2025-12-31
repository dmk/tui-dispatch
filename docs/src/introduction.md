# Introduction

**tui-dispatch** is a centralized state management framework for Rust TUI applications, inspired by Redux and Elm.

## The Core Idea

Components should be pure functions:
- **State → UI**: Render based on current state
- **Events → Actions**: Convert user input to state change requests

State mutations happen in one place (reducers), making apps predictable and testable.

## Architecture

```
Terminal
    │
    ▼
EventBus (poll + convert)
    │
    ▼ Event
Component::handle_event()
    │
    ▼ Vec<Action>
action_tx.send()
    │
    ├──────────────────────┐
    ▼                      ▼
Sync Handlers         Async Handlers
(state mutation)      (spawn task)
    │                      │
    ▼                      │ Did* action
State                      │
    │◀─────────────────────┘
    ▼
Component::render(state)
```

## Key Concepts

| Concept | Description |
|---------|-------------|
| **Action** | A description of a state change (e.g., `NextItem`, `DidLoadData`) |
| **Store** | Holds the application state and dispatches actions |
| **Reducer** | A pure function `(state, action) → state` that performs mutations |
| **Component** | Handles events and renders UI, but never mutates state directly |
| **EventBus** | Polls terminal events and routes them to components |

## Crate Structure

```
tui-dispatch/
├── tui-dispatch-core/   # Core traits: Action, Component, Event, EventBus
├── tui-dispatch-macros/ # #[derive(Action, ComponentId, BindingContext)]
└── tui-dispatch/        # Re-exports + prelude
```

## Real-World Usage

tui-dispatch is used in production by [memtui](https://github.com/dmk/memtui), a TUI for Redis, Memcached, and etcd.
