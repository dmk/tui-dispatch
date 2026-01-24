---
title: Examples Overview
description: Example applications demonstrating tui-dispatch patterns
---

tui-dispatch includes four example applications, from simple to complex.

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

## Weather

A weather TUI that fetches data from the Open-Meteo API.

**Demonstrates:**
- Async API calls with `Did*` action pattern
- Loading states and error handling
- Action logging middleware
- Debug mode (`--debug` flag)

[Read more →](/tui-dispatch/examples/weather/)

## Markdown Preview

A markdown file viewer with debug overlay and feature flags.

**Demonstrates:**
- Debug layer with F12 toggle
- State inspection overlay
- Feature flags (line numbers, wrap, stats)
- Search with navigation

[Read more →](/tui-dispatch/examples/markdown-preview/)

## Running Examples

From the repository root:

```bash
# Counter - the minimal example
cargo run -p counter

# GitHub Lookup
cargo run -p github-lookup-example

# Weather (default city: Kyiv)
cargo run -p weather-example

# Weather with debug mode
cargo run -p weather-example -- --city London --debug

# Markdown preview (default: README.md)
cargo run -p markdown-preview

# Markdown preview with debug mode
cargo run -p markdown-preview -- path/to/file.md --debug
```
