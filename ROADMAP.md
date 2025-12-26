# Roadmap to 1.0

An opinionated Redux/Elm-inspired architecture for Rust TUI apps.
Not trying to be everything - just making the core patterns ergonomic.

## Current State: 8/10

**Done:**
- [x] `Store` with reducer dispatch and middleware
- [x] `EventBus` with subscriptions, focus, component areas
- [x] `Keybindings` with context-aware lookup, merge, serde
- [x] `#[derive(Action)]` with category inference, dispatcher generation
- [x] `#[derive(BindingContext)]`, `#[derive(ComponentId)]`
- [x] Testing: `TestHarness`, `RenderHarness`, fluent assertions, key helpers, time control

---

## Path to 10/10

### 1. Fix Component Trait (High Priority)

The core `Component` trait isn't used by memtui because it receives `EventKind`
instead of the full `Event` with context. Two options:

**Option A: Pass context through props (current design intent)**
```rust
// Component receives EventKind, focus passed via Props
fn handle_event(&mut self, event: &EventKind, props: Self::Props<'_>) -> Vec<impl Action>;

// Props include focus info
struct MyProps<'a> {
    state: &'a AppState,
    is_focused: bool,  // caller determines this
}
```

**Option B: Make Component generic over event type**
```rust
trait Component<E = EventKind> {
    fn handle_event(&mut self, event: &E, props: Self::Props<'_>) -> Vec<impl Action>;
}

// Usage: impl Component<Event<MyComponentId>> for MyComponent
```

Decision: Stick with Option A, but document the pattern clearly. The caller
(app loop) knows about focus and passes it via props - keeps components decoupled.

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

For actions that don't follow prefix convention:

```rust
#[derive(Action)]
#[action(infer_categories)]
enum Action {
    #[action(category = "search")]
    StartSearch,  // explicit override

    SearchAddChar(char),  // inferred: "search"
}
```

Already partially supported via `#[action(category = "foo")]` on variants,
just needs documentation.

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
