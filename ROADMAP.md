# Roadmap to 1.0.0

An opinionated Redux/Elm-inspired architecture for Rust TUI apps.
Not trying to be everything - just making the core patterns ergonomic.

## Current State (v0.5.x)

**Core:**
- [x] `Store` with reducer dispatch and middleware
- [x] `EventBus` with subscriptions, focus, component areas
- [x] `Keybindings` with context-aware lookup, merge, serde
- [x] `#[derive(Action)]` with category inference, dispatcher generation
- [x] `#[derive(BindingContext)]`, `#[derive(ComponentId)]`
- [x] Testing: `TestHarness`, `RenderHarness`, fluent assertions, key helpers

**Debug & Dev Tools:**
- [x] `DebugLayer::simple()` - one-liner debug overlay setup
- [x] `#[derive(DebugState)]` with `#[debug(section, skip, label)]` attributes
- [x] `#[derive(FeatureFlags)]` with runtime toggle, export/import
- [x] `ActionLoggerMiddleware` with pattern-based filtering
- [x] `tui-dispatch-debug` crate: snapshots, replay, JSON schema generation
- [x] Action replay with `_await` / `_await_any` markers for async coordination

**Runtime & Effects:**
- [x] `EffectStore` with `DispatchResult<E>` for effect-based reducers
- [x] `TaskManager` (spawn, debounce, cancel)
- [x] `Subscriptions` (intervals, streams)
- [x] `DispatchRuntime` / `EffectRuntime` for event loop boilerplate
- [x] `Component<A>` trait in core

**Components (`tui-dispatch-components`):**
- [x] `SelectList` - scrollable selection with keyboard nav
- [x] `TextInput` - single-line input with cursor, selection
- [x] `ScrollView` - scrollable container
- [x] `StatusBar` - customizable status line
- [x] `TreeView` - collapsible tree navigation
- [x] `Modal` - overlay helpers

**Testing:**
- [x] `StoreTestHarness` / `EffectStoreTestHarness`
- [x] `reducer_compose!` macro for large reducers

---

## Remaining for 1.0

### Documentation (High Priority)

- [ ] Document Component trait pattern (focus via props)
- [ ] Architecture overview in lib.rs (the "why" and data flow)
- [ ] Make doc examples compile (remove `ignore` where possible)
- [ ] Document `#[action(category = "foo")]` in derive macro docs

### Code Quality (Medium Priority)

- [ ] Split testing module (~1400 lines → assertions, harness, render, keys, time)
- [ ] Unify examples to use consistent runtime patterns

---

## Non-Goals (Opinionated Choices)

- **Async middleware** - Keep middleware sync. Async belongs in effect handlers.
- **Selector/memoization** - Use regular functions. No magic caching.
- **Time-travel debugging** - Use tracing + ActionLoggerMiddleware.
- **Global state injection** - Pass state explicitly through props.
- **Component lifecycle hooks** - Not worth the complexity for TUI.

---

## Post 1.0 Ideas

See [Ideas](docs/src/ideas.md) for full details.

**Likely:**
- Theme system (`Theme` trait + derive + presets)
- Unified component API (BaseStyle, standardized Props/Callbacks)
- More components: CmdLine, CommandPalette, Tabs/TabBar, Toast

**Maybe:**
- Animation system
- Scenario test macro
- Unix socket action injection (live LLM debugging)

**Unlikely:**
- Selectors / memoization
- Time-travel debugging
