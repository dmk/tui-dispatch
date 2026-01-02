# Roadmap to 1.0.0

An opinionated Redux/Elm-inspired architecture for Rust TUI apps.
Not trying to be everything - just making the core patterns ergonomic.

## Current State

**Core (Done):**
- [x] `Store` with reducer dispatch and middleware
- [x] `EventBus` with subscriptions, focus, component areas
- [x] `Keybindings` with context-aware lookup, merge, serde
- [x] `#[derive(Action)]` with category inference, dispatcher generation
- [x] `#[derive(BindingContext)]`, `#[derive(ComponentId)]`
- [x] Testing: `TestHarness`, `RenderHarness`, fluent assertions, key helpers, time control

**Added in v0.2.x:**
- [x] `DebugLayer::simple()` - one-liner debug overlay setup
- [x] `#[derive(DebugState)]` with `#[debug(section, skip, label)]` attributes
- [x] `#[derive(FeatureFlags)]` with runtime toggle, export/import
- [x] `ActionLoggerMiddleware` with pattern-based filtering

---

## Plans

### 1. Document Component Trait Pattern (High Priority)

The core `Component` trait receives `EventKind` instead of the full `Event`
with context. This is intentional - focus is passed via props.

**Design (Option A - current):**
```rust
// Component receives EventKind, focus passed via Props
fn handle_event(&mut self, event: &EventKind, props: Self::Props<'_>) -> Vec<impl Action>;

// Props include focus info
struct MyProps<'a> {
    state: &'a AppState,
    is_focused: bool,  // caller determines this
}
```

- [ ] Document this pattern clearly in the Component trait docs
- [ ] Add example showing focus handling via props

### 2. Documentation (High Priority)

- [ ] Architecture overview in lib.rs (the "why" and data flow)
- [ ] Make doc examples compile (remove `ignore` where possible)
- [ ] Add a minimal working example in examples/
- [ ] Document the Component pattern with focus handling

### 3. Split Testing Module (Medium Priority)

`testing.rs` at 1400+ lines is unwieldy. Split into:
```
testing/
├── mod.rs          // re-exports
├── assertions.rs   // ActionAssertions, macros
├── harness.rs      // TestHarness
├── render.rs       // RenderHarness, buffer_to_string
├── keys.rs         // key(), keys(), key_event()
└── time.rs         // pause_time, advance_time (feature-gated)
```

### 4. Explicit Category Attribute (Low Priority)

~~For actions that don't follow prefix convention~~ **Already supported:**

```rust
#[derive(Action)]
#[action(infer_categories)]
enum Action {
    #[action(category = "search")]
    StartSearch,  // explicit override

    SearchAddChar(char),  // inferred: "search"
}
```

- [x] `#[action(category = "foo")]` works on variants
- [ ] Document this in the derive macro docs

---

## Non-Goals (Opinionated Choices)

These are intentionally not planned:

- **Async middleware** - Keep middleware simple/sync. Async belongs in effect handlers.
- **Selector/memoization** - Use regular functions. No magic caching.
- **Time-travel debugging** - Cool but overkill. Use tracing + LoggingMiddleware.
- **Global state injection** - Pass state explicitly through props.
- **Component lifecycle hooks** - Mount/unmount complexity not worth it for TUI.

---

## Nice-to-Have (Post 1.0)

- [ ] `ComponentHarness` for isolated component testing
- [ ] Insta integration helpers for render snapshots
- [ ] More key format variants (vim-style `<C-p>`, emacs-style `C-p`)

---

## Post 1.0 Directions

See [Ideas](docs/src/ideas.md) for exploratory features and
[Architectural Additions](docs/src/architecture-additions.md) for
larger architectural proposals.

**Likely next:**
- Effects/Command pipeline (`EffectReducer`, `DispatchResult`)
- Task manager (spawn, debounce, cancel)
- `tui-dispatch-components` crate (SelectList, TextInput, CmdLine)

**Maybe:**
- Theme system
- Input routing / focus tree
- `StoreTestHarness`

**Unlikely (non-goals):**
- Selectors / memoization
- Time-travel debugging
- Async middleware
