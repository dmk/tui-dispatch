# Reducing Boilerplate and Framework Friction

This document captures high-leverage additions that keep app code focused on
logic instead of scaffolding, wiring, or repetitive framework patterns.

Examples below are proposed APIs that illustrate intent and are not implemented
yet.

## Immediate wins

### 1) Derive ActionSummary

Provide `#[derive(ActionSummary)]` plus per-variant attributes to generate
concise log output without hand-written `impl ActionSummary` or custom `Debug`.

Example (proposed):

```rust
#[derive(Clone, Debug, tui_dispatch::Action, tui_dispatch::ActionSummary)]
#[action(infer_categories)]
enum Action {
    #[summary(fmt = "DidLoad { bytes: {bytes} }")]
    DidLoad { bytes: usize },

    #[summary(truncate = 40)]
    Error(String),

    #[summary(skip)]
    Tick,
}
```

Outcome: action logging stays readable with zero boilerplate.

### 2) Extend DebugState + adopt derives

The `#[derive(DebugState)]` macro exists; expand it with additional attributes
and use it to replace hand-written `debug_sections()` implementations.

Example (proposed):

```rust
use tui_dispatch::debug::DebugState;

fn format_bytes(bytes: &usize) -> String {
    format!("{bytes}B")
}

fn is_empty<T>(items: &Vec<T>) -> bool {
    items.is_empty()
}

#[derive(DebugState)]
struct AppState {
    #[debug(section = "Connection")]
    connection: ConnectionState,

    #[debug(flatten, section = "UI")]
    ui: UiState,

    #[debug(with = "format_bytes")]
    cache_bytes: usize,

    #[debug(skip_if = "is_empty")]
    errors: Vec<String>,
}
```

Outcome: debug overlays stay rich without manual section building.

### 3) Action metadata attributes

Add optional per-variant metadata in the `Action` derive to control naming and
logging behavior without extra logic.

Example (proposed):

```rust
#[derive(Clone, Debug, tui_dispatch::Action)]
enum Action {
    #[action(name = "Resize", tags = "ui")]
    UiTerminalResize(u16, u16),

    #[action(skip_log)]
    Tick,
}
```

Outcome: logging and UI labeling are configured at the action definition.

### 4) Small runtime helpers for dispatch

Add helpers to remove common loops and guard boilerplate.

Example (proposed):

```rust
store.dispatch_all([Action::StartSearch, Action::OpenPalette]);
bus.emit_all(actions);
dispatch_if(&mut store, dirty, Action::Save);
```

Outcome: fewer hand-rolled loops and scattered guard clauses.

## Longer-term ideas

### 5) Declarative keymap + help generation

Create a macro or builder that defines key bindings once and produces event
matching, action dispatch, and help text.

Example (proposed):

```rust
let keymap = keymap! {
    "q" => Action::Quit,
    "?" => Action::ToggleHelp,
    "j" => Action::NextItem,
    "k" => Action::PrevItem,
    "ctrl+p" => Action::OpenPalette,
};

if let Some(action) = keymap.action_for(&event) {
    let _ = action_tx.send(action);
}

render_help(keymap.help_rows());
```

Outcome: no duplicated keybinding tables and no drift between behavior and docs.

### 6) Component/handler attribute macros

Optional macros to standardize component signatures and reduce wiring in
`handle_event` and `render`.

Example (proposed):

```rust
#[component]
impl SearchPanel {
    fn render(&self, frame: &mut Frame, area: Rect, props: &Props) {
        // render UI from props
    }

    #[handler]
    fn handle_event(&mut self, event: &Event, props: &Props) -> impl IntoIterator<Item = Action> {
        // map events to actions
        None
    }
}
```

Outcome: consistent component shape with less boilerplate.

## Adoption path

1) Start with `#[derive(ActionSummary)]` and `#[derive(DebugState)]` usage.
2) Add action metadata to drive logging and overlays.
3) Layer in keymap generation and component/handler macros.

Each step is additive and does not require breaking changes.
