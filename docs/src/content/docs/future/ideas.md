---
title: Ideas
description: Future directions and experiments for tui-dispatch
---

Future directions and experiments. Not committed to any of these.

---

## Centralized Theme System

Like keybindings, but for styling. Apps define themes once, components reference them.

```rust
// Define theme
#[derive(Theme)]
struct AppTheme {
    // Base colors
    fg: Color,
    bg: Color,
    accent: Color,

    // Semantic colors
    success: Color,
    warning: Color,
    error: Color,

    // Component-specific
    border: Style,
    selection: Style,
    highlight: Style,
}

impl Default for AppTheme {
    fn default() -> Self {
        Self {
            fg: Color::White,
            bg: Color::Rgb(20, 20, 30),
            accent: Color::Cyan,
            // ...
        }
    }
}

// Usage in components
fn render(&self, frame: &mut Frame, area: Rect, theme: &impl Theme) {
    let style = Style::default()
        .fg(theme.fg())
        .bg(theme.bg());
    // ...
}
```

Could provide preset themes: `Theme::dark()`, `Theme::light()`, `Theme::nord()`, etc.

---

## Additional Components (Planned)

**CmdLine** - Vim-style command input
```rust
// Modes: Command (:), Search (/), Goto (g), custom
let cmdline = CmdLine::new()
    .mode(CmdLineMode::Command)
    .prompt(":")
    .history(true);

// Emits: CmdLineAddChar, CmdLineConfirm, CmdLineHistoryPrev, etc.
```

**CommandPalette** - Searchable command list with suggestions
```rust
let commands = commands![
    "quit" | "q" => "Quit application",
    "help" | "h" | "?" => "Show help",
    "save" | "w" => "Save file",
];

let palette = CommandPalette::new(commands)
    .fuzzy(true)
    .max_visible(8);

// Emits: PaletteSelect(Command), PaletteClose, etc.
```

**Tabs** / **TabBar** - Tab navigation
```rust
let tabs = TabBar::new(&["Overview", "Details", "Settings"])
    .selected(0)
    .style(TabStyle::Underline);

// Emits: TabSelect(index), TabClose(index)
```

**Toast** / **Notification** - Transient messages
```rust
Toast::success("File saved!")
    .duration(Duration::from_secs(3))
    .position(ToastPosition::BottomRight);

// Auto-dismisses, or emits: ToastDismiss(id)
```

---

## Test Helper Improvements (Planned)

### Scenario Macro

Behavior-driven test syntax:

```rust
test_scenario! {
    name: search_workflow,
    state: AppState::default(),
    reducer: reducer,

    steps: [
        // Action dispatch
        dispatch SearchStart
            => state.search.active == true,

        // Key sequence
        press "f o o"
            => state.search.query == "foo",

        // Assert emitted actions
        press "enter"
            => emits SearchSubmit,

        // Combined
        press "n"
            => emits SearchNext
            => state.search.current_match == 1,
    ]
}
```

### Async Test Helpers

For testing async action flows:

```rust
#[tokio::test]
async fn test_async_flow() {
    let mut harness = AsyncTestHarness::new(state, reducer);

    harness.dispatch(Action::FetchData);

    // Simulate async completion
    harness.complete_async(Action::DidFetchData {
        data: mock_data()
    }).await;

    harness.assert_state(|s| s.data.is_some());
}
```

---

## Large Project Patterns

Ideas for keeping larger tui-dispatch apps organized and maintainable.

### State Organization Convention

Document the AppState/UiState split pattern:

```rust
// Domain state - what the app "knows"
struct AppState {
    keys: Vec<Key>,
    selected_key: Option<usize>,
    connection: Option<Connection>,
}

// UI state - how it's displayed
struct UiState {
    focus: ComponentId,
    scroll_offsets: HashMap<ComponentId, usize>,
    modal: Option<ModalKind>,
    search: SearchState,
}
```

Benefits:
- Clear ownership (backend touches AppState, UI touches UiState)
- Easier to serialize AppState for persistence
- UiState can be reset without losing data

### Common Utilities (in `tui-dispatch-components`)

Bundle utilities that every TUI app needs into the components crate:

```rust
use tui_dispatch_components::util::{geometry, fmt, text};

// Geometry
geometry::point_in_rect(x, y, rect);
geometry::centered_rect(width, height, area);
geometry::margin(rect, horizontal, vertical);

// Formatting
fmt::number(1234567);                    // "1,234,567"
fmt::size(1_500_000);                    // "1.4 MB"
fmt::duration(Duration::from_secs(154)); // "2m 34s"
fmt::truncate_middle("very long string", 10);  // "very...ing"

// Text
text::wrap(text, width);
text::visible_width(s);  // Unicode-aware
```

### Project Structure Convention

Recommend a folder structure for larger apps:

```
src/
├── main.rs              # Entry point, terminal setup, main loop
├── lib.rs               # Re-exports for testing
├── action.rs            # Action enum with #[derive(Action)]
├── reducer.rs           # Main reducer, composes domain handlers
├── store.rs             # Store type alias, middleware setup
│
├── state/
│   ├── mod.rs
│   ├── app.rs           # AppState (domain)
│   └── ui.rs            # UiState (presentation)
│
├── actions/             # Handler implementations
│   ├── mod.rs
│   ├── sync_handlers.rs
│   └── async_handlers.rs
│
├── ui/
│   ├── mod.rs
│   ├── render.rs        # Main render function, layout
│   ├── theme.rs         # Colors, styles
│   ├── components/      # Event-handling components
│   │   ├── mod.rs
│   │   └── ...
│   └── widgets/         # Pure rendering widgets
│       ├── mod.rs
│       └── ...
│
├── app/                 # Core app logic (non-UI)
│   ├── mod.rs
│   └── ...
│
└── types/               # Domain types
    ├── mod.rs
    └── ...
```

---

## Priority Summary

| Idea | Effort | Value | Status |
|------|--------|-------|--------|
| Theme system | Medium | Medium | Planned |
| `CmdLine` component | Medium-High | High | Planned |
| `Tabs`/`TabBar` component | Medium | Medium | Planned |
| Unified component API | Medium | Medium | Planned |
| Animation system | High | Medium | Future |
