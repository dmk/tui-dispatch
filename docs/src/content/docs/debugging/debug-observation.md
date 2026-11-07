---
title: Dispatch observation
description: Collect action outcomes and state snapshots without an overlay
---

`DebugCollector` captures dispatches without a terminal, a debug overlay, or a
connected debugger. Attach it to a store, then use that store directly or pass it
to `Runtime::from_store`:

```rust
use serde_json::json;
use tui_dispatch::{Runtime, StoreWithMiddleware};
use tui_dispatch::observe::{DebugCollector, DebugStoreExt};

let collector = DebugCollector::new(json!({ "count": 0 }), 256);
let store = StoreWithMiddleware::new(state, reducer, middleware)
    .with_observer(collector.observer());
let runtime = Runtime::from_store(store);
// Run the runtime, then inspect collector.records().
```

The collector lives in `tui_dispatch_debug::observe` and is also re-exported by
`tui_dispatch::observe` when the facade's `debug` feature is enabled. State and
actions must implement `serde::Serialize`; effects need no additional traits.
`Store` supports the same attachment API.

Each record contains a sequence number, action ID, injecting parent ID, action
name and serialized payload, outcome, change flag, and full JSON state snapshot.
Applied actions are captured immediately after their reducer, before
`Middleware::after` and injected actions. Cancelled actions and dispatch-limit
rejections capture the unchanged state at that boundary. A reducer reporting no
change still produces an applied record. The change flag excludes child actions.
Direct `state_mut` edits and reducer panics do not produce records.

The oldest records are evicted at the configured capacity. IDs keep increasing
across eviction and are scoped to the collector; a parent may already be evicted.
Cloned collectors share history. Create a separate `observer()` handle for each
store to keep injection ancestry separate. `initial_state()` returns the snapshot
you supplied at construction; collection does not validate it against the store.

`collector.set_enabled(false)` retains history and skips new serialization.
Capacity zero disables collection. Re-enabling preserves sequence numbers; work
processed while disabled is absent from history. Serialization errors appear in
`serialization_errors`, with `null` for the failed payload or snapshot.

Capacity bounds record count, not bytes. Full snapshots scale with state size,
and `records()` clones the retained history. Use a modest capacity and avoid
copying large histories on every dispatch. No background workers or I/O run in
the collector. The overlay's existing action logging remains independent.

For custom observers, enable `tui-dispatch-core`'s `debug` feature and implement
`observe::DispatchObserver`. This borrowed callback requires no serialization.
Install it with `set_observer`; `clear_observer` removes it. Observer panics
propagate. With `debug` compiled out, the store has no observer field or callbacks.
