# Examples

tui-dispatch includes two example applications that demonstrate different aspects of the framework.

## Weather

A weather TUI that fetches data from the Open-Meteo API. This is the best starting point for understanding tui-dispatch patterns.

**Demonstrates:**
- Full event → action → state → render cycle
- Async API calls with the `Did*` action pattern
- Loading states and error handling
- Action category inference

[Read more →](./weather.md)

## Markdown Preview

A markdown file viewer with debug overlay capabilities. Shows advanced features for building complex TUI applications.

**Demonstrates:**
- Debug layer with F12 toggle
- State inspection overlay
- Frame capture and cell inspection
- Search functionality with navigation
- Vim-like keybindings

[Read more →](./markdown-preview.md)

## Running Examples

From the repository root:

```bash
# Weather example (default city: Kyiv)
cargo run -p weather-example

# Weather with custom city
cargo run -p weather-example -- --city London

# Markdown preview (default: README.md)
cargo run -p markdown-preview

# Preview a specific file
cargo run -p markdown-preview -- path/to/file.md
```
