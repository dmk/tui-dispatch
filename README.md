# tui-dispatch

Centralized state management for Rust TUI apps. Like Redux/Elm, but for terminals.

## The Pitch

Components are pure: state → UI, events → actions. State mutations happen in reducers, making apps predictable and testable.

```rust
use tui_dispatch::prelude::*;

#[derive(Action, Clone, Debug)]
#[action(infer_categories)]
enum Action {
    NextItem,
    PrevItem,
    DidLoadData(Vec<String>),  // async result
}

fn reducer(state: &mut AppState, action: &Action) -> bool {
    match action {
        Action::NextItem => { state.selected += 1; true }
        Action::PrevItem => { state.selected -= 1; true }
        Action::DidLoadData(items) => { state.items = items.clone(); true }
    }
}
```

## Derive Macros

### Action

```rust
#[derive(Action, Clone, Debug)]
#[action(infer_categories)]
enum Action {
    SearchStart,           // category: "search"
    SearchClear,
    DidConnect(String),    // category: "async_result"
    Quit,                  // uncategorized
}

action.name()           // "SearchStart"
action.category()       // Some("search")
action.is_search()      // true
```

### DebugState

```rust
#[derive(DebugState)]
struct AppState {
    #[debug(section = "Connection")]
    host: String,
    port: u16,

    #[debug(section = "Data")]
    items: Vec<String>,

    #[debug(skip)]
    internal_cache: HashMap<String, Value>,
}
```

### FeatureFlags

```rust
#[derive(FeatureFlags, Default)]
struct Features {
    #[flag(default = true)]
    line_numbers: bool,
    wrap_lines: bool,
}

features.is_enabled("line_numbers")  // true
features.toggle("wrap_lines");
```

### ComponentId & BindingContext

```rust
#[derive(ComponentId, Clone, Copy, PartialEq, Eq, Hash)]
enum ComponentId { KeyList, ValueViewer, Modal }

#[derive(BindingContext, Clone, Copy, PartialEq, Eq, Hash)]
enum Context { Default, Search, Modal }
```

## Debug Layer

F12 to freeze UI and inspect state. One-line setup:

```rust
let mut debug: DebugLayer<Action, _> = DebugLayer::simple();

// In render loop
terminal.draw(|frame| {
    debug.render(frame, |f, area| {
        render_app(f, area, &state);
    });
})?;

// Handle F12
if key.code == KeyCode::F(12) {
    debug.handle_action(DebugAction::Toggle);
}
```

Debug mode keys: `S` state overlay, `A` action log, `Y` copy frame, `I` cell inspect.

### Action Logging

```rust
let middleware = ActionLoggerMiddleware::with_default_log();
let mut store = StoreWithMiddleware::new(state, reducer, middleware);

// In debug mode, show action history
if let Some(log) = store.middleware().log() {
    debug.show_action_log(log);
}
```

## Testing

```rust
#[test]
fn test_navigation() {
    let mut harness = TestHarness::new(AppState::default(), reducer);

    harness.send_keys("jjk");  // down, down, up
    harness.complete_actions();

    assert_eq!(harness.state().selected, 1);
    assert_emitted!(harness, Action::NextItem);
}
```

## Architecture

```
Terminal → EventBus → Component::handle_event() → Vec<Action>
                                                      │
              ┌───────────────────────────────────────┤
              ▼                                       ▼
        Sync Handler                           Async Handler
        (reducer)                              (spawn task)
              │                                       │
              ▼                                       │ Did* action
           State ◀────────────────────────────────────┘
              │
              ▼
        Component::render()
```

## Crate Structure

```
tui-dispatch/           # Re-exports + prelude
tui-dispatch-core/      # Store, EventBus, Component, Debug, Testing
tui-dispatch-macros/    # #[derive(Action, DebugState, FeatureFlags, ...)]
```

## Real-World Usage

Used in production by [memtui](https://github.com/dmk/memtui), a TUI for Redis/Memcached/etcd.

## License

MIT
