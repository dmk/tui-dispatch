---
title: Debug Sessions & Replay
description: Action recording, replay, and JSON schema generation
---

The `tui-dispatch-debug` crate provides tooling for headless debugging, action recording/replay, and JSON schema generation. These features are particularly useful for automated testing and LLM-assisted development.

## Overview

The debug session system enables:

- **Recording**: Capture all dispatched actions to a JSON file
- **Replay**: Load and replay actions, including async coordination
- **Snapshots**: Save/load state as JSON
- **Schema Generation**: Produce JSON schemas for state and actions
- **Headless Mode**: Render once and exit for CI/testing

## CLI Arguments

`DebugCliArgs` integrates with [clap](https://docs.rs/clap) for standard debug flags:

```rust
use clap::Parser;
use tui_dispatch_debug::DebugCliArgs;

#[derive(Parser)]
struct Args {
    #[clap(flatten)]
    debug: DebugCliArgs,

    // Your other args...
}
```

Available flags:

| Flag | Description |
|------|-------------|
| `--debug` | Enable debug mode (F12 overlay) |
| `--debug-render-once` | Render one frame and exit |
| `--debug-state-in <PATH>` | Load initial state from JSON |
| `--debug-actions-in <PATH>` | Load and replay actions from JSON |
| `--debug-actions-out <PATH>` | Save dispatched actions to JSON |
| `--debug-actions-include <PATTERNS>` | Include patterns (comma-separated globs) |
| `--debug-actions-exclude <PATTERNS>` | Exclude patterns (comma-separated globs) |
| `--debug-actions-include-categories <CATS>` | Include categories (comma-separated) |
| `--debug-actions-exclude-categories <CATS>` | Exclude categories (comma-separated) |
| `--debug-actions-include-names <NAMES>` | Include exact action names (comma-separated) |
| `--debug-actions-exclude-names <NAMES>` | Exclude exact action names (comma-separated) |
| `--debug-state-schema-out <PATH>` | Save state JSON schema |
| `--debug-actions-schema-out <PATH>` | Save actions JSON schema |
| `--debug-replay-timeout <SECS>` | Timeout for async awaits (default: 30) |

## DebugSession

`DebugSession` orchestrates the CLI flags into your app:

```rust
use tui_dispatch_debug::{DebugCliArgs, DebugSession};

#[derive(Parser)]
struct Args {
    #[clap(flatten)]
    debug: DebugCliArgs,
}

let args = Args::parse();
let session = DebugSession::new(args.debug);

// Check if debug mode is enabled
if session.enabled() {
    // Configure debug layer
}

// Check for render-once mode (CI/testing)
if session.render_once() {
    // Will exit after first render
}
```

## Loading State

Load initial state from a JSON file, with a fallback for normal startup:

```rust
let state = session.load_state_or_else(|| {
    // Normal initialization
    Ok(AppState::default())
})?;

// Or with async initialization:
let state = session.load_state_or_else_async(|| async {
    Ok(AppState::fetch_initial().await)
}).await?;
```

## Recording Actions

Record dispatched actions for later replay:

```rust
// Create middleware that records actions
let (middleware, recorder) = session.middleware_with_recorder::<Action>();

// Use with your store
let mut store = StoreWithMiddleware::new(state, reducer, middleware);

// ... run your app ...

// Save recorded actions when done
session.save_actions(recorder.as_ref())?;
```

The recorder respects include/exclude filtering from:
- `--debug-actions-include` / `--debug-actions-exclude` for glob patterns
- `--debug-actions-include-categories` / `--debug-actions-exclude-categories` for inferred categories
- `--debug-actions-include-names` / `--debug-actions-exclude-names` for exact action names

Use the `--debug-actions-include` / `--debug-actions-exclude` glob flags when you want wildcard name matching (`Search*`), and use the `...-names` flags for exact matches only.

## Replay Items

Actions can be replayed from a JSON file. The format supports both simple action arrays and replay items with async coordination:

### Simple Format

```json
[
  "Increment",
  "Increment",
  {"SetValue": 42},
  "Submit"
]
```

### With Await Markers

For async operations, use `_await` to pause replay until an action is dispatched:

```json
[
  {"UserFetch": "octocat"},
  {"_await": "UserDidLoad"},
  "ViewProfile"
]
```

The replay pauses after `UserFetch` and waits until `UserDidLoad` (or `UserDidError`) is dispatched by your effect handler.

### Await Any

Wait for any of several possible actions:

```json
[
  {"WeatherFetch": {"lat": 51.5, "lon": -0.1}},
  {"_await_any": ["WeatherDidLoad", "WeatherDidError"]},
  "ShowDetails"
]
```

## Loading Replay Items

```rust
let replay_items = session.load_replay_items::<Action>()?;

// replay_items: Vec<ReplayItem<Action>>
// - ReplayItem::Action(a) - dispatch this action
// - ReplayItem::AwaitOne { _await } - wait for matching action
// - ReplayItem::AwaitAny { _await_any } - wait for any match
```

## JSON Schema Generation

Generate JSON schemas for your state and action types. Requires the `json-schema` feature:

```toml
[dependencies]
tui-dispatch = { version = "0.5", features = ["json-schema"] }
```

```rust
#[cfg(feature = "json-schema")]
{
    // Save state schema
    session.save_state_schema::<AppState>()?;

    // Save actions schema (includes awaitable_actions list)
    session.save_actions_schema::<Action>()?;
}
```

Your types need to derive `JsonSchema`:

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize, JsonSchema)]
struct AppState {
    count: i32,
    user: Option<User>,
}

#[derive(Action, Clone, Debug, Serialize, Deserialize, JsonSchema)]
enum Action {
    Increment,
    UserFetch(String),
    UserDidLoad(User),
    UserDidError(String),
}
```

The actions schema includes an `awaitable_actions` extension listing actions containing "Did" (async completion markers).

## Complete Integration Example

Here's a full example integrating debug session into an effect-based app:

```rust
use clap::Parser;
use std::io;
use tui_dispatch::prelude::*;
use tui_dispatch_debug::{DebugCliArgs, DebugSession};
use tui_dispatch_debug::debug::DebugLayer;

#[derive(Parser)]
struct Args {
    #[clap(flatten)]
    debug: DebugCliArgs,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let args = Args::parse();
    let session = DebugSession::new(args.debug);

    // Load state (from file or default)
    let state = session.load_state_or_else(|| Ok(AppState::default()))?;

    // Set up middleware with optional recorder
    let (middleware, recorder) = session.middleware_with_recorder::<Action>();

    // Load replay items (empty if no --debug-actions-in)
    let replay_items = session.load_replay_items::<Action>()?;

    // Create store and debug layer
    let mut store = EffectStoreWithMiddleware::new(state, reducer, middleware);
    let debug_layer = if session.enabled() {
        DebugLayer::simple().active(true)
    } else {
        DebugLayer::simple()
    };

    let mut bus: SimpleEventBus<AppState, Action, AppComponentId> = SimpleEventBus::new();
    let keybindings: Keybindings<DefaultBindingContext> = Keybindings::new();

    // Set up terminal...
    let mut terminal = setup_terminal()?;

    // Run with replay support
    let output = session.run_effect_app_with_bus(
        &mut terminal,
        store,
        debug_layer,
        replay_items,
        session.auto_fetch().then_some(Action::AutoFetch),
        Some(Action::Quit),
        |_runtime| {},
        &mut bus,
        &keybindings,
        |frame, area, state, ctx, event_ctx| render(frame, area, state, ctx, event_ctx),
        |action| matches!(action, Action::Quit),
        |effect, ctx| handle_effect(effect, ctx),
    ).await?;

    // Cleanup terminal...
    restore_terminal(&mut terminal)?;

    // Save recorded actions
    session.save_actions(recorder.as_ref())?;

    // Save schemas if requested
    #[cfg(feature = "json-schema")]
    {
        session.save_state_schema::<AppState>()?;
        session.save_actions_schema::<Action>()?;
    }

    Ok(())
}
```

## LLM-Assisted Debugging Workflow

The debug session tooling enables a powerful workflow for LLM-assisted development:

### 1. Record a Session

Run your app and interact with it normally:

```bash
cargo run -- --debug-actions-out session.json
```

### 2. Generate Schemas

```bash
cargo run --features json-schema -- \
  --debug-state-schema-out state-schema.json \
  --debug-actions-schema-out actions-schema.json \
  --debug-render-once
```

### 3. Share with LLM

Provide the LLM with:
- `state-schema.json` - describes your state structure
- `actions-schema.json` - describes available actions and which are awaitable
- `session.json` - example of recorded interactions

### 4. LLM Generates Replay

The LLM can generate a replay file:

```json
[
  {"SearchQuery": "rust tui"},
  {"_await": "SearchDidComplete"},
  {"SearchSelect": 0},
  "Confirm"
]
```

### 5. Replay and Verify

```bash
cargo run -- \
  --debug-actions-in llm-generated.json \
  --debug-render-once \
  --debug-replay-timeout 10
```

The app will:
1. Dispatch `SearchQuery("rust tui")`
2. Wait up to 10 seconds for `SearchDidComplete`
3. Dispatch `SearchSelect(0)`
4. Dispatch `Confirm`
5. Render final state and exit

## Error Handling

Replay errors are reported clearly:

```rust
pub enum ReplayError {
    Timeout { pattern: String },  // Await timed out
    ChannelClosed,                // Action channel closed
}
```

If an `_await` times out, you'll see which pattern was being waited for.

## See Also

- [Debug Layer](/tui-dispatch/debugging/debug-layer/) - Interactive F12 overlay
- [Middleware](/tui-dispatch/patterns/middleware/) - Action recording via middleware
- [Async Patterns](/tui-dispatch/patterns/async/) - Effect-based apps with Did* actions
