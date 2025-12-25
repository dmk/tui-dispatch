# tui-dispatch

Centralized state management for Rust TUI apps. Like Redux/Elm, but for terminals.

## The Pitch

Components should be pure functions: state → UI, events → actions. State mutations happen in one place (reducers), making apps predictable and testable.

```rust
use tui_dispatch::prelude::*;

#[derive(Action, Clone, Debug)]
enum Action {
    NextItem,
    PrevItem,
    Select(usize),
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
    }
}
```

## Core Concepts

### Action

Actions represent intents to change state. Use `#[derive(Action)]` to auto-implement the trait:

```rust
#[derive(Action, Clone, Debug)]
enum MyAction {
    LoadData,           // action.name() returns "LoadData"
    DidLoadData(Data),  // action.name() returns "DidLoadData"
}
```

Convention: async operations use `Did*` prefix for their result actions.

### Component

Components handle events and render using ratatui widgets:

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
        // use ratatui widgets here
    }
}
```

Key rule: `handle_event` returns actions, never mutates external state.

### Events

Events wrap crossterm input with context about focus and component areas:

```rust
pub struct Event {
    pub kind: EventKind,      // Key, Mouse, Scroll, Resize, Tick
    pub context: EventContext,
}

pub struct EventContext {
    pub focused_component: Option<ComponentId>,
    pub mouse_position: Option<(u16, u16)>,
    pub component_areas: HashMap<ComponentId, Rect>,
    pub is_modal_open: bool,
    // ...
}
```

### Store (coming soon)

Centralized state with reducer pattern:

```rust
let store = Store::new(initial_state);
store.dispatch(action, reducer); // returns needs_render
```

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                      Terminal                           │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│   crossterm ──▶ EventBus ──▶ Component::handle_event   │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼ Vec<Action>
┌─────────────────────────────────────────────────────────┐
│                    Action Queue                         │
└─────────────────────────────────────────────────────────┘
          │                               │
          ▼                               ▼
┌─────────────────────┐     ┌─────────────────────────────┐
│   Sync Handlers     │     │      Async Handlers         │
│  (state mutations)  │     │   (spawn tasks, send back)  │
└─────────┬───────────┘     └──────────────┬──────────────┘
          │                                │
          ▼                                │
┌─────────────────────┐                    │
│       Store         │◀───────────────────┘
│  (central state)    │       DidXxx actions
└─────────┬───────────┘
          │
          ▼ state changed
┌─────────────────────────────────────────────────────────┐
│   Component::render(frame, area, props_from_state)     │
└─────────────────────────────────────────────────────────┘
```

## Crate Structure

```
tui-dispatch/
├── tui-dispatch-core/   # Core traits: Action, Component, Event
├── tui-dispatch-macros/ # #[derive(Action)]
└── tui-dispatch/        # Re-exports + prelude
```

## vs tui-realm

| Aspect | tui-realm | tui-dispatch |
|--------|-----------|--------------|
| State | Distributed (component-owned) | Centralized store |
| Components | Stateful actors with `perform()` | Event → Action, render with props |
| Mutations | `perform(&mut self)` | Reducers only |
| Testing | MockComponent | Dispatch action, assert state |
| Concepts | ~15 | ~5 |

## Status

Early development. Core traits are implemented, Store and EventBus coming next.

## License

MIT
