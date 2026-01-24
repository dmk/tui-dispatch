---
title: Reducer Composition
description: Splitting large reducers into domain-specific handlers
---

As your app grows, a single reducer can become unwieldy. The `reducer_compose!` macro helps you split large reducers into domain-specific handlers while maintaining a single entry point.

## When to Use

**Good fit:**
- Reducers with 20+ action variants
- Clear domain boundaries (navigation, search, modals, etc.)
- Mode-aware apps where the same action behaves differently based on context

**Overkill for:**
- Small apps with few actions
- Actions that don't group naturally into domains

## Basic Syntax (3-Argument Form)

```rust
use tui_dispatch::reducer_compose;

fn reducer(state: &mut AppState, action: Action) -> bool {
    reducer_compose!(state, action, {
        category "nav" => handle_nav,
        category "search" => handle_search,
        Action::Quit => |s, _| { s.should_quit = true; true },
        _ => handle_default,
    })
}

fn handle_nav(state: &mut AppState, action: Action) -> bool {
    match action {
        Action::NavUp => { state.selected = state.selected.saturating_sub(1); true }
        Action::NavDown => { state.selected += 1; true }
        _ => false,
    }
}

fn handle_search(state: &mut AppState, action: Action) -> bool {
    match action {
        Action::SearchStart => { state.mode = Mode::Search; true }
        Action::SearchClear => { state.query.clear(); true }
        _ => false,
    }
}

fn handle_default(state: &mut AppState, action: Action) -> bool {
    false
}
```

## Arm Types

### Category Routing

Route actions by their category (requires `#[action(infer_categories)]`):

```rust
category "nav" => handle_nav,
category "search" => handle_search,
```

### Pattern Matching

Route specific action variants:

```rust
Action::Quit => handle_quit,
Action::Resize { width, height } => handle_resize,
Action::Input(c) if c.is_alphabetic() => handle_alpha_input,
```

### Fallback

The `_` arm is required and must be last:

```rust
_ => handle_default,
```

## Context-Aware Routing (4-Argument Form)

For mode-aware apps, pass a context value:

```rust
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Normal,
    Insert,
    Command,
}

fn reducer(state: &mut AppState, action: Action) -> bool {
    let mode = state.mode;

    reducer_compose!(state, action, mode, {
        context Mode::Insert => handle_insert_mode,
        context Mode::Command => handle_command_mode,
        category "nav" => handle_nav,
        _ => handle_normal,
    })
}
```

Context arms are checked before category arms.

## Category Inference

Enable automatic category inference on your action enum:

```rust
#[derive(Action, Clone, Debug)]
#[action(infer_categories)]
enum Action {
    // Category: "nav" (prefix before verb "Scroll")
    NavScrollUp,
    NavScrollDown,

    // Category: "search" (prefix before verb "Start")
    SearchStart,
    SearchClear,

    // Category: "async_result" (special handling for "Did" prefix)
    WeatherDidLoad(Data),
    WeatherDidError(String),

    // No category (no recognized verb)
    Quit,
    Tick,
}
```

### Recognized Verbs

Categories are inferred by finding these verbs in action names:

`Start`, `End`, `Open`, `Close`, `Submit`, `Confirm`, `Cancel`, `Next`, `Prev`, `Up`, `Down`, `Left`, `Right`, `Enter`, `Exit`, `Escape`, `Add`, `Remove`, `Clear`, `Update`, `Set`, `Get`, `Load`, `Save`, `Delete`, `Create`, `Fetch`, `Change`, `Resize`, `Error`, `Show`, `Hide`, `Enable`, `Disable`, `Toggle`, `Focus`, `Blur`, `Select`, `Move`, `Copy`, `Cycle`, `Reset`, `Scroll`

### Category Override

Override inference for specific variants:

```rust
#[derive(Action)]
#[action(infer_categories)]
enum Action {
    // Would infer "api", but we want "network"
    #[action(category = "network")]
    ApiFetch,

    // Exclude from categorization
    #[action(skip_category)]
    InternalTick,
}
```

## Handler Signature

All handlers must have the same signature:

```rust
fn handler(state: &mut S, action: A) -> R
```

Where `R` is your return type (`bool` or `DispatchResult<E>`).

## Complete Example

Here's a full example with multiple domains:

```rust
use tui_dispatch::prelude::*;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Browse,
    Search,
    Modal,
}

#[derive(Default)]
struct AppState {
    mode: Mode,
    selected: usize,
    items: Vec<String>,
    query: String,
    modal_open: bool,
}

#[derive(Action, Clone, Debug)]
#[action(infer_categories)]
enum Action {
    // Nav domain
    NavUp,
    NavDown,
    NavSelect,

    // Search domain
    SearchStart,
    SearchInput(char),
    SearchClear,
    SearchSubmit,

    // Modal domain
    ModalOpen,
    ModalClose,
    ModalConfirm,

    // Global
    Quit,
}

fn reducer(state: &mut AppState, action: Action) -> bool {
    let mode = state.mode;

    reducer_compose!(state, action, mode, {
        // Mode-specific handling first
        context Mode::Modal => handle_modal,
        context Mode::Search => handle_search,

        // Then category-based routing
        category "nav" => handle_nav,

        // Specific patterns
        Action::Quit => |s, _| { true },

        // Fallback
        _ => |_, _| false,
    })
}

fn handle_nav(state: &mut AppState, action: Action) -> bool {
    match action {
        Action::NavUp => {
            state.selected = state.selected.saturating_sub(1);
            true
        }
        Action::NavDown => {
            if state.selected < state.items.len().saturating_sub(1) {
                state.selected += 1;
            }
            true
        }
        Action::NavSelect => {
            // Handle selection
            true
        }
        _ => false,
    }
}

fn handle_search(state: &mut AppState, action: Action) -> bool {
    match action {
        Action::SearchInput(c) => {
            state.query.push(c);
            true
        }
        Action::SearchClear => {
            state.query.clear();
            state.mode = Mode::Browse;
            true
        }
        Action::SearchSubmit => {
            // Execute search
            state.mode = Mode::Browse;
            true
        }
        _ => false,
    }
}

fn handle_modal(state: &mut AppState, action: Action) -> bool {
    match action {
        Action::ModalClose => {
            state.modal_open = false;
            state.mode = Mode::Browse;
            true
        }
        Action::ModalConfirm => {
            // Handle confirmation
            state.modal_open = false;
            state.mode = Mode::Browse;
            true
        }
        _ => false, // Modal captures all input
    }
}
```

## With Effects

The macro works with `DispatchResult<E>` too:

```rust
fn reducer(state: &mut AppState, action: Action) -> DispatchResult<Effect> {
    reducer_compose!(state, action, {
        category "api" => handle_api,
        _ => handle_default,
    })
}

fn handle_api(state: &mut AppState, action: Action) -> DispatchResult<Effect> {
    match action {
        Action::ApiFetch => {
            state.loading = true;
            DispatchResult::changed_with(Effect::Fetch)
        }
        Action::ApiDidLoad(data) => {
            state.data = Some(data);
            state.loading = false;
            DispatchResult::changed()
        }
        _ => DispatchResult::unchanged(),
    }
}
```

## See Also

- [Core Concepts: Action Categories](/tui-dispatch/getting-started/core-concepts/#actioninfer_categories) - Category inference details
- [Async Patterns](/tui-dispatch/patterns/async/) - Effect-based reducers
