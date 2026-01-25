---
title: Async & Effects
description: Handling async operations with effects, TaskManager, and subscriptions
---

tui-dispatch provides complementary tools for async operations:

| Tool | Purpose | Feature Flag |
|------|---------|--------------|
| **DataResource** | State type for async data lifecycle | — |
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
fn reducer(state: &mut AppState, action: Action) -> DispatchResult<Effect> {
    match action {
        Action::WeatherFetch => {
            state.weather = DataResource::Loading;
            DispatchResult::changed_with(Effect::FetchWeather)
        }
        Action::WeatherDidLoad(data) => {
            state.weather = DataResource::Loaded(data);
            DispatchResult::changed()
        }
        Action::WeatherDidError(msg) => {
            state.weather = DataResource::Failed(msg);
            DispatchResult::changed()
        }
        _ => DispatchResult::unchanged(),
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

## Cargo Features

Enable features in your `Cargo.toml`:

```toml
[dependencies]
tui-dispatch = { version = "0.5.3", features = ["tasks", "subscriptions"] }
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

fn reducer(state: &mut AppState, action: Action) -> DispatchResult<Effect> {
    match action {
        Action::WeatherFetch => {
            state.weather = DataResource::Loading;
            let loc = &state.location;
            DispatchResult::changed_with(Effect::FetchWeather {
                lat: loc.lat,
                lon: loc.lon,
            })
        }
        Action::WeatherDidLoad(data) => {
            state.weather = DataResource::Loaded(data);
            DispatchResult::changed()
        }
        Action::WeatherDidError(msg) => {
            state.weather = DataResource::Failed(msg);
            DispatchResult::changed()
        }
        Action::Copy(text) => {
            DispatchResult::effect(Effect::CopyToClipboard(text))
        }
        _ => DispatchResult::unchanged(),
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

See [DispatchResult](/tui-dispatch/getting-started/core-concepts/#dispatchresult) for all builder methods.

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

```rust
fn handle_effect(effect: Effect, tasks: &mut TaskManager<Action>, tx: Sender<Action>) {
    match effect {
        Effect::FetchWeather { lat, lon } => {
            tasks.spawn("weather", async move {
                match api::fetch(lat, lon).await {
                    Ok(data) => Action::WeatherDidLoad(data),
                    Err(e) => Action::WeatherDidError(e.to_string()),
                }
            });
        }
        Effect::Search { query } => {
            tasks.debounce("search", Duration::from_millis(200), async move {
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

fn reducer(state: &mut State, action: Action) -> DispatchResult<Effect> {
    match action {
        Action::Tick => {
            state.animation_frame += 1;
            DispatchResult::changed()
        }
        Action::Search(query) => {
            state.search_query = query.clone();
            DispatchResult::effect(Effect::Search { query })
        }
        Action::SearchDidComplete(results) => {
            state.search_results = results;
            DispatchResult::changed()
        }
        Action::Refresh => {
            state.data = DataResource::Loading;
            DispatchResult::changed_with(Effect::FetchData)
        }
        Action::DataDidLoad(data) => {
            state.data = DataResource::Loaded(data);
            DispatchResult::changed()
        }
        Action::DataDidError(msg) => {
            state.data = DataResource::Failed(msg);
            DispatchResult::changed()
        }
    }
}

async fn run() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    let mut store = EffectStore::new(State::default(), reducer);
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
