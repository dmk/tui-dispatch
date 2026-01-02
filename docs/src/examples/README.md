# Examples

tui-dispatch includes three example applications, from simple to complex.

## Counter

The simplest possible tui-dispatch app - increment/decrement a counter. Start here.

**Demonstrates:**
- Core pattern in ~80 lines
- State, Actions, Reducer, Store
- Event loop and conditional render

[Read more →](./counter.md)

## Weather

A weather TUI that fetches data from the Open-Meteo API.

**Demonstrates:**
- Async API calls with `Did*` action pattern
- Loading states and error handling
- Action logging middleware
- Debug mode (`--debug` flag)

[Read more →](./weather.md)

## Markdown Preview

A markdown file viewer with debug overlay and feature flags.

**Demonstrates:**
- Debug layer with F12 toggle
- State inspection overlay
- Feature flags (line numbers, wrap, stats)
- Search with navigation

[Read more →](./markdown-preview.md)

## Running Examples

From the repository root:

```bash
# Counter - the minimal example
cargo run -p counter

# Weather (default city: Kyiv)
cargo run -p weather-example

# Weather with debug mode
cargo run -p weather-example -- --city London --debug

# Markdown preview (default: README.md)
cargo run -p markdown-preview

# Markdown preview with debug mode
cargo run -p markdown-preview -- path/to/file.md --debug
```
