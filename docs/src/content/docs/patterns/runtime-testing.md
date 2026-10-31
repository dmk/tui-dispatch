---
title: Runtime testing
description: Inject events and test async runtime behavior with virtual time
---

Use `RuntimeTestHarness` to test direct event mapping, middleware, effects, and
rendering together. It owns a real `Runtime` and uses its event normalization,
dispatch, debug, and rendering helpers with an in-memory terminal.

Enable `testing-time` (and `tasks` or `subscriptions` when needed), then run the
harness in a current-thread Tokio test with paused time:

```rust
use std::time::Duration;
use tui_dispatch::{testing::RuntimeTestHarness, EventKind, EventOutcome, Runtime};

#[tokio::test(start_paused = true)]
async fn fetch_after_debounce() {
    // AppState, Action, Effect, and reducer are your application's types.
    let mut app = RuntimeTestHarness::from_runtime(Runtime::new(AppState::default(), reducer))
        .with_event_mapper(|event, _| match event {
            EventKind::Key(_) => EventOutcome::action(Action::Fetch),
            _ => EventOutcome::ignored(),
        })
        .with_effect_handler(|Effect::Fetch, ctx| {
            ctx.tasks().debounce("fetch", Duration::from_millis(200), async {
                Action::DidLoad(42)
            });
        });

    app.press("enter");
    app.run_until_idle().await;
    assert_eq!(app.state().count, 0);
    app.advance(Duration::from_millis(200)).await;
    assert_eq!(app.state().count, 42);
    assert_eq!(app.take_actions(), [Action::Fetch, Action::DidLoad(42)]);
}
```

`run_until_idle` settles queued input, actions, and runnable work spawned through
the runtime's task and subscription managers. It does not advance time or wait
for pending I/O, sleeping tasks, or paused completion queues. `advance` first
settles existing work, advances Tokio's clock by the requested duration, and
settles again. Synchronize work spawned outside these managers yourself.
A continuously runnable task or action cycle fails after 10,000 steps.

Queued actions run before the next injected event. `take_actions` records root
actions dequeued by the runtime, including quit actions and actions cancelled by
middleware; it does not include middleware injections. Use `with_render` and
`with_size` to configure rendering, then inspect `buffer()` after settling.
`inject(RawEvent::Resize(width, height))` resizes the test terminal and routes the
resize event. `with_quit_predicate` uses the runtime's quit-before-dispatch behavior.
Dropping or stopping the harness cancels its tasks and subscriptions.

## Custom event sources

All runtime loop variants, including EventBus routing, accept `with_event_source`.
The factory starts a fresh event-producing task for each run, replacing terminal
polling:

```rust
let runtime = Runtime::new(state, reducer).with_event_source(|tx, _cancel| {
    tokio::spawn(async move {
        tx.send(tui_dispatch::RawEvent::Key(tui_dispatch::testing::key("enter")))
            .expect("runtime input channel");
    })
});
```

Use the supplied cancellation token for cooperative cancellation when useful.
The runtime cancels it and aborts the returned task on quit, dispatch-stop,
rendering errors, and cancellation of the run future. Async destruction happens
when Tokio next schedules that task. Source channel closure disables input;
actions and async work continue until the application stops the runtime.

The harness supplies its own scripted input and does not start a configured
source. For EventBus integration tests, use a custom source with the normal
runtime loop and a Ratatui `TestBackend`.
