---
title: Counter Example
description: The simplest possible tui-dispatch app
---

The simplest possible tui-dispatch app - a counter that you can increment and decrement.

## Run it

```bash
cargo run -p counter-example
```

## Keys

- `k` / `Up` - increment
- `j` / `Down` - decrement
- `q` / `Esc` - quit

## What it demonstrates

This ~120 line example shows the core pattern without any extensions:

1. **State** - A struct holding what the app knows
2. **Actions** - An enum describing what can happen
3. **Reducer** - A function that updates state based on actions
4. **Store** - Container that holds state and applies reducer
5. **Main loop** - Synchronous event polling, action dispatch, conditional render

No async runtime, no EventBus, no debug layer — just the essentials.

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
enum Action {
    Increment,
    Decrement,
    Quit,
}
```

### Reducer

```rust
fn reducer(state: &mut AppState, action: Action) -> bool {
    match action {
        Action::Increment => {
            state.count += 1;
            true  // state changed, need re-render
        }
        Action::Decrement => {
            state.count -= 1;
            true
        }
        Action::Quit => false,  // handled in main loop
    }
}
```

The reducer returns `bool` - true means state changed and UI should re-render.

### Store + Main Loop

```rust
let mut store = Store::new(AppState::default(), reducer);

loop {
    // Render
    terminal.draw(|frame| {
        // ... render UI using store.state() ...
    })?;

    // Handle input
    if let Event::Key(key) = event::read()? {
        let action = match key.code {
            KeyCode::Char('k') | KeyCode::Up => Action::Increment,
            KeyCode::Char('j') | KeyCode::Down => Action::Decrement,
            KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
            _ => continue,
        };

        if !store.dispatch(action) {
            break;
        }
    }
}
```

The loop renders, waits for a key event, maps it to an action, and dispatches. When `dispatch` returns `false` (the `Quit` arm), the loop exits.

## Next steps

- [Weather example](/tui-dispatch/examples/weather/) - adds async API calls, effects, EventBus, and debug overlay
- [Markdown Preview](/tui-dispatch/examples/markdown-preview/) - adds debug overlay and feature flags
