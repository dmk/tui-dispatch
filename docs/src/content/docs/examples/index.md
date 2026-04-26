---
title: Examples Overview
description: Example applications demonstrating tui-dispatch patterns
---

This repo includes starter examples. For more complete apps, see [dmk/tui-stuff](https://github.com/dmk/tui-stuff).

## Counter

The simplest possible tui-dispatch app - increment/decrement a counter. Start here.

**Demonstrates:**
- Core pattern in ~80 lines
- State, Actions, Reducer, Store
- Event loop and conditional render

[Read more →](/tui-dispatch/examples/counter/)

## GitHub Lookup

A GitHub user lookup TUI. Good next step after Counter.

**Demonstrates:**
- Async API calls with `Did*` action pattern
- TaskManager for HTTP cancellation
- Loading states and error handling
- Text input handling

[Read more →](/tui-dispatch/examples/github-lookup/)

## Markdown Preview

A markdown file viewer with debug overlay and feature flags.

**Demonstrates:**
- Debug layer with F12 toggle
- State inspection overlay
- Feature flags (line numbers, wrap, stats)
- Search with navigation

[Read more →](/tui-dispatch/examples/markdown-preview/)

## Log Viewer

A structured log viewer for files and stdin streams.

**Demonstrates:**
- ComponentHost with multiple mounted widgets
- Routed widget commands through EventBus and Keybindings
- Debug components overlay for widget-local state
- JSON log parsing, filtering, follow mode, and details inspection

[Read more →](/tui-dispatch/examples/log-viewer/)

## Minesweeper

A minesweeper game demonstrating middleware cancel/inject patterns.

**Demonstrates:**
- Middleware for game logic (mine placement on first click)
- Cancel/inject action patterns
- Grid-based UI rendering

## Running Examples

From the repository root:

```bash
# Counter - the minimal example
cargo run -p counter-example

# GitHub Lookup
cargo run -p github-lookup-example

# Log viewer
cargo run -p log-viewer-example --bin log-viewer -- --help

# Markdown preview (default: README.md)
cargo run -p md-preview-example --bin mdpreview

# Markdown preview with debug mode
cargo run -p md-preview-example --bin mdpreview -- path/to/file.md --debug

# Minesweeper
cargo run -p minesweeper-example
```

## More Examples

See [dmk/tui-stuff](https://github.com/dmk/tui-stuff) for larger apps built with tui-dispatch.
