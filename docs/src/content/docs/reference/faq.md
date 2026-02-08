---
title: FAQ
description: Frequently asked questions about tui-dispatch
---

## Do I need Tokio?

If you use `DispatchRuntime` / `EffectRuntime`, you'll typically be running inside a Tokio runtime because:

- the runtimes use `tokio::select!` internally
- async patterns (`TaskManager`, `Subscriptions`, async effects) obviously require it

If your app is fully synchronous and you don't want Tokio, you can still use the lower-level pieces (`Store`, `EffectStore`, reducers, components) and write your own loop.

## Where should async code live?

Not in the reducer.

Recommended:

- reducer mutates state and returns `ReducerResult<Effect>`
- effect handler executes effects (spawn tasks, do IO)
- async completion sends a normal action back (`Did*` style)

This keeps reducers easy to test and makes side effects explicit.

## How do I re-render when a component updates internal state?

Internal component state (cursor position, scroll offsets) lives in `&mut self`, so you need a way to request a render even when no app state changes.

Options:

- Return an app action that causes the reducer to return `true` / `ReducerResult::changed()` (common, simple).
- In your EventBus handler, return `HandlerResponse::ignored().with_render().with_consumed(true)` when the component handled the event but emitted no actions.

If you're writing reusable components, consider an explicit "Render" action.

For more on routing, see [Event Bus](/tui-dispatch/patterns/event-bus/).

## What's the difference between Effects and TaskManager?

- `Effect` is *your app's* enum describing side effects as data.
- `TaskManager` is a runtime helper that *executes async work* and gives you cancellation/debouncing.

Most apps use them together:

- reducer returns `Effect::FetchThing`
- effect handler uses `ctx.tasks().spawn("fetch", async { Action::DidLoad(...) })`

## How do I do periodic ticks / background polling?

Enable `subscriptions` and use `Subscriptions` (usually through `EffectRuntime`):

- `interval("tick", Duration::from_millis(100), || Action::Tick)`
- `interval_immediate(...)` if you want an initial fire
- `stream(...)` to forward a stream into actions

See [Async Patterns](/tui-dispatch/patterns/async/).

## How do I enable the debug overlay?

Use `DebugLayer` and wire it into the runtime:

- `DebugLayer::simple()` uses `F12` as the toggle key
- your state type must implement `DebugState` for the state overlay

Example:

```rust
use tui_dispatch::debug::DebugLayer;

#[derive(Default, tui_dispatch::DebugState)]
struct AppState {
    count: i32,
}

let debug: DebugLayer<Action> = DebugLayer::simple().active(true);
let mut runtime = DispatchRuntime::new(AppState::default(), reducer).with_debug(debug);
```

While enabled:
- `S` opens the state tree
- `A` opens the action log

## How do I test async flows?

You usually don't test "async" directly.

Test the pieces:

- reducer emits the right `Effect`
- reducer handles `Did*` actions correctly

`EffectStoreTestHarness` supports this well:

- dispatch intent -> drain effects
- complete `Did*` action -> `process_emitted()`

See [Tutorial: Fetching Data from an API](/tui-dispatch/tutorials/async-fetch/).

---

## See Also

- [Quick Start](/tui-dispatch/getting-started/quick-start/) - Quick start guide
- [Async Patterns](/tui-dispatch/patterns/async/) - Tasks, subscriptions, effects
- [Debug Layer](/tui-dispatch/debugging/debug-layer/) - State inspection and action logging
- [Building Custom Components](/tui-dispatch/components/custom/) - Component trait guide
