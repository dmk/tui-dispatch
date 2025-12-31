# Getting Started

## Installation

Add tui-dispatch to your `Cargo.toml`:

```toml
[dependencies]
tui-dispatch = "0.1"
ratatui = "0.29"
crossterm = "0.28"
tokio = { version = "1", features = ["full"] }
```

## Quick Example

Here's a minimal example showing the core pattern:

```rust
use tui_dispatch::prelude::*;

// 1. Define your actions
#[derive(Action, Clone, Debug)]
#[action(infer_categories)]
enum Action {
    NextItem,
    PrevItem,
    Select(usize),
    DidLoadData(Vec<String>),  // async result
}

// 2. Define your state
struct AppState {
    items: Vec<String>,
    selected: usize,
}

// 3. Write a reducer
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

## Core Concepts

### Actions

Actions describe state changes. Use the `#[derive(Action)]` macro with `#[action(infer_categories)]` to automatically categorize actions by their prefix:

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
}
```

This generates:
- `action.name()` - returns the variant name
- `action.category()` - returns the inferred category
- `action.is_search()` - returns true for Search* variants
- `action.is_async_result()` - returns true for Did* variants

### Components

Components handle events and render UI, but never mutate state directly:

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
        // ratatui rendering here
    }
}
```

### Async Pattern

For async operations, split into intent and result actions:

```rust
// In your action handler:
match action {
    Action::DataFetch { id } => {
        let tx = action_tx.clone();
        tokio::spawn(async move {
            match api_call().await {
                Ok(data) => tx.send(Action::DidLoadData { id, data }),
                Err(e) => tx.send(Action::DidError { id, error: e.to_string() }),
            }
        });
    }
    // Result actions update state in reducer
    Action::DidLoadData { id, data } => {
        state.data.insert(id, data);
        true
    }
    // ...
}
```

## Next Steps

Check out the [examples](./examples/README.md) to see full working applications:
- [Weather](./examples/weather.md) - async API calls and basic UI
- [Markdown Preview](./examples/markdown-preview.md) - debug overlay and advanced features
