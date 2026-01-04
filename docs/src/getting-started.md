# Getting Started

## Installation

Add tui-dispatch to your `Cargo.toml`:

```toml
[dependencies]
tui-dispatch = "0.2"
ratatui = "0.29"
crossterm = "0.28"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time"] }
tokio-util = "0.7"
```

## Minimal Example

The counter example (~80 lines) shows the core pattern:

```bash
cargo run -p counter
```

The pattern is simple:

```
Event → Action → Store.dispatch() → reducer() → state change → render
```

### 1. State - What the app knows

```rust
#[derive(Default)]
struct AppState {
    count: i32,
}
```

### 2. Actions - What can happen

```rust
#[derive(Clone, Debug, Action)]
#[action(infer_categories)]
enum AppAction {
    CountIncrement,
    CountDecrement,
    Quit,
}
```

### 3. Reducer - How state changes

```rust
fn reducer(state: &mut AppState, action: AppAction) -> bool {
    match action {
        AppAction::CountIncrement => { state.count += 1; true }
        AppAction::CountDecrement => { state.count -= 1; true }
        AppAction::Quit => false,
    }
}
```

### 4. Store - Where state lives

```rust
let mut store = Store::new(AppState::default(), reducer);

// In event loop:
let state_changed = store.dispatch(action);
if state_changed { /* render */ }
```

### 5. Main loop - Event → Action → Dispatch → Render

```rust
// Map events to actions
if let EventKind::Key(key) = event {
    let action = match key.code {
        KeyCode::Char('k') | KeyCode::Up => Some(AppAction::CountIncrement),
        KeyCode::Char('j') | KeyCode::Down => Some(AppAction::CountDecrement),
        KeyCode::Char('q') => Some(AppAction::Quit),
        _ => None,
    };
    if let Some(a) = action {
        action_tx.send(a);
    }
}
```

## Action Categories

Use `#[action(infer_categories)]` to auto-categorize actions by prefix:

```rust
#[derive(Action, Clone, Debug)]
#[action(infer_categories)]
enum Action {
    // Category: "search"
    SearchStart,
    SearchAddChar(char),
    SearchClear,

    // Category: "async_result" (Did* prefix)
    DidConnect(String),
    DidLoadData(Vec<Data>),

    // Uncategorized
    Quit,
}
```

Generated methods:
- `action.name()` - variant name as string
- `action.category()` - inferred category
- `action.is_search()` - true for Search* variants
- `action.is_async_result()` - true for Did* variants

## Async Pattern

Split async work into intent + result actions:

```rust
// Intent action triggers async work
Action::DataFetch { id } => {
    let tx = action_tx.clone();
    tokio::spawn(async move {
        match api_call().await {
            Ok(data) => tx.send(Action::DidLoadData { id, data }),
            Err(e) => tx.send(Action::DidError { id, error: e.to_string() }),
        }
    });
}

// Result action updates state (in reducer)
Action::DidLoadData { id, data } => {
    state.data.insert(id, data);
    true
}
```

## Debug Mode

Add debug overlay with zero overhead when disabled:

```rust
// CLI flag
#[arg(long)]
debug: bool,

// Setup (only active when --debug passed)
let mut debug = DebugLayer::<Action>::new(KeyCode::F(12)).active(args.debug);

// In event loop - handles F12 toggle, overlays, etc.
if debug.intercepts(&event) {
    continue;
}

// In render
debug.render_state(frame, &state, |f, area| render_app(f, area, state));
```

## Next Steps

Check out the [examples](./examples/README.md):
- [Counter](./examples/counter.md) - minimal example (~80 lines)
- [Weather](./examples/weather.md) - async API calls, middleware
- [Markdown Preview](./examples/markdown-preview.md) - debug overlay, feature flags
