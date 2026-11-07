---
title: Recording files
description: Save dispatch records, follow a running session, and recover interrupted recordings
---

`RecordingWriter` saves dispatch records without requiring a socket or terminal.
Append records in dispatch order, flush to publish them to followers, and finish
when the session closes:

```rust
use tui_dispatch_debug::recording::{RecordingReader, RecordingWriter};

let mut writer = RecordingWriter::create("session.tuidbg")?;
for record in collector.records() {
    writer.append(&record)?;
}
writer.flush()?;
// More records may be appended here.
writer.finish()?;

let records = RecordingReader::open("session.tuidbg")?.read_all()?;
```

For continuous capture, append each newly collected record once, before the
collector evicts it. The writer is explicit synchronous file I/O: it is not
installed into Store dispatch automatically. A background producer needs its
own queue and backpressure policy. Payloads and state may contain sensitive
data; creation refuses existing paths and uses mode `0600` on Unix.

Open a recording in the standalone debugger, using the same action/state views
and keys as [socket attachment](../debugger-attach/):

Try the headless counter in one terminal. It records 60 increments over 30
seconds; choose a new path each time because existing files are never replaced:

```sh
cargo run -p tui-dispatch-debug --example record_counter -- /tmp/counter-session.tuidbg
```

In another terminal:

```sh
cargo run -p tui-dispatch-debug -- --recording /tmp/counter-session.tuidbg
cargo run -p tui-dispatch-debug -- --recording /tmp/counter-session.tuidbg --follow
cargo run -p tui-dispatch-debug -- --recording /tmp/counter-session.tuidbg --once
```

The inspector retains the latest 1024 records by default; change this with
`--history-limit 256`. It streams through the file, keeping memory independent
of file length. `--once` prints a JSON snapshot and exits. An interrupted file
shows its complete records with an incomplete-tail status (or a stderr notice
with `--once`). Empty recordings have a `null` state: files contain dispatch
boundaries, not a separate initial state. Recording inspection also works on
platforms without Unix sockets.

To follow from Rust, inside a Tokio runtime:

```rust
let mut follower = RecordingReader::follow("session.tuidbg")?;
while let Some(record) = follower.next().await? {
    println!("{}: {}", record.sequence, record.name);
}
```

At live EOF, `next()` waits for appended bytes or the finish marker. It checks
every 10 ms using asynchronous file reads. Cancelling a call preserves partial
prefixes and payloads for the next call. A slow follower reads from disk without
adding a producer queue. It follows the opened file; rotation and replacement
are unsupported. The inspector refreshes at most every 250 ms while following.

`finish()` writes a marker and flushes. Dropping a writer leaves an unfinished
file. Following a crashed writer waits until cancelled; it cannot distinguish
a crash from an idle producer. For offline recovery use:

```rust
let recovery = RecordingReader::open("session.tuidbg")?.recover()?;
println!("incomplete tail: {}", recovery.incomplete_tail);
```

Recovery preserves complete records before a partial frame or missing finish
marker. Corrupt JSON and oversized frames remain errors. `read_all()` requires
a clean finish; `read_next()` streams individual records. Both return owned
`records::SnapshotRecord` values, including rejection details, matching the
socket debugger's representation. IDs, order, payloads, outcomes, and
serialization diagnostics are preserved as supplied.

Format version 1 uses an eight-byte `TUIDBG\x01\n` header, little-endian u32
lengths, JSON records up to 16 MiB each, and a zero-length finish marker.
Unsupported headers fail explicitly. Streaming readers and the writer each
retain at most one bounded frame plus I/O buffers; `read_all()` and `recover()`
accumulate their returned records. `flush()` publishes to the OS page cache;
it does not fsync or guarantee survival of power loss.
