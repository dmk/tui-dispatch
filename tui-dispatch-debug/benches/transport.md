# Debugger attachment performance

Roadmap item 08, measured 2026-11-02 on the same Mac mini Mac16,10 / Apple M4
(4 performance + 6 efficiency cores), 16 GB RAM as item 07. macOS 26.6.2
(25G83), aarch64-apple-darwin; Rust 1.95.0 (`59807616e`, LLVM 22.1.2),
optimized Cargo bench profile. Implementation: uncommitted changes based on
`399e5a1725c6a35c18130ee95e83a5d9e69e9238`.

## Reproduction

Run each command three times sequentially, with no other build or verification
jobs running. These runs set `DEVELOPER_DIR=/Library/Developer/CommandLineTools`.
Full commands, individual samples, and process-memory output are in the
[`results/transport-*.txt`](results/) files.

```sh
cargo bench --offline -p tui-dispatch-core --bench debug_dispatch --no-default-features
cargo bench --offline -p tui-dispatch-core --bench debug_dispatch --no-default-features --features debug
cargo bench --offline -p tui-dispatch-debug --bench debug_observation
cargo bench --offline -p tui-dispatch-debug --bench debug_observation --all-features
cargo bench --offline -p tui-dispatch-debug --bench debug_transport
cargo bench --offline -p tui-dispatch-debug --bench debug_transport --all-features
```

For the before measurements, archive the parent commit above into a temporary
directory and copy the working tree's ignored `Cargo.lock` into it. Run the first
four commands there, using the same target directory/toolchain as the working
tree. The existing observation benchmark also accounts for allocations; the
new transport does not change its allocation counts or retained sizes.

Core configurations are minimal and `debug` only. The debug crate's default
features enable core `debug`; all features add `tasks`, `subscriptions`, and
`json-schema`. Neither configuration enables `testing-time`. The transport adds
Tokio `net` and `io-util`; it does not add a background worker unless bound.

`debug_transport` uses two Tokio workers and a real local Unix socket. States
contain a counter and a string of 0 / 4096 / 65536 bytes; actions contain an
increment and a string of 0 / 256 / 4096 bytes. History capacities are 1 and 64.
After warming 256 dispatches, it measures nine batches of 256, with 2 ms sleeps
between batches so readers can run. Modes are collection disabled, collecting
without a server, an idle bound server, a concurrent reader requesting snapshots
with 1 ms between replies, and a client that reads a reply header but leaves its
body unread. Small replies can fit in the kernel buffer; the 4.5 MB case exercises
actual socket backpressure. No action queue is created for these clients.

Each connected mode then closes its original client and measures 100 sequential
snapshot round trips on a fresh connection, after one warmup request. These
latencies include history cloning, serialization, I/O, and client deserialization;
they exclude setup, app dispatch, and rendering. Latencies are individual request
percentiles; dispatch numbers are batch averages, not dispatch percentiles.

## Dispatch comparison

Ranges are minimum–maximum per-run medians over three runs, ns/dispatch.

| No observer | Before | After |
| --- | ---: | ---: |
| Plain, minimal | 0.91–2.27 | 0.91–0.93 |
| Middleware, minimal | 20.05–22.91 | 18.96–24.67 |
| Plain, debug | 1.00–2.29 | 0.99–1.00 |
| Middleware, debug | 20.71–21.26 | 19.35–25.04 |

| State / payload, capacity 64 | Before default | After default | Before all features | After all features |
| --- | ---: | ---: | ---: | ---: |
| Scalar, disabled | 3.09–8.79 | 3.58–8.95 | 3.09–8.30 | 3.58–9.28 |
| Scalar, collect | 184.89–598.63 | 189.62–615.56 | 204.91–621.42 | 180.99–598.63 |
| 4 KiB / 256 B, disabled | 21.81–26.53 | 21.65–27.34 | 21.65–31.41 | 21.64–24.58 |
| 4 KiB / 256 B, collect | 344.89–406.41 | 345.38–404.46 | 346.68–366.54 | 344.24–399.41 |
| 64 KiB / 4 KiB, disabled | 56.80–59.73 | 57.62–58.75 | 57.46–65.59 | 54.85–55.66 |
| 64 KiB / 4 KiB, collect | 1338.87–1361.00 | 1353.03–1360.02 | 1328.77–1471.19 | 1324.38–1327.64 |

No repeatable increase outside measured variation appears in the existing
dispatch workloads. The production dispatch callback is unchanged. Scalar
collection samples were noisy in both revisions (roughly 0.2–0.6 microseconds),
so these runs do not establish a small percentage regression threshold.

The transport benchmark's sleeps and concurrent scheduling produce substantially
different CPU/cache conditions from the tight observation benchmark. For 64 KiB
state, capacity 64, default-feature per-run dispatch medians were 1.80–2.18 us
without a server, 2.15–4.33 us with an idle server, 1.46–1.86 us with an active
reader, and 1.53–1.56 us with a blocked reader. All-feature ranges were
1.80–1.88 / 2.11–2.36 / 1.53–1.68 / 1.50–1.63 us respectively. The idle worker
does no collection work: these differences demonstrate scheduling variability,
not a speed benefit from readers. Bounded copying can contend on the history
lock; blocked network writes happen outside that lock.

## Initial transport baseline

Capacity 64, idle-server round trips; ranges across three runs. Sizes exclude
the four-byte length header. Other modes and capacity 1 are in the raw results.

| State / payload | Reply bytes | Default p50 | Default p95 | All features p50 | All features p95 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Scalar | 12,910 | 162–184 us | 203–309 us | 185–187 us | 238–394 us |
| 4 KiB / 256 B | 295,534 | 546–908 us | 1045–1112 us | 548–571 us | 1013–1104 us |
| 64 KiB / 4 KiB | 4,534,894 | 5.33–5.37 ms | 5.75–12.04 ms | 5.35–5.37 ms | 5.73–5.75 ms |

The default large idle case had one p95 outlier at 12.04 ms; its other runs were
5.75–5.76 ms. The corresponding p99 range was 6.71–12.58 ms. Large active/slow
cases were about 5.2–5.5 ms p50 and 5.6–5.9 ms p95 across both feature sets.
Default and all-feature results overlap within the observed scheduling noise.

This opt-in cost is accepted for the initial read-only full-snapshot interface.
At four polls per second, the largest workload transfers about 18 MB/s. It is
unsuitable as a per-action stream; cursor-based incremental delivery and gap
reporting remain item 11. History collection does not send or serialize any
transport data until a client requests it. Server and client reuse their byte
buffers across requests to avoid repeated large buffer allocations.

## Memory and deterministic bounds

Measure a single case per process, repeating each three times. Build first, then
run the executable printed by Cargo under `/usr/bin/time -l`; wrapping Cargo
itself would include compilation in the memory high-water mark. The raw files
record the exact executable paths and commands. For example:

```sh
TUI_DEBUG_TRANSPORT_CASE=65536,64,slow cargo bench --offline -p tui-dispatch-debug --bench debug_transport
TUI_DEBUG_TRANSPORT_CASE=65536,64,slow cargo bench --offline -p tui-dispatch-debug --bench debug_transport_memory
```

Repeat for sizes 0 / 4096 / 65536 and modes `no-server` / `reader` / `slow`, with
both default and all features. `debug_transport_memory` runs the same workload
with a counting allocator. Its timings are ignored. It accounts across all
threads, includes setup, and conservatively counts both old/new realloc storage
at the peak. RSS includes allocator caches, stacks, and runtime/OS overhead.

Capacity 64; ranges below combine both feature sets and three runs:

| State / payload | No-server peak RSS | Reader peak RSS | Slow-reader peak RSS | Reader peak owned heap | Slow-reader peak owned heap |
| --- | ---: | ---: | ---: | ---: | ---: |
| Scalar | 2.27–2.30 MiB | 2.66–2.75 MiB | 2.61–2.64 MiB | 0.282–0.293 MiB | 0.282 MiB |
| 4 KiB / 256 B | 2.84–2.88 MiB | 4.41–4.77 MiB | 4.12–4.45 MiB | 2.312 MiB | 2.312 MiB |
| 64 KiB / 4 KiB | 10.94–10.95 MiB | 32.78–37.12 MiB | 28.25–32.39 MiB | 33.194 MiB | 33.194 MiB |

No-server peak owned heap was 238,926 / 800,337 / 9,221,458 bytes for the three
sizes. Every instrumented run returned to 1,868 live bytes after dropping the
runtime, store, collector, clients, and server: process/standard-library lifetime
allocations, independent of workload size. No record queue accumulates behind a
slow consumer. These measurements include subsequent normal round trips after
the slow client disconnects, rather than isolating just its blocked reply.

The transport accepts at most eight clients. Each has one reply buffer and one
in-flight copied snapshot, not a queue proportional to dispatch count. Serialized
replies are limited to 16 MiB; copying JSON still scales with the app-configured
record count and state sizes, and Vec capacities can exceed the serialized length.
Large frame buffers remain allocated for reuse until the connection closes.
Owning-crate tests assert history bounds under slow/excess consumers, frame
rejection, malformed input handling, and continued collection after disconnect.

Initial same-machine review triggers are: existing observation workloads use
item 07's budgets, with noisy scalar collection reviewed against the paired
parent run; transport batch-average dispatch 6 us for 64 KiB states; capacity-64
snapshot p50 0.5 / 1.5 / 8 ms and p95 1 / 2 / 15 ms for the three sizes; isolated
large-workload peak RSS 50 MiB and accounted heap 40 MiB. These are workload-specific
manual review thresholds, not CI timing assertions or guarantees for larger states.

## Measured source identifiers

SHA-256 of the final transport and benchmark sources:

```text
7eb238efae403cdffb1ce8fbca6f64a4d517ddc02e2eda797d6f38f0f20b349d  Cargo.lock
baa15411d96927f24bed10a6af7167c2a144fb0589c4104a1c9f2fa26f941a26  tui-dispatch-debug/src/observe.rs
babbc6d5efda60b5e84aa909a3c065f0b51af33f1e51d67fb937e49743618b4c  tui-dispatch-debug/src/transport.rs
2210b10ca93e0326d0c49134674de9ce2a71827f0240699f2ac8af1d92b80d0c  tui-dispatch-debug/benches/debug_transport.rs
c05bb5e2a9a08a847de869419020ddf0c6994161ad7d2b1c086c445eb4c39a91  tui-dispatch-debug/benches/debug_transport_memory.rs
```
