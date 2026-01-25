---
title: Philosophy
description: Why tui-dispatch exists and how to think about it
---

## Why tui-dispatch exists

Three goals, in priority order:

### 1. Manageable data layer + async

Complex TUI apps have state scattered everywhere - in widgets, in globals, in closures. Bugs hide. Changes cascade unpredictably.

tui-dispatch centralizes state with Redux/Elm patterns: single store, pure reducers, declarative effects. You always know where state lives and how it changes.

### 2. Debuggable by default

Every state change goes through an action. If your state is serializable, you can snapshot it, log the action history, and replay sessions to reproduce bugs.

This requires discipline: state must be serializable (no file handles, no `Rc<RefCell<...>>`), and side effects must go through the effect system. The framework provides derive macros to help, but you can break debuggability if you bypass the patterns.

The debug overlay and replay tools exist because this architecture makes them possible - not the other way around.

### 3. Reduce boilerplate

Derive macros for actions and component IDs. Prebuilt components for common UI patterns. Runtime helpers that wire up the event loop. The goal is less ceremony for common cases.

This is ongoing work. If you find yourself writing the same pattern repeatedly, that's a gap in the framework.

## When not to use tui-dispatch

- Simple single-screen apps with minimal state
- Scripts that just render output and exit
- Apps where state is naturally local to each widget with no sharing
- Teams unfamiliar with Redux/Elm who need to ship quickly

## Core principles

### State is the source of truth

If it affects what the user sees, it should be in state. Hidden component state can't be inspected, logged, or replayed.

Corollary: if you can't serialize it, you can't debug it. Keep non-serializable resources (connections, file handles) outside AppState, accessed via effects.

### Unidirectional flow

```
Event → Component → Action → Reducer → State → Render
```

Events produce actions. Actions produce state changes. State changes produce renders. Side effects are declared by reducers and executed by the runtime.

Escape hatches exist (you can mutate state outside the reducer, skip actions, render without state changes), but using them breaks the guarantees that make debugging work.

### Declarative effects

Reducers don't perform side effects - they return descriptions of work to do:

```rust
// Reducer declares intent, returns effect
Action::FetchWeather => {
    state.weather = DataResource::Loading;
    DispatchResult::changed_with(Effect::FetchWeather { city })
}

// Runtime executes effect, sends result back as action
Effect::FetchWeather { city } => {
    match api::fetch(city).await {
        Ok(data) => tx.send(Action::WeatherDidLoad(data)),
        Err(e) => tx.send(Action::WeatherDidFail(e.to_string())),
    }
}
```

This keeps reducers pure: given the same state and action, they produce the same result. See [Async & Effects](/tui-dispatch/patterns/async/) for the full pattern.

### Explicit over implicit

No hidden state, no implicit mutations. If something changes, there's an action for it. If there's state, it lives in AppState.

## Recommended patterns

### Domain/UI state separation

State naturally splits into two concerns:

```rust
struct AppState {
    // Domain: your app's data and business logic
    items: Vec<Item>,
    selected_id: Option<ItemId>,
    filters: FilterConfig,
    favorites: HashSet<ItemId>,

    // UI: component internals (scroll, cursors, modals)
    ui: UiState,
}

struct UiState {
    list_scroll: usize,
    search_input: String,
    search_cursor: usize,
    active_modal: Option<ModalId>,
}
```

**Domain state** is your app's data - what you'd persist or sync. **UI state** is component internals that affect rendering but aren't business logic: scroll positions, cursor locations, which modal is open.

Benefits:
- Clean domain logic (no scroll offsets mixed with business data)
- UI state is serializable, enabling session replay
- Reusable components can find their state in a predictable location

This is a recommended pattern, not enforced.

### DataResource for async state

Instead of scattering `loading: bool` and `error: Option<String>` across your state:

```rust
enum DataResource<T> {
    Empty,
    Loading,
    Loaded(T),
    Failed(String),
}

// Usage
struct AppState {
    weather: DataResource<Weather>,
}

// In reducer
Action::FetchWeather => {
    state.weather = DataResource::Loading;
    DispatchResult::changed_with(Effect::FetchWeather)
}
Action::WeatherDidLoad(data) => {
    state.weather = DataResource::Loaded(data);
    DispatchResult::changed()
}
Action::WeatherDidFail(err) => {
    state.weather = DataResource::Failed(err);
    DispatchResult::changed()
}
```

One type captures the full lifecycle. UI code matches on the variant to show loading spinners, errors, or data.

### Event routing

EventBus routes events by priority: **modal → focused → global**

- Modal handlers get first shot (dialogs, overlays)
- Then the focused component
- Then global handlers (quit, help)

This matches user expectation: when a modal is open, it captures input. You declare which component is focused and which modals are open; the framework routes.

For custom routing, you can bypass EventBus and handle events directly - see [Core Concepts](/tui-dispatch/getting-started/core-concepts/).

## Framework design principles

For contributors adding features to tui-dispatch:

1. **State over behavior** - If it can be state, make it state. Framework components that hold internal state (instead of reading from AppState) break debuggability.

2. **Composable over complete** - Small pieces that combine well beat monolithic solutions.

3. **Ergonomic defaults, escape hatches** - Common case should be easy. Unusual cases should be possible.

4. **Guide, don't enforce** - Recommend patterns, provide helpers, but don't mandate structure.
