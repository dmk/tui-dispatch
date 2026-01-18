# Changelog

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

## [0.4.0] - 2025-01-10

Runtime helpers that eliminate event loop boilerplate.

### Added

- `DispatchRuntime` and `EffectRuntime` - wrap the event/action/render loop so you don't have to write it yourself. Handles event polling, action dispatch, debug layer integration, and conditional rendering in ~5 lines instead of ~50.
- `Component<A>` trait now in core - apps can use this instead of defining their own component traits.

### Changed

- All examples updated to use the new runtime helpers

## [0.3.3] - 2025-01-04

Fix some info in LICENSE and Cargo.toml.

## [0.3.2] - 2025-01-04

I'll stop messing up tags at some point, I promise. See changelog for [0.3.1](#031---2025-01-04).

## [0.3.1] - 2025-01-04

### Added

- `DebugLayer::simple()` and `DebugLayer::simple_with_toggle_key()` constructors
- `DebugLayer::set_enabled()` and `DebugLayer::toggle_enabled()` for programmatic control
- `BannerPosition` plus `with_banner_position()` / `set_banner_position()` helpers
- `DebugOutcome::dispatch_queued()` to streamline event-loop wiring

### Fixed

- Debug table/action log scrollbar positions now use full content length

## [0.3.0] - 2025-01-04

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

## [0.2.2] - 2025-01-01

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

## [0.2.1] - 2025-01-01

Runtime feature flags for toggling functionality at runtime.

### Added

- `FeatureFlags` trait for runtime feature flag management
- `#[derive(FeatureFlags)]` - auto-generate feature flag accessors
  - `#[flag(default = true)]` - set default values
- `DynamicFeatures` - runtime-defined feature flags
- Feature flags documentation page
- Feature flags example in markdown-preview (L/W/T toggles)

## [0.2.0] - 2024-12-31

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

## [0.1.1] - 2024-12-28

Fix workspace dependency versions for crates.io publishing.

## [0.1.0] - 2024-12-28 [YANKED]

Initial release - centralized state management for Rust TUI apps.

### Added

- `Store` with reducer pattern and middleware support
- `EventBus` for pub/sub event routing with focus management
- `Component` trait for pure UI components
- `Keybindings` with context-aware key mapping
- Derive macros: `Action`, `ComponentId`, `BindingContext`
- Debug tools: `DebugLayer`, `ActionLoggerMiddleware`, frame freeze/inspect
- Testing: `TestHarness`, `RenderHarness`, assertion macros
