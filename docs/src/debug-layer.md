# Debug Layer

The debug layer provides powerful debugging tools for TUI applications: frame freeze, state inspection, cell inspection, and clipboard export.

## Quick Start

Simple setup with a toggle key:

```rust
use tui_dispatch::debug::DebugLayer;
use crossterm::event::KeyCode;

// Create debug layer with F12 as toggle key
let mut debug = DebugLayer::<Action>::new(KeyCode::F(12));

// In event loop - handles toggle key, overlays, etc.
let outcome = debug.handle_event(&event.kind);
if outcome.consumed {
    for action in outcome.queued_actions {
        dispatch(action);
    }
    continue;
}

// In render loop:
debug.render_state(frame, &state, |f, area| {
    render_your_app(f, area, state);
});
```

Default keybindings (when debug mode is active):
- Toggle key (e.g., `F12`) - Toggle debug mode
- `S` - Show/hide state overlay
- `B` - Toggle banner position (top/bottom)
- `A` - Show/hide action log
- `J/K`, arrows, `PgUp/PgDn`, `g/G`, mouse wheel - Scroll tables
- `Y` - Copy frozen frame to clipboard
- `I` - Toggle mouse capture for cell inspection
- `Esc` / `Q` - Close overlay

## Custom Toggle Key

```rust
// Use F11 instead of F12
let debug = DebugLayer::<Action>::new(KeyCode::F(11));

// Use Escape key
let debug = DebugLayer::<Action>::new(KeyCode::Esc);
```

## State Inspection

Implement `DebugState` to show state in the debug overlay:

### Manual Implementation

```rust
use tui_dispatch::debug::{DebugState, DebugSection};

impl DebugState for AppState {
    fn debug_sections(&self) -> Vec<DebugSection> {
        vec![
            DebugSection::new("Connection")
                .entry("host", &self.host)
                .entry("port", self.port.to_string()),
            DebugSection::new("UI")
                .entry("scroll", self.scroll_offset.to_string()),
        ]
    }
}
```

### Derive Macro

Use `#[derive(DebugState)]` for automatic implementation:

```rust
use tui_dispatch::DebugState;

#[derive(DebugState)]
struct AppState {
    #[debug(section = "Connection")]
    host: String,
    #[debug(section = "Connection")]
    port: u16,

    #[debug(section = "UI")]
    scroll_offset: usize,

    #[debug(skip)]
    internal_cache: HashMap<String, Data>,
}
```

#### Attributes

| Attribute | Description |
|-----------|-------------|
| `#[debug(section = "Name")]` | Group field under a section |
| `#[debug(skip)]` | Exclude field from debug output |
| `#[debug(label = "Custom Label")]` | Custom label instead of field name |
| `#[debug(debug_fmt)]` | Use `{:?}` format instead of `Display` |
| `#[debug(format = "{:#?}")]` | Custom format string |

#### Example with All Attributes

```rust
#[derive(DebugState)]
struct ComplexState {
    #[debug(section = "Info", label = "Full Name")]
    name: String,

    #[debug(section = "Info")]
    count: usize,

    #[debug(section = "Status", debug_fmt)]
    level: ConnectionStatus,

    #[debug(skip)]
    cache: Vec<u8>,
}
```

## Showing the State Overlay

```rust
// Provide state data during render (recommended):
debug.render_state(frame, &app_state, |f, area| {
    render_your_app(f, area, &app_state);
});

// Or trigger it manually:
debug.show_state_overlay(&app_state);
```

## Cell Inspection

When mouse capture is enabled (`I` key), clicking on any cell shows its styling:

```rust
use tui_dispatch::debug::{inspect_cell, DebugTableBuilder};

if let Some(cell) = inspect_cell(&snapshot, x, y) {
    let overlay = DebugTableBuilder::new()
        .section("Cell Info")
        .entry("position", format!("({}, {})", x, y))
        .entry("symbol", format!("'{}'", cell.symbol))
        .entry("fg", format!("{:?}", cell.fg))
        .entry("bg", format!("{:?}", cell.bg))
        .cell_preview(cell)
        .finish_inspect("Cell Inspector");
    debug.freeze_mut().set_overlay(overlay);
}
```

## Full Control (Escape Hatch)

For custom layouts, use the lower-level methods:

```rust
// Split area manually
let (app_area, banner_area) = debug.split_area(frame.area());

// Custom layout
render_my_ui(frame, app_area);

// Let debug layer render its parts
debug.render_overlay(frame, app_area);
debug.render_banner(frame, banner_area);
```

## Handling Side Effects

The debug layer can produce side effects (e.g., clipboard copy):

```rust
use tui_dispatch::debug::{DebugAction, DebugSideEffect};

if let Some(effect) = debug.handle_action(DebugAction::CopyFrame) {
    match effect {
        DebugSideEffect::CopyToClipboard(text) => {
            // Copy to clipboard via OSC52 or system clipboard
        }
        DebugSideEffect::ProcessQueuedActions(actions) => {
            // Actions queued while frozen
        }
        _ => {}
    }
}
```
