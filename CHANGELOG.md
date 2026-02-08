# Changelog

## [0.6.0] - 2026-02-08

Breaking middleware API changes, new EventBus customization, feature-gated dependencies, and docs/examples cleanup.

### Breaking Changes

- **`Middleware<A>` → `Middleware<S, A>`**: Middleware `before()` and `after()` now receive `&S` state reference. Update implementations: `before(&mut self, action: &A, state: &S) -> bool`, `after(&mut self, action: &A, state_changed: bool, state: &S) -> Vec<A>`
- **`DispatchResult<E>` renamed to `ReducerResult<E>`**: All references must be updated
- **`tracing` and `serde` are now feature-gated** in `tui-dispatch-core`: previously always-on dependencies, now require `features = ["tracing"]` or `features = ["serde"]`. `LoggingMiddleware` requires the `tracing` feature
- **Action category inference**: `Did` is now treated as a verb boundary in category inference, grouping intent and result actions under the same category. `WeatherDidLoad` is now category `"weather"` instead of `"weather_did"`, matching `WeatherFetch`. This aligns macro behavior with documented semantics
- **`EventOutcome` moved** from `runtime` to `bus` module — import paths changed
- **Debug formatting helpers renamed**: `ron_string` → `debug_string`, `ron_string_compact` → `debug_string_compact`, `ron_string_pretty` → `debug_string_pretty`
- **`bitflags` dependency removed** from `tui-dispatch-core`

### Added

- **`GlobalKeyPolicy`** for customizing which keys bypass modal blocking in EventBus: `GlobalKeyPolicy::without_esc()`, `::keys()`, `::none()`, `::custom()`. New `EventBus::with_global_key_policy()` builder
- **Middleware cancel/inject semantics**: `before()` returns `false` to cancel an action; `after()` returns `Vec<A>` to inject follow-up actions through the full pipeline with recursion depth guard (`MAX_DISPATCH_DEPTH`)
- Minesweeper example demonstrating middleware cancel/inject patterns
- Facade prelude now exports `SubPauseHandle`, `TaskPauseHandle`, `DefaultBindingContext`, `SimpleEventBus`, and `GlobalKeyPolicy`

### Fixed

- **Middleware dispatch correctness**: `StoreWithMiddleware::dispatch()` now aggregates return values from injected actions — previously it could return `false` even when injected actions changed state, suppressing re-renders
- **Middleware recursion guard**: Dispatch depth is now decremented after recursive injected dispatches complete, so `MAX_DISPATCH_DEPTH` correctly detects injection loops instead of being bypassed
- Same fixes applied to `EffectStoreWithMiddleware`

### Removed

- Weather example (moved to [dmk/tui-stuff](https://github.com/dmk/tui-stuff))

### Improved

- Keybindings matching and EventBus event routing performance
- Runtime event loop methods deduplicated (internal)

### Docs

- Fixed broken internal doc anchors (`#dispatchresult` → `#reducerresult`, `#bindingcontext` → `#keybindings`)
- Fixed markdown-preview example docs showing removed `run_with_bus()` API — now shows actual `DispatchRuntime::run()` usage
- Removed stale "mdBook" reference from crate-level docs
- Converted core rustdoc examples from `ignore` to compile-tested (`Store`, `EffectStore`, crate-level example)
- Added Starlight docs site build to PR CI

## [0.5.4] - 2026-01-25

### Added

- `DataResource<T>` - typed async data lifecycle (Empty/Loading/Loaded/Failed)

### Changed

- Documentation rewrite: clearer separation of core vs extensions
  - Quick start now shows minimal Store-only example (no EventBus boilerplate)
  - Core concepts page restructured into Core and Extensions sections
  - EventBus, TaskManager, Subscriptions framed as optional add-ons
- README updated with minimal example

## [0.5.3] - 2026-01-18

Yeah it's me again. Messed up the publish CI. See [0.5.2](#052---2026-01-18)

## [0.5.2] - 2026-01-18

New `tui-dispatch-debug` crate for LLM-friendly debugging and action replay.

### Added

- `tui-dispatch-debug` crate - debug tooling extracted from core
  - `DebugSession` - CLI integration helper for debug flags
  - `StateSnapshot` / `ActionSnapshot` - JSON serialization for state and actions
  - `ReplayItem` - action replay with `_await` and `_await_any` markers for async coordination
  - `DebugActionRecorder` - middleware for recording dispatched actions
  - `DebugCliArgs` - clap args struct for standard debug CLI flags
- JSON Schema generation (`json-schema` feature)
  - `generate_schema<T>()` / `schema_json<T>()` - generate JSON schema for any type
  - `save_schema<T>(path)` - save schema to file
  - `save_replay_schema<A>(path)` - schema for replay items with `awaitable_actions` list
- New CLI flags in `DebugCliArgs`:
  - `--debug-state-schema-out` - export state JSON schema
  - `--debug-actions-schema-out` - export actions JSON schema
  - `--debug-replay-timeout` - timeout for async await markers (default 30s)
- `tui-dispatch-components` additions:
  - `ScrollView` - scrollable container with keyboard navigation
  - `StatusBar` - customizable status line component
  - `TreeView` - collapsible tree navigation component

### Changed

- Debug layer code moved from `tui-dispatch-core` to `tui-dispatch-debug`
- Weather example updated to use new debug crate and demonstrate replay

## [0.5.1] - 2026-01-11

Testing harnesses and reducer composition macro.

### Added

- `StoreTestHarness` - test harness for `Store` with action collection and state assertions
- `EffectStoreTestHarness` - test harness for `EffectStore` with effect assertions
- `EffectAssertions` - chainable assertions for effects (`effects_count`, `effects_first_matches`, etc.)
- `reducer_compose!` macro - route actions to handlers by category, context, or pattern (for large reducers)

### Fixed

- `reducer_compose!` macro pattern ordering (3-arg form now correctly matches before 4-arg)
- Added missing action verbs for category inference: `Fetch`, `Change`, `Resize`, `Error`

## [0.5.0] - 2026-01-11

New `tui-dispatch-components` crate with reusable UI components.

### Added

- `tui-dispatch-components` crate - common components for TUI apps
  - `TextInput` - text input field with cursor navigation, selection, and customizable styling
  - `SelectList` - keyboard-navigable list with single/multi-select support
  - `Modal` - modal dialog wrapper component
  - `ComponentStyle` - flexible styling system for components
- `TextInputProps::render_action` - emit a render action on cursor-only changes
- Components documentation page (`docs/src/components.md`)

### Changed

- Improved `Component` trait API in core
- Weather example updated to use new components crate

## [0.4.0] - 2026-01-10

Runtime helpers that eliminate event loop boilerplate.

### Added

- `DispatchRuntime` and `EffectRuntime` - wrap the event/action/render loop so you don't have to write it yourself. Handles event polling, action dispatch, debug layer integration, and conditional rendering in ~5 lines instead of ~50.
- `Component<A>` trait now in core - apps can use this instead of defining their own component traits.

### Changed

- All examples updated to use the new runtime helpers

## [0.3.3] - 2026-01-04

Fix some info in LICENSE and Cargo.toml.

## [0.3.2] - 2026-01-04

I'll stop messing up tags at some point, I promise. See changelog for [0.3.1](#031---2025-01-04).

## [0.3.1] - 2026-01-04

### Added

- `DebugLayer::simple()` and `DebugLayer::simple_with_toggle_key()` constructors
- `DebugLayer::set_enabled()` and `DebugLayer::toggle_enabled()` for programmatic control
- `BannerPosition` plus `with_banner_position()` / `set_banner_position()` helpers
- `DebugOutcome::dispatch_queued()` to streamline event-loop wiring

### Fixed

- Debug table/action log scrollbar positions now use full content length

## [0.3.0] - 2026-01-04

Effects, TaskManager, and Subscriptions for declarative async handling.

### Added

- `EffectStore` - reducer returns effects alongside state changes
- `EffectStoreWithMiddleware` - effect store with middleware support
- `DispatchResult<E>` - result type with `changed` flag and `effects` vec
- `TaskManager` - spawn/cancel async tasks that produce actions
  - `spawn(key, future)` - run task, cancel previous with same key
  - `debounce(key, duration, future)` - debounced task execution
  - `cancel(key)` / `cancel_all()` - task cancellation
  - `pause()` / `resume()` - pause/resume task output
- `Subscriptions` - interval and stream-based action sources
  - `interval(key, duration, action_fn)` - periodic action emission
  - `interval_immediate(key, duration, action_fn)` - emit immediately then periodically
  - `stream(key, stream, map_fn)` - forward stream items as actions
  - `cancel(key)` / `cancel_all()` - subscription cancellation
  - `pause()` / `resume()` - pause/resume subscriptions
- `DebugLayer::with_task_manager()` - auto-pause tasks in debug mode
- `DebugLayer::with_subscriptions()` - auto-pause subscriptions in debug mode
- Weather example: multi-color sprite layers (sun=yellow, cloud=gray, etc.)
- Weather example: loading indicator in title bar (sprite stays visible during refresh)

### Changed

- Weather example now uses `EffectStore` with `TaskManager` and `Subscriptions`

## [0.2.2] - 2026-01-01

In-memory action logging with debug overlay integration.

### Added

- `ActionSummary` trait for custom action display (default uses Debug)
- `ActionLog` ring buffer storing recent actions with timestamps
- `ActionLogEntry` with name, summary, timestamp, sequence, state_changed
- `ActionLogConfig` for capacity and filtering settings
- `ActionLoggerMiddleware::with_default_log()` for in-memory storage
- `ActionLoggerMiddleware::with_log()` for custom log configuration
- `ActionLogOverlay` and `ActionLogDisplayEntry` for debug UI
- `ActionLogWidget` for rendering action history table
- `DebugAction::ToggleActionLog` and scroll actions
- `DebugLayer::show_action_log()` method
- `debug.action_log` keybinding (A key) in default debug bindings
- Weather example: full action logging integration
- Weather example: `--refresh-interval` CLI arg for auto-refresh

## [0.2.1] - 2026-01-01

Runtime feature flags for toggling functionality at runtime.

### Added

- `FeatureFlags` trait for runtime feature flag management
- `#[derive(FeatureFlags)]` - auto-generate feature flag accessors
  - `#[flag(default = true)]` - set default values
- `DynamicFeatures` - runtime-defined feature flags
- Feature flags documentation page
- Feature flags example in markdown-preview (L/W/T toggles)

## [0.2.0] - 2025-12-31

Simplified debug layer setup and auto-derive for state inspection.

### Added

- `DebugLayer::simple()` - one-line debug layer setup with sensible defaults
- `DebugLayer::simple_with_toggle_key()` - custom toggle key variant
- `SimpleDebugContext` - built-in context enum for zero-config debug layer
- `#[derive(DebugState)]` - auto-generate `debug_sections()` from struct fields
  - `#[debug(section = "Name")]` - group fields by section
  - `#[debug(skip)]` - exclude fields from debug output
  - `#[debug(label = "Custom Label")]` - custom field labels
  - `#[debug(debug_fmt)]` - use `{:?}` format instead of `Display`
  - `#[debug(format = "...")]` - custom format strings
- `default_debug_keybindings()` and `default_debug_keybindings_with_toggle()`
- Neon color palette for debug UI styling
- `KeyStyles` for per-action key hint colors
- Cell preview rendering in inspect overlays
- Debug layer documentation page

### Changed

- Debug layer styling now uses vibrant neon colors
- `DebugTableStyle` defaults to neon theme
- `CellPreviewWidget` uses neon styling by default

## [0.1.1] - 2025-12-28

Fix workspace dependency versions for crates.io publishing.

## [0.1.0] - 2025-12-28 [YANKED]

Initial release - centralized state management for Rust TUI apps.

### Added

- `Store` with reducer pattern and middleware support
- `EventBus` for pub/sub event routing with focus management
- `Component` trait for pure UI components
- `Keybindings` with context-aware key mapping
- Derive macros: `Action`, `ComponentId`, `BindingContext`
- Debug tools: `DebugLayer`, `ActionLoggerMiddleware`, frame freeze/inspect
- Testing: `TestHarness`, `RenderHarness`, assertion macros
