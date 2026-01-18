# Getting Started

## Installation

Add tui-dispatch to your `Cargo.toml`:

```toml
[dependencies]
tui-dispatch = "0.2"
ratatui = "0.29"
crossterm = "0.28"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time"] }
```

## Choose Your Pattern

| Need async operations? | Pattern | Example |
|------------------------|---------|---------|
| No | Simple (`bool` reducer) | [Counter](./examples/counter.md) |
| Yes | Effect (`DispatchResult` reducer) | [Weather](./examples/weather.md) |

## Simple Pattern (No Async)

Best for apps without API calls or background tasks.

### 1. Define State

```rust
#[derive(Default)]
struct AppState {
    count: i32,
}
```

### 2. Define Actions

```rust
use tui_dispatch::prelude::*;

#[derive(Clone, Debug, Action)]
enum AppAction {
    Increment,
    Decrement,
    Quit,
}
```

### 3. Write Reducer

The reducer returns `true` if state changed (triggering a re-render):

```rust
fn reducer(state: &mut AppState, action: AppAction) -> bool {
    match action {
        AppAction::Increment => { state.count += 1; true }
        AppAction::Decrement => { state.count -= 1; true }
        AppAction::Quit => false, // returning false won't trigger render
    }
}
```

### 4. Map Events to Actions

```rust
fn map_event(event: &Event, _state: &AppState) -> EventOutcome<AppAction> {
    if let EventKind::Key(key) = &event.kind {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => EventOutcome::Action(AppAction::Increment),
            KeyCode::Down | KeyCode::Char('j') => EventOutcome::Action(AppAction::Decrement),
            KeyCode::Char('q') => EventOutcome::Action(AppAction::Quit),
            _ => EventOutcome::Ignore,
        }
    } else {
        EventOutcome::Ignore
    }
}
```

### 5. Run with DispatchRuntime

```rust
use tui_dispatch::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    let terminal = ratatui::init();

    DispatchRuntime::new(AppState::default(), reducer)
        .run(
            terminal,
            render_app,
            map_event,
            |action| matches!(action, AppAction::Quit),
        )
        .await?;

    ratatui::restore();
    Ok(())
}

fn render_app(frame: &mut Frame, area: Rect, state: &AppState, _ctx: RenderContext) {
    let text = format!("Count: {}", state.count);
    frame.render_widget(Paragraph::new(text).centered(), area);
}
```

## Effect Pattern (With Async)

Best for apps with API calls, file I/O, or other side effects.

### 1. Define Effects

```rust
enum Effect {
    FetchData,
    SaveFile(PathBuf),
}
```

### 2. Write Effect Reducer

Returns `DispatchResult<Effect>` instead of `bool`:

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
        AppAction::DidError(err) => {
            state.error = Some(err);
            state.is_loading = false;
            DispatchResult::changed()
        }
        _ => DispatchResult::unchanged(),
    }
}
```

### 3. Handle Effects

```rust
async fn handle_effect(
    effect: Effect,
    ctx: EffectContext<'_, AppAction>,
) {
    match effect {
        Effect::FetchData => {
            let tx = ctx.action_tx().clone();
            tokio::spawn(async move {
                match api::fetch().await {
                    Ok(data) => tx.send(AppAction::DidLoad(data)).ok(),
                    Err(e) => tx.send(AppAction::DidError(e.to_string())).ok(),
                };
            });
        }
        Effect::SaveFile(path) => {
            // sync effect - no need to spawn
            std::fs::write(&path, "data").ok();
        }
    }
}
```

### 4. Run with EffectRuntime

```rust
EffectRuntime::new(AppState::default(), reducer)
    .run(
        terminal,
        render_app,
        map_event,
        |action| matches!(action, AppAction::Quit),
        handle_effect,
    )
    .await?;
```

## DispatchResult Methods

| Method | State Changed | Effects |
|--------|---------------|---------|
| `unchanged()` | No | None |
| `changed()` | Yes | None |
| `effect(e)` | No | One |
| `changed_with(e)` | Yes | One |
| `changed_with_many(v)` | Yes | Multiple |

## Adding Debug Mode

Add `--debug` CLI flag support:

```rust
use tui_dispatch::debug::DebugLayer;

DispatchRuntime::new(AppState::default(), reducer)
    .with_debug(DebugLayer::simple().active(args.debug))
    .run(...)
```

Press `F12` to toggle debug overlay when active.

## Action Categories (Optional)

Use `#[action(infer_categories)]` to auto-group actions by name prefix:

```rust
#[derive(Clone, Debug, Action)]
#[action(infer_categories)]
enum AppAction {
    SearchStart,      // category: "search"
    SearchClear,      // category: "search"
    DidLoadData,      // category: "async_result"
    Quit,             // no category
}
```

This enables:
- `action.category()` - returns `Option<&str>`
- `action.is_search()` - returns `true` for `Search*` variants
- `reducer_compose!` macro for routing by category

**When to use:** Large apps with many actions that benefit from grouped handling. Skip for simple apps.

## Testing

Use test harnesses for integrated testing:

```rust
use tui_dispatch::testing::{StoreTestHarness, EffectStoreTestHarness};

// Simple reducer
let mut harness = StoreTestHarness::new(AppState::default(), reducer);
harness.dispatch(AppAction::Increment);
assert_eq!(harness.state().count, 1);

// Effect reducer
let mut harness = EffectStoreTestHarness::new(AppState::default(), reducer);
harness.dispatch(AppAction::Fetch);
assert!(harness.state().is_loading);
assert_eq!(harness.drain_effects().len(), 1);
```

## Next Steps

- [Async Patterns](./async.md) - TaskManager, Subscriptions, debouncing
- [Pre-built Components](./components.md) - SelectList, TextInput, Modal
- [Debug Layer](./debug-layer.md) - State inspection, action logging
- [Examples](./examples/README.md) - Full working apps
