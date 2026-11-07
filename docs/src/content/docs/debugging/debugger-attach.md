---
title: Standalone debugger
description: Inspect and control a running app through a Unix socket
---

On Unix, an app can expose its `DebugCollector` through `DebugServer`. Run the
standalone debugger in another terminal; the app keeps its own terminal and
continues collecting when the debugger disconnects.

```rust
use serde_json::json;
use tui_dispatch_debug::{
    observe::{DebugCollector, DebugStoreExt},
    transport::DebugServer,
};

let collector = DebugCollector::new(json!({ "count": 0 }), 64);
let store = Store::new(state, reducer).with_observer(collector.observer());
let server = DebugServer::bind(socket_path, collector.clone()).await?;
// Use the store directly, or pass it to Runtime::from_store(store).
// Keep `server` alive while the runtime runs.
server.shutdown().await?;
```

Use a socket path inside a private directory owned by your user. Binding creates
a socket with mode `0600` and refuses every existing entry, including stale
sockets and symlinks. Shutdown removes the socket if that path still identifies
the socket this server created. Dropping the server also cleans up its path and
aborts its worker; `shutdown().await` additionally waits for client tasks to close.
After an abrupt process exit, remove a stale socket yourself before restarting.

Try the included headless counter in one terminal:

```sh
mkdir -m 700 /tmp/my-tui-debug
cargo run -p tui-dispatch-debug --example attach_counter -- /tmp/my-tui-debug/app.sock
```

In another terminal:

```sh
cargo run -p tui-dispatch-debug --bin tui-dispatch-debug -- /tmp/my-tui-debug/app.sock
```

The debugger reuses the live debug layer's action log, state tree, detail views,
syntax colors, scrollbars, and status bar. Standalone views fill the terminal;
the in-app debug layer keeps its modal layout. Reusable `DebugInspector` views
also default to modals; the standalone command explicitly opts into fullscreen
with `DebugInspector::new().fullscreen()`. It opens on the action log:

- `a` / `c` / `s`: toggle the action log / components / state tree.
- `P` / `p`: pause/resume the app when live controls are attached.
- `/`: search actions or filter state fields; Enter/Esc finishes typing.
- `j` / `k`, arrows, PgUp/PgDn: navigate; `n` / `N` move between action matches.
- Enter: open action or state-field details; Esc/Backspace returns to the list/tree.
- `b`: move the status bar; `w`: save the current JSON state locally.
- `q` / Ctrl+C: quit; Esc also quits from a list/tree. During search, `q` is text.

New snapshots preserve search, selection, and expanded state sections. Open
details stay fixed until you return to the list/tree. Action details include the
payload, state at that dispatch boundary, state diff, outcome, injecting parent
ID, change flag, and serialization errors. Diffs compare the selected record to
the preceding retained dispatch, including actions hidden by search. The `s`
tab continues to show the latest application state.

Diff paths are JSON pointers: `/items/0/name` addresses an array element, `~0`
escapes `~`, and `~1` escapes `/`. The empty pointer means the root value changed.
Arrays compare by index; added/removed containers appear as whole subtrees.
Missing `before` or `after` fields mean addition or removal; explicit JSON null
remains a captured value. An empty diff means the state is unchanged. If the
preceding record was evicted, a sequence gap exists, or state serialization
failed, the diff is marked unavailable. The oldest socket/recording entry usually
has no baseline; these sources do not include the state before their first record.

Embedded clients can use the same typed view:

```rust
use tui_dispatch_debug::debug::DebugInspector;

let mut inspector = DebugInspector::new(); // modal; add .fullscreen() if desired
inspector.update_snapshot(snapshot); // owned DebugSnapshot, history oldest first
inspector.set_search("Fetch");
if let Some(state) = inspector.selected_state() {
    println!("Selected boundary: {state}");
}
// Forward events with handle_event(), and draw with render(frame, status).
```

`selected_payload()`, `selected_record()`, and `selected_diff()` expose the
selection. `select_sequence(n)` selects a visible record and returns false if it
is hidden or absent. `with_initial_state(value)` supplies a known baseline for
sequence 1 of that source. `update_snapshot` replaces history, preserving search
and selection; an open detail retains one selected record and its predecessor
even after eviction. Start a new inspector when switching sources. Callers
control the retained history size. Clients may pass an `Arc<DebugSnapshot>` to
share history with the view without copying it. Diffs and historical state formatting happen
on demand when opening details, rather than for every record on each refresh.

Elapsed time shows `—` because the current transport does not carry timestamps.
Action/state data is inspected without injecting app actions. The live socket
also supports processing pause and app-side component highlighting. Frozen-frame
copying and buffer-cell inspection remain part of the in-app debug layer.

Enable live controls by connecting the collector to the app's runtime adapter:

```rust
use tui_dispatch_debug::debug::DebugLayer;

let debug = DebugLayer::<AppAction>::simple()
    .active(in_app_debug_enabled)
    .with_remote_debug(collector.clone());
let runtime = Runtime::from_store(store).with_debug(debug);
```

This works even when the in-app overlay is inactive. Space Invaders attaches it
when using `--debug-socket`. Plain snapshot servers and recordings remain
inspectable without enabling live controls.

`P` pauses processing at the next dispatch boundary, after the current reducer,
synchronous middleware injections, and emitted effects finish. Pending root
actions stay queued. In-flight tasks may finish; their completions are retained
until resume. Subscription pause policies still apply (interval ticks are
dropped while paused). App input is intercepted while processing is paused;
debugger controls and resize/highlight redraws continue. The status bar confirms
`PAUSED`; while editing a search query, `P` is text rather than a command.

Selecting a component sends its binding/mount ID back to the app. The app uses
its latest published bounds to draw a cyan outline on its own buffer. The
debugger never paints app-coordinate outlines on its UI. Changing selection,
opening details, or switching away from components updates/clears the outline;
highlighting also works while paused. Disconnecting releases that connection's
pause/highlight requests, so closing a debugger resumes an app it paused and
clears its outline. Snapshot-only clients do not affect another client's controls.

The `c` view shows app-published component types, bindings, last rendered areas,
and local debug entries, with the same filtering and details as the in-app view.
Publish the latest complete frame on the app's UI thread after rendering. For a
`ComponentHost`:

```rust
use tui_dispatch_debug::debug::ComponentSnapshot;

collector.set_components(
    host.mounted_components().iter()
        .map(ComponentSnapshot::from_mounted_info)
        .collect(),
);
```

A custom registry can pass its `Vec<ComponentSnapshot>` directly. Only the most
recent frame is retained; component updates do not need a dispatched action.
Socket workers copy these owned values without accessing UI objects on another
thread. State/history and component frames are sampled independently.

If an app has not published component snapshots, the components title says so.
Older socket servers and action recordings remain readable; they do not supply
component frames. Space Invaders enables its component registry for
`--debug-socket` and publishes it after each render, without needing `--debug`.

It polls every 250 ms and retries disconnected sockets every second, retaining
the last successful snapshot while disconnected. The counter exits after 60
ticks, or after the count specified by `--ticks`.

For scripts or terminals without an interactive UI, add `--once` to print one
JSON snapshot and exit. To embed a client:

```rust
use tui_dispatch_debug::transport::DebugClient;

let mut client = DebugClient::connect(socket_path).await?;
let snapshot = client.snapshot().await?;
println!("{} retained actions", snapshot.history.len());
```

The debugger also opens [recording files](../recording-files/) with
`--recording session.tuidbg`; add `--follow` to watch an active recording.

Late attachment includes the collector's retained history. Each snapshot contains
a consistent history and its last state, or the supplied initial state if history
is empty. Disabled collection retains its last observed state; it does not track
unobserved mutations. Reconnect with a new client after an I/O error, timeout, or
cancelled snapshot request. Wire records use owned `SnapshotOutcome` values so
dispatch rejection details can cross process boundaries.

The server allows eight simultaneous clients and limits each JSON reply to
16 MiB. Oversized snapshots and malformed requests close that connection. Writes
and client snapshot requests have five-second I/O deadlines. There is no queue
of actions per client: a slow reader retains at most one serialized reply, and
collection never waits for socket I/O. Copying a requested history briefly holds
the collector's lock; serialization and network writes happen after it is released.
Snapshot copies still scale with state size and the history capacity.

This transport returns the full retained history on every request. Its wire
format is currently unversioned. Cursor/gap negotiation, stepping,
action injection, and state restoration remain later roadmap items. With the
facade's `debug` feature, the transport is also available at
`tui_dispatch::transport`.
