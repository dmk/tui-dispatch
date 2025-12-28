# Changelog

## [Unreleased]

## [0.1.0] - 2024-12-28

Initial release - centralized state management for Rust TUI apps.

### Added

- `Store` with reducer pattern and middleware support
- `EventBus` for pub/sub event routing with focus management
- `Component` trait for pure UI components
- `Keybindings` with context-aware key mapping
- Derive macros: `Action`, `ComponentId`, `BindingContext`
- Debug tools: `DebugLayer`, `ActionLoggerMiddleware`, frame freeze/inspect
- Testing: `TestHarness`, `RenderHarness`, assertion macros
