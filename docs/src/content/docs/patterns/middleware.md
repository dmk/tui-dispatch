---
title: Middleware
description: Intercepting actions before and after they reach the reducer
---

Middleware intercepts actions before and after they reach the reducer, enabling cross-cutting concerns like logging, analytics, throttling, and debugging without modifying your reducer logic.

## The Middleware Trait

```rust
pub trait Middleware<S, A: Action> {
    /// Called before the action is dispatched to the reducer.
    /// Return `true` to proceed, `false` to cancel the action.
    fn before(&mut self, action: &A, state: &S) -> bool;

    /// Called after the reducer processes the action.
    /// Return follow-up actions to dispatch through the full pipeline.
    fn after(&mut self, action: &A, state_changed: bool, state: &S) -> Vec<A>;
}
```

Middleware receives a read-only reference to the store's state both before and after dispatch.

### Cancel

Returning `false` from `before()` prevents the action from reaching the reducer entirely. Use this for throttling, rate limiting, or conditional gating.

### Inject

Returning actions from `after()` dispatches them through the full middleware → reducer → middleware pipeline. This enables derived actions, automatic follow-ups, and cascade patterns.

The dispatch flow with middleware:

```
action → middleware.before(&state) →  false? → cancelled
                                  →  true?  → reducer() → middleware.after(&state) → result
                                                                   ↓
                                                            injected actions
                                                                   ↓
                                                     full pipeline (recursive)
```

Middleware dispatch is protected by configurable `DispatchLimits` (defaults: `max_depth = 16`, `max_actions = 10_000`) to prevent runaway loops. Both limits should be at least `1`. The action budget counts attempted dispatches, including actions cancelled by `before()`.

## Using StoreWithMiddleware

Wrap your store with middleware using `StoreWithMiddleware`:

```rust
use tui_dispatch::prelude::*;

let middleware = LoggingMiddleware::new();
let mut store = StoreWithMiddleware::new(AppState::default(), reducer, middleware)
    .with_dispatch_limits(DispatchLimits {
        max_depth: 64,
        max_actions: 10_000,
    });

// Non-panicking path with typed errors
match store.try_dispatch(Action::Increment) {
    Ok(changed) => {
        if changed {
            // render
        }
    }
    Err(err) => {
        // log / telemetry / fallback
        eprintln!("dispatch failed: {err}");
    }
}
```

`dispatch(...)` remains available for compatibility and wraps `try_dispatch(...)`; it panics if dispatch limits are exceeded.

`try_dispatch(...)` is not transactional: if an injected chain overflows, earlier actions may already have mutated state.

## Built-in Middleware

### LoggingMiddleware

Logs actions via the `tracing` crate:

```rust
use tui_dispatch::LoggingMiddleware;

// Log after dispatch only (default)
let middleware = LoggingMiddleware::new();

// Log both before and after
let middleware = LoggingMiddleware::verbose();
```

Output (at debug level):
```
DEBUG action dispatched: Increment (changed: true)
```

### NoopMiddleware

A no-op middleware for type-level plumbing when you need the `StoreWithMiddleware` type but don't want any middleware behavior:

```rust
use tui_dispatch::NoopMiddleware;

let store = StoreWithMiddleware::new(state, reducer, NoopMiddleware);
```

### ComposedMiddleware

Chain multiple middleware together:

```rust
use tui_dispatch::{ComposedMiddleware, LoggingMiddleware};

let mut composed = ComposedMiddleware::new();
composed.add(LoggingMiddleware::new());
composed.add(MyCustomMiddleware::new());

let store = StoreWithMiddleware::new(state, reducer, composed);
```

Execution order:
- `before()`: called in order (first added → last added). Short-circuits if any returns `false`.
- `after()`: called in reverse order (last added → first added). All injected actions are collected.

This ensures proper nesting (like try/finally blocks).

## ActionLoggerMiddleware

The debug crate provides `ActionLoggerMiddleware` for detailed action logging with pattern filtering and in-memory storage.

```rust
use tui_dispatch::debug::ActionLoggerMiddleware;

// Default: filters out Tick and Render actions
let middleware = ActionLoggerMiddleware::default_filtering();

// Log everything
let middleware = ActionLoggerMiddleware::log_all();

// With in-memory ring buffer (for debug overlay)
let middleware = ActionLoggerMiddleware::with_default_log();
```

### Pattern Filtering

Filter actions using glob patterns:

```rust
use tui_dispatch::debug::{ActionLoggerConfig, ActionLoggerMiddleware};

let config = ActionLoggerConfig::new(
    Some("Search*,User*"),  // Include: only these patterns
    Some("*Tick,*Render"),  // Exclude: filter these out
);

let middleware = ActionLoggerMiddleware::new(config);
```

You can combine filter types:

```rust
let config = ActionLoggerConfig::new(
    Some("cat:search,name:SearchSubmit"), // Include search category + one exact action
    Some("name:SearchCancel"),            // Exclude one action explicitly
);
```

Pattern syntax:
- `*` matches zero or more characters
- `?` matches exactly one character
- `Search*` matches `SearchStart`, `SearchClear`, `SearchSubmit`
- `*DidLoad` matches `UserDidLoad`, `WeatherDidLoad`
- `cat:search` matches actions in inferred `search` category
- `cat:search_*` supports category globs
- `name:WeatherDidLoad` matches one exact action name (no wildcards)

Notes:
- `name:` is exact-match only and case-sensitive. Use plain glob patterns (for example `WeatherDid*`) for wildcard name matching.
- `cat:` uses inferred action categories, which are strongest with NounVerb naming (`SearchStart`, `WeatherDidLoad`). VerbNoun names (`OpenFile`) typically do not infer categories.
- Inferred category values are lowercase (for example `search_query`), so category filter patterns should also be lowercase.
- Single-word action names (for example `Tick`) usually have no inferred category, so `cat:tick` will not match them. Use name/glob filters instead.

### Accessing the Action Log

When using `with_default_log()` or `with_log()`, you can access recorded actions:

```rust
let middleware = ActionLoggerMiddleware::with_default_log();

// After some dispatches...
if let Some(log) = middleware.log() {
    for entry in log.entries() {
        println!("{}: {}",
            entry.elapsed,
            entry.name,
        );
    }

    // Get recent entries
    for entry in log.recent(5) {
        // Last 5 actions
    }
}
```

### Integration with Debug Layer

The debug layer can display the action log in an overlay:

```rust
use tui_dispatch::debug::{ActionLoggerMiddleware, DebugLayer};

let middleware = ActionLoggerMiddleware::with_default_log();
let debug = DebugLayer::simple();

// In your render loop, the debug layer can show the action log
// Press 'A' when debug is enabled to toggle the action log overlay
```

## Writing Custom Middleware

Implement the `Middleware` trait for custom behavior:

```rust
use tui_dispatch::{Action, Middleware};
use std::time::Instant;

struct TimingMiddleware {
    start: Option<Instant>,
}

impl TimingMiddleware {
    fn new() -> Self {
        Self { start: None }
    }
}

impl<S, A: Action> Middleware<S, A> for TimingMiddleware {
    fn before(&mut self, action: &A, _state: &S) -> bool {
        self.start = Some(Instant::now());
        true
    }

    fn after(&mut self, action: &A, state_changed: bool, _state: &S) -> Vec<A> {
        if let Some(start) = self.start.take() {
            let elapsed = start.elapsed();
            if elapsed.as_millis() > 10 {
                tracing::warn!(
                    "Slow action: {} took {:?}",
                    action.name(),
                    elapsed
                );
            }
        }
        vec![]
    }
}
```

### Cancelling Actions

Use `before()` returning `false` to prevent actions from reaching the reducer:

```rust
struct ThrottleMiddleware {
    last_dispatch: Option<Instant>,
    min_interval: Duration,
}

impl<S, A: Action> Middleware<S, A> for ThrottleMiddleware {
    fn before(&mut self, _action: &A, _state: &S) -> bool {
        let now = Instant::now();
        if let Some(last) = self.last_dispatch {
            if now.duration_since(last) < self.min_interval {
                return false; // Too soon — cancel
            }
        }
        self.last_dispatch = Some(now);
        true
    }

    fn after(&mut self, _action: &A, _state_changed: bool, _state: &S) -> Vec<A> {
        vec![]
    }
}
```

### Injecting Follow-Up Actions

Use `after()` to inject actions that go through the full pipeline:

```rust
struct AutoSaveMiddleware {
    dirty_count: u32,
    save_threshold: u32,
}

impl<S, A: Action> Middleware<S, A> for AutoSaveMiddleware
where
    A: From<SaveAction>,
{
    fn before(&mut self, _action: &A, _state: &S) -> bool {
        true
    }

    fn after(&mut self, _action: &A, state_changed: bool, _state: &S) -> Vec<A> {
        if state_changed {
            self.dirty_count += 1;
            if self.dirty_count >= self.save_threshold {
                self.dirty_count = 0;
                return vec![SaveAction::AutoSave.into()];
            }
        }
        vec![]
    }
}
```

### Middleware with State

Middleware can maintain state across dispatches:

```rust
struct MetricsMiddleware {
    action_counts: HashMap<&'static str, u64>,
    total_dispatches: u64,
}

impl<S, A: Action> Middleware<S, A> for MetricsMiddleware {
    fn before(&mut self, _action: &A, _state: &S) -> bool {
        true
    }

    fn after(&mut self, action: &A, _state_changed: bool, _state: &S) -> Vec<A> {
        self.total_dispatches += 1;
        *self.action_counts.entry(action.name()).or_insert(0) += 1;
        vec![]
    }
}
```

## Middleware with Effects

For `Store`, use `StoreWithMiddleware`:

```rust
use tui_dispatch::prelude::*;

let middleware = LoggingMiddleware::new();
let mut store = StoreWithMiddleware::new(state, reducer, middleware);

let result = store.dispatch(action);
// result.changed: bool
// result.effects: Vec<Effect>
```

The middleware sees the action but not the effects. Effects are handled separately in your effect handler. Injected actions from `after()` have their effects merged into the returned result.

## See Also

- [Debug Layer](/tui-dispatch/debugging/debug-layer/) - Interactive debugging with action log overlay
- [Debug Sessions](/tui-dispatch/debugging/debug-sessions/) - Recording and replaying actions
