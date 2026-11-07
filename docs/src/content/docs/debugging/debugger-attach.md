---
title: Standalone debugger
description: Attach a read-only debugger through a Unix socket
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
the in-app debug layer keeps its modal layout. It opens on the action log:

- `a` / `s`: toggle the action log / state tree.
- `/`: search actions or filter state fields; Enter/Esc finishes typing.
- `j` / `k`, arrows, PgUp/PgDn: navigate; `n` / `N` move between action matches.
- Enter: open action or state-field details; Esc/Backspace returns to the list/tree.
- `b`: move the status bar; `w`: save the current JSON state locally.
- `q` / Ctrl+C: quit; Esc also quits from a list/tree. During search, `q` is text.

New snapshots preserve search, selection, and expanded state sections. Open
details stay fixed until you return to the list/tree. Action details include the
payload, outcome, injecting parent ID, change flag, and serialization errors.
Elapsed time shows `—` because the current transport does not carry timestamps.
This interface is read only; frame freezing and local component/cell inspection
remain part of the in-app debug layer.

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

This first transport returns the full retained history on every request. It is
read only and its wire format is currently unversioned. Recording, state diffs,
cursor/gap negotiation, and execution controls are later roadmap
items. With the facade's `debug` feature, the transport is also available at
`tui_dispatch::transport`.
