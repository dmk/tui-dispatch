---
title: Log Viewer Example
description: Structured log viewer with ComponentHost and debug tooling
---

`log-viewer-example` is a terminal log viewer that reads structured logs from a file or stdin, builds filter facets, and lets you inspect individual entries.

## Run it

```bash
cargo run -p log-viewer-example --bin log-viewer -- path/to/app.log
```

To see stdin mode and debug flags:

```bash
cargo run -p log-viewer-example --bin log-viewer -- --help
```

## Keys

- `Tab` / `Shift+Tab` - move focus between filters, log list, and details
- `j` / `Down` - next log row
- `k` / `Up` - previous log row
- `g` / `Home` - first log row
- `G` / `End` - last log row
- `Enter` - open details for the selected log row
- `f` - toggle follow mode
- `v` - toggle details view mode
- `Esc` - close details
- `q` - quit

## What it demonstrates

This example is the reference app for the newer Layer 2 component flow:

1. **ComponentHost** - mounted `FilterPane`, `SelectList`, and `LogDetails` widgets keep local UI state across frames.
2. **EventBus routing** - route IDs and keybinding contexts direct keys to the focused widget.
3. **Routed commands** - widgets receive semantic command names instead of hardcoded keys.
4. **Debug overlay** - `DebugLayer::with_component_host(...)` exposes mounted component state and render areas.
5. **Runtime wrapper** - `RuntimeHostExt::with_component_host(...)` keeps component hit areas synchronized after render.

## Input Modes

Pass a file path to read an existing log file:

```bash
cargo run -p log-viewer-example --bin log-viewer -- logs/app.jsonl
```

Omit the file path to read from stdin:

```bash
tail -f logs/app.jsonl | cargo run -p log-viewer-example --bin log-viewer
```

Each line can be JSON or plain text. JSON entries use common fields like level, message, timestamp, service, target, and tags when present; plain text still appears in the log list.

## Code Map

| File | Purpose |
|------|---------|
| `src/main.rs` | Runtime, EventBus, ComponentHost, debug layer, and ingestion wiring |
| `src/lib.rs` | Route IDs, binding contexts, default keybindings, and global commands |
| `src/ingest.rs` | File/stdin input mode handling |
| `src/state.rs` | Parsed log entries, filters, follow mode, and selection state |
| `src/reducer.rs` | State transitions for ingestion, filtering, focus, and details |
| `src/ui/` | App layout and mounted component rendering |

## Next Steps

- [Component Host](/tui-dispatch/components/host/) - mounted widgets and runtime integration
- [Interactive Widgets](/tui-dispatch/components/interactive/) - routed widget input model
- [Debug Layer](/tui-dispatch/debugging/debug-layer/) - components overlay and cell inspection
