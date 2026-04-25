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
fn reducer(state: &mut AppState, action: Action) -> ReducerResult {
    match action {
        Action::Increment => {
            state.count += 1;
            ReducerResult::changed()
        }
        Action::Decrement => {
            state.count -= 1;
            ReducerResult::changed()
        }
        Action::Quit => ReducerResult::unchanged(),
    }
}
```

The reducer returns `ReducerResult`; `changed()` means state changed and the UI should re-render.

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

        if matches!(&action, Action::Quit) {
            break;
        }
        store.dispatch(action);
    }
}
```

The loop renders, waits for a key event, maps it to an action, exits on `Quit`, and dispatches everything else.

## Next steps

- [GitHub Lookup](/tui-dispatch/examples/github-lookup/) - adds async API calls, effects, and TaskManager
- [Markdown Preview](/tui-dispatch/examples/markdown-preview/) - adds debug overlay and feature flags
- [dmk/tui-stuff](https://github.com/dmk/tui-stuff) - more complete apps built with tui-dispatch
