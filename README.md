# tui-dispatch

Centralized state management for Rust TUI apps. Like Redux/Elm, but for terminals.

## The Pitch

Components should be pure functions: state → UI, events → actions. State mutations happen in one place (reducers), making apps predictable and testable.

```rust
use tui_dispatch::prelude::*;

#[derive(Action, Clone, Debug)]
#[action(infer_categories)]
enum Action {
    NextItem,
    PrevItem,
    Select(usize),
    DidLoadData(Vec<String>),  // async result
}

struct AppState {
    items: Vec<String>,
    selected: usize,
}

fn reducer(state: &mut AppState, action: &Action) -> bool {
    match action {
        Action::NextItem => {
            state.selected = (state.selected + 1) % state.items.len();
            true // needs render
        }
        Action::PrevItem => {
            state.selected = state.selected.saturating_sub(1);
            true
        }
        Action::Select(idx) => {
            state.selected = *idx;
            true
        }
        Action::DidLoadData(items) => {
            state.items = items.clone();
            true
        }
    }
}
```

## Derive Macros

### Action

```rust
#[derive(Action, Clone, Debug)]
#[action(infer_categories, generate_dispatcher)]
enum Action {
    // Category: "search" (inferred from prefix)
    SearchStart,
    SearchAddChar(char),
    SearchClear,

    // Category: "async_result" (inferred from Did* prefix)
    DidConnect(String),
    DidLoadKeys(Vec<Key>),

    // Uncategorized
    Quit,
    Render,
}

// Generated methods:
action.name()              // "SearchStart", "DidConnect", etc.
action.category()          // Some("search"), Some("async_result"), None
action.is_search()         // true for SearchStart, SearchAddChar, SearchClear
action.is_async_result()   // true for DidConnect, DidLoadKeys

// Generated trait (with generate_dispatcher):
trait ActionDispatcher {
    fn dispatch(&mut self, action: &Action) -> bool;
    fn dispatch_search(&mut self, action: &Action) -> bool;
    fn dispatch_async_result(&mut self, action: &Action) -> bool;
    // ...
}
```

### ComponentId

```rust
#[derive(ComponentId, Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum ComponentId {
    KeyList,
    ValueViewer,
    SearchInput,
    Modal,
}
```

### BindingContext

```rust
#[derive(BindingContext, Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum BindingContext {
    Default,
    Search,
    Modal,
    Help,
}
```

## Keybindings

Context-aware keybinding system with JSON configuration:

```rust
// Load from config
let keybindings = Keybindings::from_config(&config);

// Match key event to command in context
if let Some(cmd) = keybindings.get_command(key_event, BindingContext::Search) {
    match cmd.as_str() {
        "search.confirm" => vec![Action::SearchConfirm],
        "search.cancel" => vec![Action::SearchClear],
        _ => vec![],
    }
}

// Get keybinding for help display
keybindings.get_first_keybinding("quit", BindingContext::Default)
// Returns Some("q")
```

Config format (`keybindings.json`):
```json
{
  "global": {
    "quit": ["q", "ctrl+c"],
    "help": ["?"]
  },
  "search": {
    "search.confirm": ["enter"],
    "search.cancel": ["esc"]
  }
}
```

## EventBus

Polls terminal events and converts to typed events with context:

```rust
let (tx, mut rx) = mpsc::unbounded_channel();
let event_bus = EventBus::new(tx);

// Spawn poller
spawn_event_poller(raw_tx, poll_timeout, loop_sleep, cancel_token);

// Process events
while let Some(raw) = raw_rx.recv().await {
    let event_kind = process_raw_event(raw);
    let event = event_bus.create_event(event_kind);

    // Route to component
    let actions = component.handle_event(&event, props);
    for action in actions {
        action_tx.send(action);
    }
}
```

## Component Pattern

Components handle events and render, but never mutate external state:

```rust
struct ItemList;

impl Component for ItemList {
    type Props<'a> = (&'a [String], usize);

    fn handle_event(&mut self, event: &Event, _props: Self::Props<'_>) -> Vec<Action> {
        match &event.kind {
            EventKind::Key(k) if k.code == KeyCode::Down => vec![Action::NextItem],
            EventKind::Key(k) if k.code == KeyCode::Up => vec![Action::PrevItem],
            _ => vec![],
        }
    }

    fn render(&mut self, f: &mut Frame, area: Rect, (items, selected): Self::Props<'_>) {
        // ratatui widgets here
    }
}
```

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

## Crate Structure

```
tui-dispatch/
├── tui-dispatch-core/   # Core traits: Action, Component, Event, EventBus
├── tui-dispatch-macros/ # #[derive(Action, ComponentId, BindingContext)]
└── tui-dispatch/        # Re-exports + prelude
```

## Real-World Usage

Used in production by [memtui](https://github.com/dmk/memtui), a TUI for Redis/Memcached/etcd.

## Status

| Feature | Status |
|---------|--------|
| Action trait + derive | ✅ |
| Category inference | ✅ |
| ActionDispatcher generation | ✅ |
| ComponentId derive | ✅ |
| BindingContext derive | ✅ |
| EventBus + polling | ✅ |
| Keybindings system | ✅ |
| Component trait | ✅ |
| Store abstraction | 🔜 |
| Debug overlay | 🔜 |

## vs tui-realm

| Aspect | tui-realm | tui-dispatch |
|--------|-----------|--------------|
| State | Distributed (component-owned) | Centralized |
| Components | Stateful actors | Event → Action mappers |
| Mutations | `perform(&mut self)` | Reducers only |
| Testing | MockComponent | Dispatch action, assert state |
| Concepts | ~15 | ~5 |

## License

MIT
