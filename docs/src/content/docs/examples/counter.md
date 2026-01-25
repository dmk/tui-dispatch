---
title: Counter Example
description: The simplest possible tui-dispatch app
---

The simplest possible tui-dispatch app - a counter that you can increment and decrement.

## Run it

```bash
cargo run -p counter
```

## Keys

- `k` / `Up` - increment
- `j` / `Down` - decrement
- `q` / `Esc` - quit

## What it demonstrates

This ~80 line example shows the core pattern:

1. **State** - A struct holding what the app knows
2. **Actions** - An enum describing what can happen
3. **Reducer** - A function that updates state based on actions
4. **Store** - Container that holds state and applies reducer
5. **Main loop** - Event polling, action dispatch, conditional render

## Code walkthrough

### State

```rust
#[derive(Default)]
struct AppState {
    count: i32,
}
```

### Actions

```rust
#[derive(Clone, Debug, Action)]
#[action(infer_categories)]
enum AppAction {
    CountIncrement,
    CountDecrement,
    Quit,
}
```

The `#[action(infer_categories)]` attribute automatically groups actions by prefix:
- `CountIncrement` and `CountDecrement` both have category "count"

### Reducer

```rust
fn reducer(state: &mut AppState, action: AppAction) -> bool {
    match action {
        AppAction::CountIncrement => {
            state.count += 1;
            true  // state changed, need re-render
        }
        AppAction::CountDecrement => {
            state.count -= 1;
            true
        }
        AppAction::Quit => false,  // handled in main loop
    }
}
```

The reducer returns `bool` - true means state changed and UI should re-render.

### Store

```rust
let mut store = Store::new(AppState::default(), reducer);

// Later, dispatch actions:
let state_changed = store.dispatch(action);
```

### Event routing with SimpleEventBus

```rust
let mut runtime = DispatchRuntime::new(AppState::default(), reducer)
    .with_debug(DebugLayer::simple());
let mut bus: SimpleEventBus<AppState, AppAction, CounterComponentId> = SimpleEventBus::new();
let keybindings: Keybindings<DefaultBindingContext> = Keybindings::new();

bus.register(CounterComponentId::Counter, |event, _state| {
    match handle_event(&event.kind) {
        Some(action) => HandlerResponse::action(action),
        None => HandlerResponse::ignored(),
    }
});

runtime
    .run_with_bus(terminal, &mut bus, &keybindings, render, |action| {
        matches!(action, AppAction::Quit)
    })
    .await?;
```

The `SimpleEventBus` uses `DefaultBindingContext`, which avoids the need to define a custom context enum for simple apps.

## Next steps

- [Weather example](/tui-dispatch/examples/weather/) - adds async API calls and middleware
- [Markdown Preview](/tui-dispatch/examples/markdown-preview/) - adds debug overlay and feature flags
