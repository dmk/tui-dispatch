# Ideas

Future directions and experiments. Not committed to any of these.

---

## ~~Easy Debug Layer Setup~~ DONE

> Implemented in v0.2.x

**`DebugLayer::simple()`** - One-liner setup with sensible defaults:

```rust
let debug = DebugLayer::<Action>::simple();  // F12 toggle, sensible defaults
let debug = DebugLayer::<Action>::simple_with_toggle_key(KeyCode::F(11));  // Custom toggle
```

**`#[derive(DebugState)]`** - Auto-derive with field attributes:

```rust
#[derive(DebugState)]
struct AppState {
    #[debug(section = "Connection")]
    host: String,
    port: u16,

    #[debug(section = "UI")]
    scroll_offset: usize,

    #[debug(skip)]
    internal_cache: HashMap<String, Data>,
}
```

---

## Debug Layer Ergonomics

- `DebugLayer::render_with_state(frame, state, |f, area| ...)` convenience that auto-builds the state table only when active.
- `DebugLayer::handle_event(&event) -> DebugOutcome { consumed, queued_actions, needs_render }` to replace ad-hoc intercept/effect plumbing.
- `DebugLayer::middleware()` or `ActionLoggerMiddleware::for_debug(&mut DebugLayer)` to avoid manual `log_action()` calls.
- Expose scrollbar styles/symbols in `DebugStyle` for the state/action overlays.
- Docs: make `render_with_state` the default pattern and list scroll keys + banner toggle.

---

## Runtime / Wiring Helpers

Reduce boilerplate in app `main` by bundling tasks/subscriptions/debug/action routing.

**Option A: run-style helpers (draft API)**

```rust
let mut runtime = DispatchRuntime::new(AppState::default(), reducer)
    .with_debug(DebugLayer::simple())
    .with_event_poller(PollerConfig::default());

runtime.run(
    terminal,
    |frame, area, state, render_ctx| render(frame, area, state, render_ctx),
    |event, state| match event {
        EventKind::Key(key) => match key.code {
            KeyCode::Char('k') => AppAction::CountIncrement.into(),
            KeyCode::Char('j') => AppAction::CountDecrement.into(),
            KeyCode::Char('q') => AppAction::Quit.into(),
            _ => EventOutcome::ignored(),
        },
        EventKind::Resize(_, _) => EventOutcome::needs_render(),
        _ => EventOutcome::ignored(),
    },
    |action| matches!(action, AppAction::Quit),
).await?;
```

Key pieces:
- `DispatchRuntime<S, A>` owns the store, debug layer, action queue, and event poller.
- `EventOutcome<A>` allows returning `actions` plus a `needs_render` hint. Actions impl `Into<EventOutcome>` so `.into()` wraps an action with `needs_render: true`.
- `RenderContext` exposes debug overlay state to render closures.
- `PollerConfig` exposes `poll_timeout` and `loop_sleep` (defaults match examples).
- `runtime.enqueue(action)` to seed initial actions.

**Effect-aware runtime (draft API)**

```rust
let mut runtime = EffectRuntime::new(AppState::new(location), reducer)
    .with_debug(DebugLayer::simple());

runtime.subscriptions().interval("tick", Duration::from_millis(100), || Action::Tick);

runtime.run(
    terminal,
    |frame, area, state, render_ctx| render(frame, area, state, render_ctx),
    |event, state| handle_event(event, state),
    |action| matches!(action, Action::Quit),
    |effect, ctx| handle_effect(effect, ctx),
).await?;
```

Notes:
- `EffectRuntime<S, A, E>` wraps `EffectStore` (or `EffectStoreWithMiddleware`) plus tasks/subs/debug.
- `EffectContext` gives effect handlers access to `tasks()` / `subscriptions()` when enabled.

---

## Component Trait in Core (draft)

Standardize component APIs so event handling + render live together and can plug into the runtime helpers.

```rust
pub trait Component<A: Action> {
    type Props<'a>;

    fn subscriptions(&self) -> Vec<EventType> {
        vec![]
    }

    fn handle_event(&mut self, event: &EventKind, props: Self::Props<'_>) -> Vec<A> {
        vec![]
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>);

    fn area(&self) -> Option<Rect> {
        None
    }
}
```

Notes:
- Fits the memtui-style component pattern (subscriptions, event routing, render).
- Pairs with `DispatchRuntime` / `EffectRuntime` via `EventOutcome`.
- Plays well with existing `TestHarness` and `RenderHarness`.
- Would let apps drop local component traits and share helpers.

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

## Animation System

Centralized animation management following the same pattern as keybindings/themes.

```rust
// Define animations
let animations = Animations::new()
    .register("fade_in", Animation::fade(0.0, 1.0, Duration::from_millis(200)))
    .register("slide_right", Animation::translate_x(-10, 0, Duration::from_millis(150)))
    .register("pulse", Animation::scale(1.0, 1.1, Duration::from_millis(300)).loop_ping_pong());

// Trigger animation
store.dispatch(Action::Animate {
    target: "modal",
    animation: "fade_in"
});

// In render - get interpolated value
let opacity = animations.value("modal.fade_in", state.animation_progress);
```

Animation types:
- `Tween<T>` - interpolate between values
- `Spring` - physics-based spring animation
- `Keyframes` - multi-step sequences

---

## ~~Feature Flags~~ DONE

> Implemented in v0.2.x

Runtime feature flag system with derive macro:

```rust
#[derive(FeatureFlags)]
struct Features {
    #[flag(default = false)]
    new_search_ui: bool,

    #[flag(default = true)]
    vim_bindings: bool,
}

// API
features.enable("new_search_ui");
features.disable("vim_bindings");
features.is_enabled("new_search_ui");
features.toggle("new_search_ui");
features.to_map();  // Export
features.load_from_map(&map);  // Import
```

Also includes `DynamicFeatures` for runtime-only flags without the derive macro.

---

## tui-dispatch-components

A companion crate providing reusable TUI components that integrate with tui-dispatch patterns.

**Separate crate**: `tui-dispatch-components` in the workspace.

### Motivation

Common TUI patterns keep getting reimplemented:
- Vim-style command lines (`:`, `/`, `g` modes)
- Command palettes with fuzzy search
- Modal dialogs (confirm, input, select)
- Scrollable lists with selection
- Forms with field navigation

These share structure: event handling, action emission, state management.
A components crate could provide battle-tested implementations.

### What's Included

1. **Components** - Handle events, return actions (SelectList, TextInput, CmdLine, etc.)
2. **Building Blocks** - Pre-styled ratatui wrappers for common UI patterns (Button, Pane, StatusBadge, TabBar)
3. **Utilities** - Geometry, formatting, text helpers (see "Large Project Patterns" section)

### Components (event-handling)

**Priority 1 - Most universal:**

**SelectList** - Generic scrollable selection
```rust
let list = SelectList::new(items)
    .wrap(true)
    .multi_select(false);

// Emits: ListSelect(index), ListScrollBy(delta), etc.
```

**TextInput** - Single-line input with cursor, selection, clipboard
```rust
let input = TextInput::new()
    .placeholder("Enter name...")
    .max_length(100)
    .validation(|s| !s.is_empty());

// Emits: InputChange(String), InputSubmit, InputCancel
```

**Priority 2 - Common patterns:**

**CmdLine** - Vim-style command input
```rust
// Modes: Command (:), Search (/), Goto (g), custom
let cmdline = CmdLine::new()
    .mode(CmdLineMode::Command)
    .prompt(":")
    .history(true);

// Emits: CmdLineAddChar, CmdLineConfirm, CmdLineHistoryPrev, etc.
```

**Modal** - Confirmation/input dialogs
```rust
let confirm = Modal::confirm("Delete key?")
    .yes_label("Delete")
    .no_label("Cancel")
    .destructive(true);

// Emits: ModalConfirm, ModalCancel
```

**Priority 3 - Nice to have:**

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

**CommandSuggestions** - Inline command hints with alias dedup + arg insertion
```rust
let suggestions = CommandSuggestions::new(commands)
    .max_visible(6)
    .insert_trailing_space_for_args(true);

// Emits: SuggestNext, SuggestPrev, SuggestSelect
```

**ScrollView** - Generic scrollable container
```rust
let scroll = ScrollView::new(content_height)
    .viewport_height(visible_height)
    .scroll_offset(offset);

// Emits: ScrollBy(delta), ScrollTo(position)
```

**Tabs** / **TabBar** - Tab navigation
```rust
let tabs = TabBar::new(&["Overview", "Details", "Settings"])
    .selected(0)
    .style(TabStyle::Underline);

// Emits: TabSelect(index), TabClose(index)
```

**Tree** - Collapsible tree view
```rust
let tree = Tree::new(root_node)
    .expanded(&["src", "src/components"])
    .selected("src/main.rs");

// Emits: TreeSelect(path), TreeToggle(path), TreeExpand(path)
```

**Toast** / **Notification** - Transient messages
```rust
Toast::success("File saved!")
    .duration(Duration::from_secs(3))
    .position(ToastPosition::BottomRight);

// Auto-dismisses, or emits: ToastDismiss(id)
```

### Design Principles

1. **Action-based** - Components emit actions, never mutate state directly
2. **Props-driven** - Render state passed in, component owns only UI state (scroll pos, etc.)
3. **Composable** - Use as building blocks, not monolithic widgets
4. **Macro-friendly** - `commands![]`, `keybindings![]` for ergonomic definitions
5. **Generic over Action type** - Apps define their own action enums, components are generic
6. **Centralized theme** - Components accept `&impl Theme` reference, consistent with "simple API, less code"

### Command Definition Macro

```rust
commands![
    // name | aliases... => description
    "quit" | "q" => "Quit application",

    // with arguments
    "get" | "g" => "Jump to key" { key: String },

    // with handler (optional)
    "theme" => "Change theme" { name: String } => |args| Action::SetTheme(args.name),
]
```

Generates:
```rust
Command {
    name: "quit",
    aliases: &["q"],
    description: "Quit application",
    args: &[],
}
```

### Integration with tui-dispatch

Components would implement the `Component<A: Action>` trait (see "Component Trait in Core" section above):
- Associated `Props<'a>` type for render-time data
- Work with `EventBus` subscriptions via `subscriptions()`
- Emit categorized actions via `#[derive(Action)]`

### Resolved Questions

- **Generic vs concrete actions**: Generic `Component<A>` with associated `Props<'a>` - apps define their own action enums
- **Theming**: Centralized theme via `&impl Theme` - simpler API, less prop drilling
- **Default keybindings**: Provide sensible defaults, allow full override

---

## Test Helper Improvements

### StoreTestHarness

Combines `Store` + `TestHarness` for integrated testing:

```rust
let mut harness = StoreTestHarness::new(AppState::default(), reducer);

// Dispatch and check state
harness.dispatch(Action::Increment);
harness.assert_state(|s| s.count == 1);

// Send keys through component
harness.send_keys("j j enter", |s, e| component.handle_event(&e, s));
harness.drain_emitted().assert_contains(Action::Select(2));

// Snapshot testing
let output = harness.render(|f, area, state| {
    component.render(f, area, state);
});
insta::assert_snapshot!(output);
```

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

## LLM-Aware Debugging

**Killer feature**: Let AI agents debug TUI apps autonomously. TUI renders to text - LLMs can literally "see" it.

### The Vision

```bash
# Agent runs this to test a feature it just implemented
cargo run --features llm-debug -- \
  --state fixtures/app_with_keys.json \
  --actions "SelectKey(5), NextPanel, ScrollDown, ScrollDown" \
  --snapshots-dir /tmp/debug-session
```

Output:
```
/tmp/debug-session/
├── 00_initial.txt      # Render after loading state
├── 01_SelectKey_5.txt  # Render after SelectKey(5)
├── 02_NextPanel.txt    # Render after NextPanel
├── 03_ScrollDown.txt   # Render after first ScrollDown
├── 04_ScrollDown.txt   # Render after second ScrollDown
└── session.json        # State + actions + metadata
```

Each snapshot is plain text the LLM can read:
```
┌─ Keys (5/100) ─────────────────┐┌─ Value ────────────────────────┐
│   user:1                       ││ {                              │
│   user:2                       ││   "name": "Alice",             │
│   user:3                       ││   "email": "alice@example.com",│
│ > user:4                       ││   "active": true               │
│   user:5                       ││ }                              │
└────────────────────────────────┘└────────────────────────────────┘
[Keys] user:4 | string | 128 bytes | TTL: 1h 23m
```

### API Design

```rust
// In app code - opt-in via feature flag
#[cfg(feature = "llm-debug")]
use tui_dispatch::llm_debug::{DebugSession, DebugConfig};

// Headless mode - no terminal, just capture
let config = DebugConfig {
    initial_state: StateSnapshot::from_file("state.json")?,
    terminal_size: (120, 40),
    output_dir: PathBuf::from("/tmp/debug"),
};

let mut session = DebugSession::new(config, app_state, reducer);

// Execute actions and capture
session.dispatch(Action::SelectKey(5));
session.snapshot("after_select");  // Saves render to file

// Or batch mode
session.run_sequence(&[
    Action::SelectKey(5),
    Action::NextPanel,
    Action::ScrollDown,
]);
// Auto-saves snapshot after each action

// Get all snapshots for LLM consumption
let report = session.summary();
// Returns structured text with all snapshots + state diffs
```

### State Serialization

```rust
// Derive macro for serializable state snapshots
#[derive(DebugState, Serialize, Deserialize)]
struct AppState {
    keys: Vec<KeyMetadata>,
    selected: Option<usize>,

    #[debug(skip_serialize)]  // Skip non-serializable fields
    backend: Arc<dyn Backend>,
}

// Generate fixture from running app
session.save_state("fixtures/current_state.json");

// Load for headless debugging
let state = AppState::load_fixture("fixtures/current_state.json")?;
```

### Use Cases

1. **Autonomous Implementation**
   - Agent implements feature
   - Agent writes test state + action sequence
   - Agent runs debug session, "sees" the renders
   - Agent fixes issues without human intervention

2. **Bug Reproduction**
   - User reports bug with state export
   - Agent loads state, replays actions, sees the bug
   - Agent fixes and verifies via snapshots

3. **Visual Regression Testing**
   - CI runs action sequences on fixtures
   - Compare snapshots to golden files
   - Failures include visual diff for LLM analysis

4. **Interactive Debugging**
   - Human describes bug: "when I press j twice the selection jumps weird"
   - Agent: runs `--actions "NextItem, NextItem"` with state
   - Agent sees the render, identifies the bug

### Output Formats

```rust
enum SnapshotFormat {
    // Plain text render (default)
    PlainText,

    // With ANSI colors preserved (for color-aware analysis)
    AnsiText,

    // Structured: render + state + action metadata
    Json {
        render: String,
        state_diff: StateDiff,
        action: String,
        timestamp: Instant,
    },
}
```

### Integration with Existing Debug

Builds on existing `DebugLayer` and `RenderHarness`:

```rust
// RenderHarness already captures renders to string
let harness = RenderHarness::new(120, 40);
let output = harness.render(|f, area| app.render(f, area));

// DebugSession wraps this with state management
let session = DebugSession::from_harness(harness, state, reducer);
```

---

## Large Project Patterns

Ideas for keeping larger tui-dispatch apps (like memtui) organized and maintainable.

### Reducer Composition

Large apps end up with 1000+ line reducers. Provide a macro for context-aware dispatch:

```rust
reducer_compose! {
    // Route by action category (from #[derive(Action)])
    category "nav" => handle_navigation,
    category "search" => handle_search,

    // Route by binding context
    context BindingContext::Command => handle_command_mode,
    context BindingContext::Search => handle_search_mode,

    // Explicit patterns
    Action::Connect* | Action::Disconnect => handle_connection,

    // Fallback
    _ => handle_ui,
}
```

Benefits:
- Scopes handlers by context (command mode vs normal mode)
- Uses existing `Action::category()` from derive macro
- Clear dispatch table instead of chained `||`

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

### Props Derive Macro

Reduce boilerplate when components need many state slices:

```rust
#[derive(Props)]
#[props(from = "AppState, UiState")]
struct KeyListProps<'a> {
    #[props(from = "app.keys")]
    keys: &'a [Key],

    #[props(from = "app.selected_key")]
    selected: Option<usize>,

    #[props(from = "ui.focus == ComponentId::KeyList")]
    is_focused: bool,

    #[props(from = "ui.scroll_offsets.get(&ComponentId::KeyList).copied().unwrap_or(0)")]
    scroll: usize,
}

// Generates:
impl<'a> KeyListProps<'a> {
    pub fn from_state(app: &'a AppState, ui: &'a UiState) -> Self { ... }
}

// Usage in render:
let props = KeyListProps::from_state(&app_state, &ui_state);
key_list.render(frame, area, props);
```

---

## Priority Summary

| Idea | Effort | Value | Status |
|------|--------|-------|--------|
| `DebugLayer::simple()` | Low | High | Done |
| `#[derive(DebugState)]` | Medium | Medium | Done |
| Feature flags | Medium | High | Done |
| `SelectList` component | Medium | High | Next |
| `TextInput` component | Medium | High | Next |
| `StoreTestHarness` | Low | Medium | Planned |
| Theme system | Medium | Medium | Planned |
| `CmdLine` component | Medium-High | High | Planned |
| Animation system | High | Medium | Future |
| LLM-aware debugging | Medium | **Very High** | Planned |
| Reducer composition macro | Low | High | Planned |
| Props derive macro | Medium | Medium | Future |
