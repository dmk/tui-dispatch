# Counter Example

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

### Event loop

```rust
tokio::select! {
    Some(raw_event) = event_rx.recv() => {
        let event = process_raw_event(raw_event);

        // Map events to actions
        if let EventKind::Key(key) = event {
            let action = match key.code {
                KeyCode::Char('k') | KeyCode::Up => Some(AppAction::CountIncrement),
                KeyCode::Char('j') | KeyCode::Down => Some(AppAction::CountDecrement),
                KeyCode::Char('q') | KeyCode::Esc => Some(AppAction::Quit),
                _ => None,
            };
            if let Some(a) = action {
                action_tx.send(a);
            }
        }
    }

    Some(action) = action_rx.recv() => {
        if matches!(action, AppAction::Quit) {
            break;
        }
        should_render = store.dispatch(action);
    }
}
```

## Next steps

- [Weather example](./weather.md) - adds async API calls and middleware
- [Markdown Preview](./markdown-preview.md) - adds debug overlay and feature flags
