---
title: Reducers
description: Why reducers are function pointers and how to work with that constraint
---

## Why Function Pointers?

tui-dispatch defines reducers as bare function pointers:

```rust
pub type Reducer<S, A> = fn(&mut S, A) -> bool;
pub type EffectReducer<S, A, E> = fn(&mut S, A) -> ReducerResult<E>;
```

This is a deliberate choice, not an oversight. Unlike `Box<dyn FnMut>` or a generic `F: FnMut`, a function pointer:

- **Enforces purity** — reducers can't capture mutable state or side-effect through closures
- **Is zero-cost** — no heap allocation, no vtable, no dynamic dispatch
- **Keeps type signatures clean** — `Store<S, A>` instead of `Store<S, A, F>`

Every Redux/Elm-style framework makes this tradeoff. The reducer's job is to compute the next state from the current state and an action — nothing else.

## Handling Configuration and Environment

Since reducers can't capture environment, put any configuration or context into your state struct:

```rust
struct AppState {
    // Application state
    items: Vec<Item>,
    selected: usize,

    // Configuration that the reducer needs
    config: AppConfig,
    api_base_url: String,
    max_items: usize,
}

fn reducer(state: &mut AppState, action: Action) -> bool {
    match action {
        Action::AddItem(item) => {
            if state.items.len() < state.max_items {
                state.items.push(item);
                true
            } else {
                false
            }
        }
        // ...
    }
}
```

This is the correct pattern — if the reducer needs data to make decisions, that data belongs in the state.

## Dependency Injection

For external services (API clients, file handles), use the effect pattern instead of trying to inject them into the reducer:

```rust
enum Effect {
    FetchData { url: String },
    SaveFile { path: String, data: Vec<u8> },
}

fn reducer(state: &mut AppState, action: Action) -> ReducerResult<Effect> {
    match action {
        Action::Save => {
            let data = serialize(&state.document);
            ReducerResult::changed_with(Effect::SaveFile {
                path: state.save_path.clone(),
                data,
            })
        }
        Action::DidSave => {
            state.dirty = false;
            ReducerResult::changed()
        }
        // ...
    }
}
```

The reducer declares *what* should happen (via effects), and your effect handler does the actual I/O with whatever clients/services it needs.

## See Also

- [Reducer Composition](/tui-dispatch/patterns/reducer-composition/) — splitting large reducers with `reducer_compose!`
- [Async Patterns](/tui-dispatch/patterns/async/) — handling async work with effects
