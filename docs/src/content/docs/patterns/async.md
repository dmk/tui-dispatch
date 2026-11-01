---
title: Async & Effects
description: Handling async operations with effects, TaskManager, and subscriptions
---

tui-dispatch provides complementary tools for async operations:

| Tool | Purpose | Feature Flag |
|------|---------|--------------|
| **DataResource** | State type for async data lifecycle | — |
| **RefreshableResource** | Retained data, typed errors, and stale-result rejection | — |
| **Effects** | Declarative side effects from reducers | — |
| **TaskManager** | One-shot async tasks with cancellation | `tasks` |
| **Subscriptions** | Continuous action sources (timers, streams) | `subscriptions` |

## DataResource

`DataResource<T>` captures the full lifecycle of async data in a single type:

```rust
use tui_dispatch::prelude::*;

// Instead of scattered fields:
//   weather: Option<Weather>,
//   is_loading: bool,
//   error: Option<String>,

// Use one type:
struct AppState {
    weather: DataResource<Weather>,
}
```

The variants:

```rust
pub enum DataResource<T> {
    Empty,           // Initial state, no data requested
    Loading,         // Request in flight
    Loaded(T),       // Success
    Failed(String),  // Error message
}
```

Usage in reducers:

```rust
fn reducer(state: &mut AppState, action: Action) -> ReducerResult<Effect> {
    match action {
        Action::WeatherFetch => {
            state.weather = DataResource::Loading;
            ReducerResult::changed().with(Effect::FetchWeather)
        }
        Action::WeatherDidLoad(data) => {
            state.weather = DataResource::Loaded(data);
            ReducerResult::changed()
        }
        Action::WeatherDidError(msg) => {
            state.weather = DataResource::Failed(msg);
            ReducerResult::changed()
        }
        _ => ReducerResult::unchanged(),
    }
}
```

Usage in rendering:

```rust
match &state.weather {
    DataResource::Empty => render_placeholder(),
    DataResource::Loading => render_spinner(),
    DataResource::Loaded(weather) => render_weather(weather),
    DataResource::Failed(err) => render_error(err),
}
```

Helper methods:

```rust
state.weather.is_loading()    // true if Loading
state.weather.is_loaded()     // true if Loaded
state.weather.data()          // Option<&T>
state.weather.error()         // Option<&str>
state.weather.map(|w| w.temp) // Transform inner value
```

## RefreshableResource

Use `RefreshableResource<T, E = String>` when refreshing should keep the previous
data visible. `begin()` clears the error and returns a `ResourceRequestId`; carry
that ID through the effect and completion action:

```rust
use tui_dispatch::prelude::*;

#[derive(Clone, Debug)]
enum FetchError { Offline }

#[derive(Action, Clone, Debug)]
enum Action {
    Fetch,
    DidFetch(ResourceRequestId, Result<String, FetchError>),
    Cancel,
}

enum Effect { Fetch(ResourceRequestId) }

fn reducer(
    resource: &mut RefreshableResource<String, FetchError>,
    action: Action,
) -> ReducerResult<Effect> {
    match action {
        Action::Fetch => {
            let request = resource.begin();
            ReducerResult::changed().with(Effect::Fetch(request))
        }
        Action::DidFetch(request, result) => {
            if resource.complete(request, result) {
                ReducerResult::changed()
            } else {
                ReducerResult::unchanged()
            }
        }
        Action::Cancel => {
            if resource.cancel() {
                ReducerResult::changed()
            } else {
                ReducerResult::unchanged()
            }
        }
    }
}
```

The effect handler returns `Action::DidFetch(request, result)` with the same ID.
Only the latest pending request can complete, once. Older, cancelled, and duplicate
completions return `false` without changing state. An accepted error stops loading
and preserves data; an accepted success replaces data. Use `data()`, `error()`, and
`is_loading()` independently to render cached content with a spinner or error.
`empty()` / `Default` starts without data; `loaded(data)` seeds a cache.

`cancel()` invalidates the pending request without clearing data or error. It does
not stop the task itself; cancel that separately through `TaskManager` if needed.
Resource operations require neither payload cloning nor heap allocation (dropping
replaced data, errors, or rejected results still runs their destructors).

IDs are local to one resource's lifetime. Route completions to their originating
resource, including a resource key in actions when using a collection. Cloning and
the optional `serde` support preserve request state for replay. Before restoring an
older snapshot or replacing a resource with a fresh instance, cancel outstanding
tasks and discard queued completions from the previous state. `json-schema` supports
both the resource and its request ID.

## Cargo Features

Enable features in your `Cargo.toml`:

```toml
[dependencies]
tui-dispatch = { version = "0.7.1", features = ["tasks", "subscriptions"] }
```

| Feature | What it enables | When to use |
|---------|-----------------|-------------|
| `tasks` | `TaskManager` | API calls, file I/O, one-shot async operations |
| `subscriptions` | `Subscriptions` | Timers, WebSocket streams, periodic polling |
| `testing-time` | Time mocking in tests | Testing time-dependent code |

> **Note:** For runtime feature toggles (A/B testing, user preferences), see [Runtime Feature Flags](/tui-dispatch/debugging/feature-flags/).

## Effects

Effects let reducers declare side effects without executing them directly.
This keeps reducers pure and testable while making async intentions explicit.

This pattern comes from [The Elm Architecture](https://guide.elm-lang.org/effects/), where "commands" describe what to do without doing it. Unlike [Redux Thunk](https://redux.js.org/usage/writing-logic-thunks) where async logic lives in action creators, tui-dispatch keeps reducers pure and handles effects separately.

```rust
use tui_dispatch::prelude::*;

// App-defined effect enum
enum Effect {
    FetchWeather { lat: f64, lon: f64 },
    CopyToClipboard(String),
}

fn reducer(state: &mut AppState, action: Action) -> ReducerResult<Effect> {
    match action {
        Action::WeatherFetch => {
            state.weather = DataResource::Loading;
            let loc = &state.location;
            ReducerResult::changed().with(Effect::FetchWeather {
                lat: loc.lat,
                lon: loc.lon,
            })
        }
        Action::WeatherDidLoad(data) => {
            state.weather = DataResource::Loaded(data);
            ReducerResult::changed()
        }
        Action::WeatherDidError(msg) => {
            state.weather = DataResource::Failed(msg);
            ReducerResult::changed()
        }
        Action::Copy(text) => {
            ReducerResult::effect(Effect::CopyToClipboard(text))
        }
        _ => ReducerResult::unchanged(),
    }
}

// Main loop handles effects
let result = store.dispatch(action);
for effect in result.effects {
    match effect {
        Effect::FetchWeather { lat, lon } => {
            let tx = action_tx.clone();
            tokio::spawn(async move {
                match api::fetch(lat, lon).await {
                    Ok(data) => tx.send(Action::WeatherDidLoad(data)),
                    Err(e) => tx.send(Action::WeatherDidError(e.to_string())),
                }
            });
        }
        Effect::CopyToClipboard(text) => {
            clipboard::copy(&text);
        }
    }
}
```

See [ReducerResult](/tui-dispatch/getting-started/core-concepts/#reducerresult) for all builder methods.

### Testing effects

Effects are returned data, making them easy to test:

```rust
#[test]
fn test_weather_fetch_emits_effect() {
    let mut state = AppState::default();
    let result = reducer(&mut state, Action::WeatherFetch);

    assert!(result.changed);
    assert!(state.weather.is_loading());
    assert_eq!(result.effects.len(), 1);
    assert!(matches!(result.effects[0], Effect::FetchWeather { .. }));
}
```

## Task Manager

TaskManager handles one-shot async tasks with automatic cancellation.
Enable with `features = ["tasks"]`.

```rust
use tui_dispatch::prelude::*;
use std::time::Duration;

let (action_tx, mut action_rx) = tokio::sync::mpsc::unbounded_channel();
let mut tasks = TaskManager::new(action_tx);

// Spawn a task - any existing task with same key is cancelled
tasks.spawn("weather", async move {
    match api::fetch(lat, lon).await {
        Ok(data) => Action::WeatherDidLoad(data),
        Err(e) => Action::WeatherDidError(e.to_string()),
    }
});

// Debounced task - waits before executing, resets on each call
tasks.debounce("search", Duration::from_millis(200), async move {
    let results = backend.search(&query).await;
    Action::SearchDidComplete(results)
});

// Manual cancellation
tasks.cancel(&TaskKey::new("weather"));

// Cancel all (e.g., on shutdown)
tasks.cancel_all();
```

### Key behaviors

- **Automatic replacement**: Spawning with an existing key cancels the previous task
- **Debounce**: Timer resets on each call, only executes after quiet period
- **Clean shutdown**: All tasks abort on `Drop`

### Integrating with Effects

When using `Runtime`, effects are handled via `EffectContext`:

```rust
fn handle_effect(effect: Effect, ctx: &mut EffectContext<Action>) {
    match effect {
        Effect::FetchWeather { lat, lon } => {
            ctx.tasks().spawn("weather", async move {
                match api::fetch(lat, lon).await {
                    Ok(data) => Action::WeatherDidLoad(data),
                    Err(e) => Action::WeatherDidError(e.to_string()),
                }
            });
        }
        Effect::Search { query } => {
            ctx.tasks().debounce("search", Duration::from_millis(200), async move {
                Action::SearchDidComplete(backend.search(&query).await)
            });
        }
    }
}
```

## Subscriptions

Subscriptions manage continuous action sources like timers and streams.
Enable with `features = ["subscriptions"]`.

```rust
use tui_dispatch::prelude::*;
use std::time::Duration;

let (action_tx, mut action_rx) = tokio::sync::mpsc::unbounded_channel();
let mut subs = Subscriptions::new(action_tx);

// Tick every 100ms for animations
subs.interval("tick", Duration::from_millis(100), || Action::Tick);

// Auto-refresh every 5 minutes
subs.interval("refresh", Duration::from_secs(300), || Action::WeatherFetch);

// Emit immediately, then at interval
subs.interval_immediate("poll", Duration::from_secs(5), || Action::Poll);

// Forward a stream as actions
subs.stream("events", backend.event_stream().map(Action::BackendEvent));

// Async stream creation
subs.stream_async("redis", async {
    let client = redis::connect().await;
    client.subscribe("events").map(Action::RedisEvent)
});

// Cancel specific subscription
subs.cancel(&SubKey::new("tick"));

// Cancel all on shutdown
subs.cancel_all();
```

### Backpressure, batching, and pause policies

Existing `stream`, `stream_async`, and interval subscriptions discard items/ticks
while paused. Choose `PausePolicy::Buffer` for lossless stream delivery:

```rust
let (tx, mut rx) = ActionChannel::bounded(16); // Capacity counts actions
let mut subs = Subscriptions::with_channel(tx);
subs.stream_batched(
    "logs",
    log_stream,
    BatchConfig::new(100, Duration::from_millis(50)),
    PausePolicy::Buffer { capacity: 500 }, // Capacity counts source items
    Action::LogsReceived, // Receives Vec<LogEntry>
);
```

A batch flushes at its size limit, at the deadline measured from its first item,
or at EOF. A partial batch's deadline does not move when more items arrive.
Pausing suspends delivery, including sends waiting for channel capacity. Buffered
items resume in order; an expired partial batch flushes after resume. Actions
already queued before pause remain readable.

A full pause buffer stops polling the source. A full bounded action channel also
stops source polling while an assembled batch waits to send. Internal storage is
bounded by `max(buffer capacity, batch size)` source items plus at most one
in-flight batch/action; add the output channel and any upstream queues separately.
Neither source items nor payloads are cloned. Batch size and buffer capacity must
be positive; zero delay requests immediate partial flushing.

Use `stream_with_policy(key, actions, policy)` without batching. `PausePolicy::Drop`
discards paused items and partial batches, including pending sends that observe
pause. Cancellation, replacement, or dropping the manager discards unsent work.
Closing the receiver stops stream subscriptions even when their source is idle
or their pause buffer is full.

The runtime's existing action channel remains unbounded. You can batch through
`ctx.subscriptions()` there; use `Subscriptions::with_channel` and drain `rx` in a
custom loop when you need a bounded action queue. `Subscriptions::new` continues
to accept a Tokio unbounded sender.

### When to use what

| Scenario | Tool |
|----------|------|
| API call triggered by user action | TaskManager::spawn |
| Search-as-you-type | TaskManager::debounce |
| Animation tick timer | Subscriptions::interval |
| Periodic data refresh | Subscriptions::interval |
| Websocket messages | Subscriptions::stream |

## Complete Example

```rust
use tui_dispatch::prelude::*;
use std::time::Duration;

#[derive(Action, Clone, Debug)]
enum Action {
    Tick,
    Search(String),
    SearchDidComplete(Vec<Item>),
    Refresh,
    DataDidLoad(Data),
    DataDidError(String),
}

enum Effect {
    Search { query: String },
    FetchData,
}

#[derive(Default)]
struct State {
    animation_frame: usize,
    search_query: String,
    search_results: Vec<Item>,
    data: DataResource<Data>,
}

fn reducer(state: &mut State, action: Action) -> ReducerResult<Effect> {
    match action {
        Action::Tick => {
            state.animation_frame += 1;
            ReducerResult::changed()
        }
        Action::Search(query) => {
            state.search_query = query.clone();
            ReducerResult::effect(Effect::Search { query })
        }
        Action::SearchDidComplete(results) => {
            state.search_results = results;
            ReducerResult::changed()
        }
        Action::Refresh => {
            state.data = DataResource::Loading;
            ReducerResult::changed().with(Effect::FetchData)
        }
        Action::DataDidLoad(data) => {
            state.data = DataResource::Loaded(data);
            ReducerResult::changed()
        }
        Action::DataDidError(msg) => {
            state.data = DataResource::Failed(msg);
            ReducerResult::changed()
        }
    }
}

async fn run() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    let mut store = Store::new(State::default(), reducer);
    let mut tasks = TaskManager::new(tx.clone());
    let mut subs = Subscriptions::new(tx.clone());

    // Start tick timer
    subs.interval("tick", Duration::from_millis(100), || Action::Tick);

    loop {
        let action = rx.recv().await.unwrap();
        let result = store.dispatch(action);

        for effect in result.effects {
            match effect {
                Effect::Search { query } => {
                    let q = query.clone();
                    tasks.debounce("search", Duration::from_millis(200), async move {
                        Action::SearchDidComplete(api::search(&q).await)
                    });
                }
                Effect::FetchData => {
                    tasks.spawn("fetch", async {
                        match api::fetch().await {
                            Ok(data) => Action::DataDidLoad(data),
                            Err(e) => Action::DataDidError(e.to_string()),
                        }
                    });
                }
            }
        }

        if result.changed {
            // re-render
        }
    }
}
```

## Runtime shutdown

All runtime loop variants cancel their tasks, subscriptions, and event poller when
quitting, returning an error, or dropping a polled run future. The runtime can stay
alive after the run ends. Task and subscription registrations are cleared immediately;
Tokio drops the cancelled futures and streams when it next schedules them.

Pausing tasks buffers their completion actions while work continues. Resume returns
buffered actions for dispatch through the normal pipeline; completions racing with
resume are delivered exactly once, either in that returned batch or on the action
channel.
