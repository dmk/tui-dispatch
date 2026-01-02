# Architectural Additions (Design Draft)

This draft outlines architectural additions that reduce framework friction at
the same level of impact as `store` + `reducer` + `actions`.

> **Status**: Items 1-3 are implemented in v0.3.0. See [Async Patterns](./async.md).
> See [ROADMAP.md](../../ROADMAP.md) for confirmed roadmap items.
> See also [Ideas](./ideas.md) for other exploratory features.

## Context

Current apps handle side effects, async tasks, subscriptions, and input routing
in ad-hoc loops. That makes logic harder to test, spreads framework concerns
across the app, and requires repeated boilerplate.

## Goals

- Centralize side effects so they are visible, testable, and consistent.
- Provide a declarative way to model background work and subscriptions.
- Standardize async task lifecycle (debounce, replace, cancel, stale results).
- Reduce event routing boilerplate without forcing a single UI style.

## Non-goals

- Replace existing reducer/store APIs.
- Impose a runtime (tokio/async-std) or IO stack on apps.
- Create a full UI framework; components remain optional.

## Proposed additions

### 1) Effect / Command pipeline (immediate) - IMPLEMENTED

Add an optional effect layer so reducers can emit side effects explicitly.

Sketch:

```rust
pub struct DispatchResult<E> {
    pub changed: bool,
    pub effects: Vec<E>,
}

pub trait EffectReducer<S, A, E> {
    fn reduce(&mut self, state: &mut S, action: A) -> DispatchResult<E>;
}

pub struct EffectStore<S, A, E, R> {
    state: S,
    reducer: R,
    // middleware stays additive
}

impl<S, A, E, R> EffectStore<S, A, E, R>
where
    R: EffectReducer<S, A, E>,
{
    pub fn dispatch(&mut self, action: A) -> DispatchResult<E> {
        self.reducer.reduce(&mut self.state, action)
    }
}
```

App loop:

```rust
let DispatchResult { changed, effects } = store.dispatch(action);
for effect in effects {
    runtime.handle(effect, &action_tx);
}
```

Notes:
- Effects can be app-defined enums to avoid coupling to a fixed runtime.
- Provide optional helpers for common effects (spawn task, clipboard, open URL).
- Logging can include effect summaries alongside actions.

### 2) Subscriptions (immediate) - IMPLEMENTED

Provide a declarative registry for continuous sources of actions.

Sketch:

```rust
let mut subs = Subscriptions::new();
subs.add(Subscription::interval(Duration::from_millis(500), || Action::Tick));
subs.add(Subscription::stream("backend", backend.stream_actions()));

// In the main loop
subs.poll(&action_tx);
```

Notes:
- Keeps timers, streams, and channels in one place.
- Supports enabling/disabling subscriptions based on state or features.

### 3) Task manager + cancellation (immediate) - IMPLEMENTED

Standardize async task lifecycle and stale-result handling.

Sketch:

```rust
tasks.spawn(TaskKey::Search, async move {
    let result = backend.search(query).await;
    Action::DidSearch(result)
});

tasks.debounce(TaskKey::Search, Duration::from_millis(250), async move { ... });
tasks.cancel(TaskKey::Search);
```

Notes:
- Provides "replace" semantics to drop outdated futures.
- Pairs naturally with `Did*` actions, but does not require them.

### 4) Input routing + focus tree (immediate)

Optional routing layer to send events to the focused component and allow
controlled bubbling.

Sketch:

```rust
let mut router = EventRouter::new();
router.set_focus(ComponentId::SearchBox);

if let Some(action) = router.route(&event, &mut components, &props) {
    let _ = action_tx.send(action);
}
```

Notes:
- Uses existing `ComponentId` derive for stable IDs.
- Supports global keymaps and focused component handlers.

## Longer-term additions

> Note: Some of these conflict with ROADMAP.md non-goals. Marked accordingly.

### 5) Derived state / selectors

Memoized computed state with clear dependencies, reducing manual caching and
re-render checks.

⚠️ **ROADMAP says non-goal**: "Use regular functions. No magic caching."

### 6) State history + replay

Action log becomes a time-travel tool for debugging and test fixtures.

⚠️ **ROADMAP says non-goal**: "Cool but overkill. Use tracing + LoggingMiddleware."

### 7) Persistence + hydration

State snapshotting with versioned migrations, enabling fast startup and
crash recovery.

### 8) Plugin/middleware extensions

Formal extension points for logging, tracing, analytics, and feature gating.

✅ **Partially done**: `LoggingMiddleware`, `ActionLoggerMiddleware`, feature flags exist.

## Compatibility and migration

- New APIs are additive and can live alongside the current `Store`.
- Existing reducers remain unchanged unless apps opt into effects.
- Debug tooling can surface effects without changing action logs.

## Testing implications

- Provide a `TestRuntime` that collects effects and allows asserting on them.
- Task manager can run in deterministic mode for debounced actions.

## Alternatives considered

- Keep ad-hoc side effects: fast to ship but creates long-term fragmentation.
- Build a monolithic runtime: simplifies wiring but reduces flexibility.

## Open questions

- Should effects live in core or stay app-defined?
- Which default effect types are worth standardizing?
- What is the minimal subscription API that works without async runtime ties?

## Phased rollout

1) Effects + task manager + subscriptions (additive core APIs).
2) Input routing helpers and optional keymap integration.
3) Derived state and replay/persistence as opt-in utilities.
