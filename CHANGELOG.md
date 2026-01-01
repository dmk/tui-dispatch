# Changelog

## [Unreleased]

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
