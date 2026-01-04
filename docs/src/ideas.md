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

- `DebugLayer::render_state(frame, state, |f, area| ...)` convenience that auto-builds the state table only when active.
- `DebugLayer::handle_event(&event) -> DebugOutcome { consumed, queued_actions, needs_render }` to replace ad-hoc intercept/effect plumbing.
- `DebugLayer::middleware()` or `ActionLoggerMiddleware::for_debug(&mut DebugLayer)` to avoid manual `log_action()` calls.
- Expose scrollbar styles/symbols in `DebugStyle` for the state/action overlays.
- Docs: make `render_with_state` the default pattern and list scroll keys + banner toggle.

---

## Runtime / Wiring Helpers

Reduce boilerplate in app `main` by bundling tasks/subscriptions/debug/action routing.

- `DispatchRuntime<A>`: owns `TaskManager`, `Subscriptions`, `DebugLayer`, and an action queue.
  - `handle_event(&EventKind) -> DebugOutcome<A>` (consumed + queued actions + render hint)
  - `next_action()` (from tasks/subs/async)
  - optional `log_action()` or auto‑log on dispatch
- `EffectRuntime<S, A, E>`: wraps `EffectStore` (or with middleware) plus tasks/subs/debug.
  - `dispatch(action, |effect, ctx| handle_effect(effect, ctx)) -> needs_render`
  - `state()` for render, `handle_event()` for input

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

### Core Components

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
6. **Style props, not global theme** - More composable, explicit dependencies

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

Components would use the existing traits:
- Implement `Component<Props, Action>` from tui-dispatch-core
- Work with `EventBus` subscriptions
- Emit categorized actions via `#[derive(Action)]`

### Resolved Questions

- **Generic vs concrete actions**: Generic `Component<Props, A>` - apps define their own action enums
- **Theming**: Accept style props, not global theme - more composable
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
