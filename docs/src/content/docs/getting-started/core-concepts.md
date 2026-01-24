---
title: Core Concepts
description: Key terminology and concepts in tui-dispatch
---

Core terms used throughout tui-dispatch. If you're familiar with [Redux](https://redux.js.org/introduction/core-concepts) or [The Elm Architecture](https://guide.elm-lang.org/architecture/), many concepts will feel familiar.

## Core Concepts

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

For effect-based apps, use `EffectStore`:
```rust
let mut store = EffectStore::new(AppState::default(), reducer);
let result = store.dispatch(action);
// result.changed: bool
// result.effects: Vec<Effect>
```

### Dispatch
The act of sending an action to the store for processing. (See also: [Redux Data Flow](https://redux.js.org/tutorials/fundamentals/part-2-concepts-data-flow))

```rust
store.dispatch(action);        // Sync dispatch
action_tx.send(action);        // Async dispatch via channel
```

### Effect
A declarative description of a side effect, returned from an effect reducer. Effects are **not** executed by the reducer - they're returned as data for the main loop to execute. This pattern comes from [The Elm Architecture](https://guide.elm-lang.org/effects/) where commands describe what to do without doing it.

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

## Runtime Concepts

### Runtime
The main event loop that ties everything together: polling events, dispatching actions, executing effects, and rendering.

```rust
DispatchRuntime::new(state, reducer)
    .with_debug(DebugLayer::simple())
    .run(terminal, render_app, map_event, is_quit)
    .await?;
```

For effect-based apps:
```rust
EffectRuntime::new(state, reducer)
    .run(terminal, render_app, map_event, is_quit, handle_effect)
    .await?;
```

### EventOutcome
The result of mapping a terminal event to an action. Tells the runtime whether to render.

```rust
EventOutcome::action(action)        // enqueue one action
EventOutcome::ignored()             // no action, no render
EventOutcome::needs_render()        // force a render (no action)
EventOutcome::action(action).with_render() // enqueue action + force render
```

### Action Channel (tx/rx)
A tokio mpsc channel for sending actions from async tasks back to the main loop.

```rust
let (action_tx, mut action_rx) = tokio::sync::mpsc::unbounded_channel();

// In async handler
tokio::spawn(async move {
    let data = api::fetch().await;
    action_tx.send(Action::DidLoad(data)).ok();
});

// In main loop
while let Some(action) = action_rx.recv().await {
    store.dispatch(action);
}
```

## Async Concepts

### TaskManager
Manages one-shot async tasks with automatic cancellation. Requires `features = ["tasks"]`.

```rust
let mut tasks = TaskManager::new(action_tx);
tasks.spawn("weather", async move { Action::DidLoad(api::fetch().await) });
tasks.debounce("search", Duration::from_millis(200), async move { ... });
```

### Subscriptions
Manages continuous action sources like timers and streams. Requires `features = ["subscriptions"]`.

```rust
let mut subs = Subscriptions::new(action_tx);
subs.interval("tick", Duration::from_millis(100), || Action::Tick);
subs.stream("events", event_stream.map(Action::Event));
```

## UI Concepts

### Component
A struct that handles events and renders UI. Implements the `Component<A>` trait.

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

## Keybindings

### BindingContext
A trait that allows multiple widgets to share the same keybinding configuration. Each widget queries the shared `Keybindings<C>` with its own context.

```rust
#[derive(BindingContext, Clone, Copy, PartialEq, Eq, Hash)]
enum Context {
    KeyList,      // Widget: key browser
    ValueEditor,  // Widget: value editor
    Search,       // Widget: search box
}
```

### Keybindings
A shared configuration that maps keys to command names, scoped by context:

```rust
// Single keybinding config shared across all widgets
let mut bindings = Keybindings::new();

// Global: same key works in all widgets
bindings.add_global("quit", vec!["q".to_string()]);

// Context-specific: different behavior per widget
bindings.add(Context::KeyList, "select", vec!["enter".to_string()]);
bindings.add(Context::ValueEditor, "save", vec!["enter".to_string()]);
bindings.add(Context::Search, "submit", vec!["enter".to_string()]);
```

Each widget queries with its context:
```rust
// In KeyList widget
if let Some(cmd) = bindings.get_command(key_event, Context::KeyList) {
    match cmd.as_str() {
        "quit" => return,    // from global
        "select" => { ... }  // context-specific
        _ => {}
    }
}
```

Benefits:
- Single source of truth for all keybindings
- User config overrides apply everywhere
- Widgets stay decoupled from key definitions

For a complete guide including config file loading and key format details, see [Keybindings](/tui-dispatch/patterns/keybindings/).

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

```
Terminal Input
      |
      v
map_event(event) --> EventOutcome::action(action)
                           |
                           v
                    store.dispatch(action)
                           |
                           v
                    reducer(state, action)
                           |
                           v
                    DispatchResult { changed, effects }
                           |
          +----------------+----------------+
          |                                 |
          v                                 v
    if changed:                    for effect in effects:
      render()                       handle_effect(effect)
                                           |
                                           v
                                    spawn async task
                                           |
                                           v
                                    action_tx.send(DidAction)
                                           |
                                           +---> back to dispatch
```
