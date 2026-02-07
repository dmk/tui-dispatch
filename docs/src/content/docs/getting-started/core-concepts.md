---
title: Core Concepts
description: Key terminology and concepts in tui-dispatch
---

If you're familiar with [Redux](https://redux.js.org/introduction/core-concepts) or [The Elm Architecture](https://guide.elm-lang.org/architecture/), many concepts will feel familiar.

## Core

The bare minimum to build an app:

### Action
A description of something that happened or should happen. Actions are immutable, cloneable values sent to the store for processing. (See also: [Redux Actions](https://redux.js.org/tutorials/fundamentals/part-2-concepts-data-flow#actions))

```rust
#[derive(Clone, Debug, Action)]
enum AppAction {
    CountIncrement,
    CountDecrement,
    WeatherFetch,
    WeatherDidLoad(WeatherData),
    Quit,
}
```

**Intent actions** trigger work: `WeatherFetch`, `SearchStart`
**Result actions** report outcomes: `WeatherDidLoad`, `SearchDidComplete`

### State
The application's data. A plain Rust struct holding everything the app "knows".

```rust
#[derive(Default)]
struct AppState {
    count: i32,
    weather: Option<WeatherData>,
    is_loading: bool,
}
```

### Reducer
A pure function that takes current state and an action, mutates the state, and returns whether the state changed. (See also: [Redux Reducers](https://redux.js.org/tutorials/fundamentals/part-3-state-actions-reducers))

**Simple reducer** (returns `bool`):
```rust
fn reducer(state: &mut AppState, action: AppAction) -> bool {
    match action {
        AppAction::CountIncrement => { state.count += 1; true }
        AppAction::Quit => false, // false = don't render
    }
}
```

**Effect reducer** (returns `DispatchResult<E>`):
```rust
fn reducer(state: &mut AppState, action: AppAction) -> DispatchResult<Effect> {
    match action {
        AppAction::WeatherFetch => {
            state.is_loading = true;
            DispatchResult::changed_with(Effect::FetchWeather)
        }
        AppAction::WeatherDidLoad(data) => {
            state.weather = Some(data);
            state.is_loading = false;
            DispatchResult::changed()
        }
    }
}
```

### Store
Container that holds state and applies the reducer when actions are dispatched. (See also: [Redux Store](https://redux.js.org/tutorials/fundamentals/part-4-store))

```rust
let mut store = Store::new(AppState::default(), reducer);
let changed = store.dispatch(AppAction::CountIncrement);
```

### Dispatch
The act of sending an action to the store for processing. (See also: [Redux Data Flow](https://redux.js.org/tutorials/fundamentals/part-2-concepts-data-flow))

```rust
store.dispatch(action);        // Sync dispatch
action_tx.send(action);        // Async dispatch via channel
```

## Extensions

Add these when your app needs them. None are required.

### Effect
For apps with async operations. A declarative description of a side effect, returned from the reducer as data. The main loop executes effects outside the reducer. This pattern comes from [The Elm Architecture](https://guide.elm-lang.org/effects/) where commands describe what to do without doing it.

```rust
enum Effect {
    FetchWeather { lat: f64, lon: f64 },
    CopyToClipboard(String),
    SaveFile(PathBuf),
}
```

### DispatchResult
The return type of an effect reducer. Contains whether state changed and any effects to execute.

```rust
DispatchResult::unchanged()            // No change, no effects
DispatchResult::changed()              // State changed, no effects
DispatchResult::effect(e)              // No change, one effect
DispatchResult::changed_with(e)        // State changed, one effect
DispatchResult::changed_with_many(v)   // State changed, multiple effects
```

Use `EffectStore` instead of `Store` when your reducer returns `DispatchResult`:
```rust
let mut store = EffectStore::new(AppState::default(), reducer);
let result = store.dispatch(action);
// result.changed: bool
// result.effects: Vec<Effect>
```

### EventBus
For apps with multiple focusable components. Routes input based on focus state (modal → hovered → focused → subscribers → global) and resolves keybindings.

Requires two app-level types: `ComponentId` to represent focusable targets, and `EventRoutingState` to read focus from your state.

```rust
#[derive(ComponentId, Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum AppComponentId { Main }

impl EventRoutingState<AppComponentId, DefaultBindingContext> for AppState {
    fn focused(&self) -> Option<AppComponentId> { Some(AppComponentId::Main) }
    fn modal(&self) -> Option<AppComponentId> { None }
    fn binding_context(&self, _id: AppComponentId) -> DefaultBindingContext { DefaultBindingContext }
    fn default_context(&self) -> DefaultBindingContext { DefaultBindingContext }
}

let mut bus: SimpleEventBus<AppState, Action, AppComponentId> = SimpleEventBus::new();
bus.register(AppComponentId::Main, |event, state| {
    let actions = handle_event(&event.kind, state);
    if actions.is_empty() {
        HandlerResponse::ignored()
    } else {
        HandlerResponse::actions(actions)
    }
});
```

See [Event Bus](/tui-dispatch/patterns/event-bus/) for the full guide.

### TaskManager
For async operations. Manages one-shot tasks with automatic cancellation. Requires `features = ["tasks"]`.

```rust
let mut tasks = TaskManager::new(action_tx);
tasks.spawn("weather", async move { Action::DidLoad(api::fetch().await) });
tasks.debounce("search", Duration::from_millis(200), async move { ... });
```

### Subscriptions
For continuous action sources like timers and streams. Requires `features = ["subscriptions"]`.

```rust
let mut subs = Subscriptions::new(action_tx);
subs.interval("tick", Duration::from_millis(100), || Action::Tick);
subs.stream("events", event_stream.map(Action::Event));
```

### Component
For reusable UI widgets. A struct that handles events and renders UI via the `Component<A>` trait.

```rust
impl Component<AppAction> for Counter {
    type Props<'a> = &'a AppState;

    fn handle_event(&mut self, event: &EventKind, props: Self::Props<'_>) -> impl IntoIterator<Item = AppAction> {
        match event {
            EventKind::Key(key) if key.code == KeyCode::Up => Some(AppAction::Increment),
            _ => None,
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        // render UI
    }
}
```

### RenderContext
Context passed to render functions, providing access to debug state.

```rust
fn render_app(frame: &mut Frame, area: Rect, state: &AppState, ctx: RenderContext) {
    if ctx.debug_enabled { /* show debug info */ }
}
```

### Keybindings
For user-configurable key mappings. A shared configuration that maps keys to command names, scoped by context.

```rust
let mut bindings = Keybindings::new();
bindings.add_global("quit", vec!["q".to_string()]);
bindings.add(Context::Search, "submit", vec!["enter".to_string()]);
```

See [Keybindings](/tui-dispatch/patterns/keybindings/) for the full guide.

## Derive Macros

### `#[derive(Action)]`
Generates the `Action` trait implementation. Required for all action enums.

```rust
#[derive(Clone, Debug, Action)]
enum AppAction { ... }
```

### `#[action(infer_categories)]`
Auto-generates category methods based on action variant name prefixes.

```rust
#[derive(Action)]
#[action(infer_categories)]
enum Action {
    SearchStart,      // category: "search", is_search() = true
    SearchClear,      // category: "search", is_search() = true
    DidLoadData,      // category: "async_result", is_async_result() = true
    Quit,             // category: None
}
```

Categories enable `reducer_compose!` for routing actions by category.

### `#[action(category = "...")]` (variant attribute)
Explicitly set the category for a specific variant, overriding inference:

```rust
#[derive(Action)]
#[action(infer_categories)]
enum Action {
    // Inferred: "search"
    SearchStart,

    // Override: "network" instead of inferred "api"
    #[action(category = "network")]
    ApiFetch,

    // Override: "network" (no prefix to infer from)
    #[action(category = "network")]
    Reconnect,
}
```

### `#[action(skip_category)]` (variant attribute)
Exclude a variant from category inference entirely:

```rust
#[derive(Action)]
#[action(infer_categories)]
enum Action {
    SearchStart,       // category: "search"

    #[action(skip_category)]
    InternalTick,      // category: None (not categorized)
}
```

### `#[derive(DebugState)]`
Auto-generates debug overlay sections for state inspection.

```rust
#[derive(DebugState)]
struct AppState {
    #[debug(section = "Connection")]
    host: String,

    #[debug(skip)]
    cache: HashMap<String, Data>,
}
```

## Data Flow

Core flow (no extensions):
```
Terminal Input → map to Action → store.dispatch(action) → reducer → render if changed
```

With effects:
```
reducer returns DispatchResult { changed, effects }
  → render if changed
  → for each effect: spawn async task → send result action back to dispatch
```

With EventBus:
```
Terminal Input → EventBus routes to focused handler → handler returns actions → dispatch
```
