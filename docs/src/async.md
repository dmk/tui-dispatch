# Async Patterns

tui-dispatch provides three complementary tools for async operations:

| Tool | Purpose | Feature Flag |
|------|---------|--------------|
| **Effects** | Declarative side effects from reducers | Always available |
| **TaskManager** | One-shot async tasks with cancellation | `tasks` |
| **Subscriptions** | Continuous action sources (timers, streams) | `subscriptions` |

## Effects

Effects let reducers declare side effects without executing them directly.
This keeps reducers pure and testable while making async intentions explicit.

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
            state.is_loading = true;
            let loc = &state.location;
            DispatchResult::changed_with(Effect::FetchWeather {
                lat: loc.lat,
                lon: loc.lon,
            })
        }
        Action::WeatherDidLoad(data) => {
            state.weather = Some(data);
            state.is_loading = false;
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

### DispatchResult builders

```rust
DispatchResult::unchanged()              // No state change, no effects
DispatchResult::changed()                // State changed, no effects
DispatchResult::effect(e)                // No state change, one effect
DispatchResult::changed_with(e)          // State changed, one effect
DispatchResult::changed_with_many(vec)   // State changed, multiple effects
```

### Testing effects

Effects are returned data, making them easy to test:

```rust
#[test]
fn test_weather_fetch_emits_effect() {
    let mut state = AppState::default();
    let result = reducer(&mut state, Action::WeatherFetch);

    assert!(result.changed);
    assert!(state.is_loading);
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
}

enum Effect {
    Search { query: String },
    FetchData,
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
            state.is_loading = true;
            DispatchResult::changed_with(Effect::FetchData)
        }
        Action::DataDidLoad(data) => {
            state.data = data;
            state.is_loading = false;
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
                        Action::DataDidLoad(api::fetch().await)
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
