---
title: Middleware
description: Intercepting actions before and after they reach the reducer
---

Middleware intercepts actions before and after they reach the reducer, enabling cross-cutting concerns like logging, analytics, and debugging without modifying your reducer logic.

## The Middleware Trait

```rust
pub trait Middleware<A: Action> {
    /// Called before the action is dispatched to the reducer
    fn before(&mut self, action: &A);

    /// Called after the reducer processes the action
    fn after(&mut self, action: &A, state_changed: bool);
}
```

The dispatch flow with middleware:

```
action → middleware.before() → reducer() → middleware.after() → result
```

## Using StoreWithMiddleware

Wrap your store with middleware using `StoreWithMiddleware`:

```rust
use tui_dispatch::prelude::*;

let middleware = LoggingMiddleware::new();
let mut store = StoreWithMiddleware::new(AppState::default(), reducer, middleware);

// Dispatch works the same way
let changed = store.dispatch(Action::Increment);
```

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
- `before()`: called in order (first added → last added)
- `after()`: called in reverse order (last added → first added)

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

Pattern syntax:
- `*` matches zero or more characters
- `?` matches exactly one character
- `Search*` matches `SearchStart`, `SearchClear`, `SearchSubmit`
- `*DidLoad` matches `UserDidLoad`, `WeatherDidLoad`

### Accessing the Action Log

When using `with_default_log()` or `with_log()`, you can access recorded actions:

```rust
let middleware = ActionLoggerMiddleware::with_default_log();

// After some dispatches...
if let Some(log) = middleware.log() {
    for entry in log.entries() {
        println!("{}: {} (changed: {})",
            entry.elapsed,
            entry.name,
            entry.state_changed
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

impl<A: Action> Middleware<A> for TimingMiddleware {
    fn before(&mut self, action: &A) {
        self.start = Some(Instant::now());
    }

    fn after(&mut self, action: &A, state_changed: bool) {
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

impl<A: Action> Middleware<A> for MetricsMiddleware {
    fn before(&mut self, _action: &A) {}

    fn after(&mut self, action: &A, _state_changed: bool) {
        self.total_dispatches += 1;
        *self.action_counts.entry(action.name()).or_insert(0) += 1;
    }
}
```

## Middleware with Effects

For `EffectStore`, use `EffectStoreWithMiddleware`:

```rust
use tui_dispatch::prelude::*;

let middleware = LoggingMiddleware::new();
let mut store = EffectStoreWithMiddleware::new(state, reducer, middleware);

let result = store.dispatch(action);
// result.changed: bool
// result.effects: Vec<Effect>
```

The middleware sees the action but not the effects. Effects are handled separately in your effect handler.

## See Also

- [Debug Layer](/tui-dispatch/debugging/debug-layer/) - Interactive debugging with action log overlay
- [Debug Sessions](/tui-dispatch/debugging/debug-sessions/) - Recording and replaying actions
