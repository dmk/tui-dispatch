# Debug observation performance

For item 08's dispatch comparison, Unix socket latency, and slow-client memory
baseline, see [Debugger attachment performance](transport.md).

Roadmap item 07, measured 2026-11-02 on a Mac mini Mac16,10, Apple M4
(4 performance + 6 efficiency cores), 16 GB RAM; macOS 26.6.2 (25G83),
aarch64-apple-darwin. Rust 1.95.0 (`59807616e`, LLVM 22.1.2), optimized Cargo
bench profile. Implementation: uncommitted changes based on
`fc0860532e3ec881f7567b1baae2e3b8c0b1ae68`.

## Reproduction

Run each command three times sequentially with no verification/build jobs running:

```sh
cargo bench --offline -p tui-dispatch-core --bench debug_dispatch --no-default-features
cargo bench --offline -p tui-dispatch-core --bench debug_dispatch --features debug
cargo bench --offline -p tui-dispatch-debug --bench debug_observation
cargo bench --offline -p tui-dispatch-debug --bench debug_observation --all-features
```

These measurements used `DEVELOPER_DIR=/Library/Developer/CommandLineTools` to
select the installed command-line toolchain. Full commands (including the shared
`--target-dir`) and individual sample ranges are in [results/](results/).

The before measurements use an archive of the parent commit above, adding only
`debug_dispatch.rs` and its Cargo benchmark declaration. **Copy the working tree's
`Cargo.lock` into that archive** before running, so dependencies match exactly.
The current root lockfile is not tracked and is absent from `git archive`.
Core configurations are minimal and `debug` only. Debug-crate configurations are
its default empty feature set (which enables core `debug`) and all its features
(`tasks`, `subscriptions`, `json-schema`); neither enables `testing-time`.
No tasks, event polling, rendering, network, or transport run in these workloads.

The core benchmark warms up 1,000,000 dispatches, then measures eleven batches of
1,000,000 with `Instant`. The collector benchmark warms up 256 dispatches, then
measures nine batches of 256. Setup and compilation are outside timed batches;
caller action cloning, snapshot serialization, old-record eviction/destruction,
and any history-read cloning/destruction are included. Results are batch averages,
not individual dispatch latency percentiles. Known absent observers can be
optimized away in this workload; these numbers are not a universal cost guarantee.

Collector state contains a counter and a string of 0/4096/65536 bytes. Actions
contain a counter increment and a string of 0/256/4096 bytes respectively. History
capacities are 1 and 64. `read_every=0` models no consumer; 1 models a consumer
copying all retained history every dispatch; 256 models an infrequent consumer.
These are in-process `records()` readers, not attached socket clients. Socket,
recording, diff, and asynchronous slow-client measurements belong to later items.

## Results and review

Ranges below are minimum–maximum per-run medians over three runs, in ns/dispatch.

| No observer | Before | After |
| --- | ---: | ---: |
| plain, minimal | 0.91–0.92 | 0.91–0.92 |
| middleware, minimal | 21.10–22.51 | 21.36–22.25 |
| plain, debug | 0.91–2.21 | 0.91–0.92 |
| middleware, debug | 19.09–21.92 | 19.26–22.12 |

There is no repeatable regression in the compiled-out or unattached configurations.
An isolated parent debug/plain run reached 2.21 ns; its other two medians were
0.91–0.92 ns, matching the implementation. Middleware variation overlaps before
and after. With debug compiled out, there is no observer field or callback code.

| State / payload | Collection / history reads (capacity 64) | Default | All features |
| --- | --- | ---: | ---: |
| 0 / 0 B | disabled | 3.42–3.42 | 3.42–3.58 |
| 0 / 0 B | collect, no reader | 187.01–189.78 | 181.32–182.13 |
| 0 / 0 B | copy history each dispatch | 10640.79–11134.77 | 10859.05–12218.91 |
| 0 / 0 B | copy history each 256 dispatches | 223.96–232.26 | 224.45–254.39 |
| 4096 / 256 B | disabled | 22.13–22.30 | 22.79–26.20 |
| 4096 / 256 B | collect, no reader | 345.86–362.96 | 342.45–354.16 |
| 4096 / 256 B | copy history each dispatch | 19161.79–19459.31 | 19141.77–19397.46 |
| 4096 / 256 B | copy history each 256 dispatches | 421.23–423.67 | 413.90–447.75 |
| 65536 / 4096 B | disabled | 58.11–59.08 | 56.80–59.89 |
| 65536 / 4096 B | collect, no reader | 1331.05–1349.28 | 1314.94–1360.84 |
| 65536 / 4096 B | copy history each dispatch | 76247.56–78192.38 | 76340.00–81876.95 |
| 65536 / 4096 B | copy history each 256 dispatches | 1786.30–1802.41 | 1755.86–1822.59 |

Disabled collection adds an atomic-enabled check and virtual call, and performs no
serialization. Its payload-bearing rows include the caller's action string clone;
that clone is independent of collection. Empty disabled dispatches allocate nothing.
Default and all-feature results are close relative to observed sample variation;
there is no repeatable extension-dependent penalty requiring a different design.

Enabled collection has no previous equivalent. Its initial baseline is about
0.19 microseconds for scalar records, 0.35 microseconds for 4 KiB state, and
1.3–1.4 microseconds for 64 KiB state with capacity 64 and no reader. This explicit
opt-in cost is accepted for retaining full payload/state snapshots at every
boundary, including intermediate states. Full-history copying each dispatch adds
substantial work: roughly 76–82 microseconds for 64 KiB snapshots. The API docs
call out this cost. It is unsuitable as a future transport's per-action polling
strategy; bounded incremental delivery must be measured when transport is added.
Infrequent reads do not accumulate an unbounded queue or retain extra history.

A separate counting allocator probe excludes allocator instrumentation from timing.
It reports steady-state allocations/requested bytes per dispatch and live owned
heap bytes after warmup. Retained/peak totals include the store, caller action,
collector, and observer. Reallocation peaks conservatively include both old and
new buffers; these are owned heap estimates, not RSS. Every probe destroys all
tracked ownership and asserts that live bytes return to zero.

| State / payload | History capacity | Collect allocations/op | Retained bytes | No-reader peak | Full-history-read peak |
| --- | ---: | ---: | ---: | ---: | ---: |
| 0 / 0 B | 1 | 7 | 2,321 | 3,610 | — |
| 0 / 0 B | 64 | 7 | 95,048 | 96,337 | 189,832 |
| 4096 / 256 B | 1 | 11 | 11,025 | 16,922 | — |
| 4096 / 256 B | 64 | 11 | 377,928 | 383,825 | 751,240 |
| 65536 / 4096 B | 1 | 11 | 141,585 | 216,602 | — |
| 65536 / 4096 B | 64 | 11 | 4,621,128 | 4,696,145 | 9,172,360 |

Counts/heap totals are identical across runs and feature sets. Full snapshots scale
with both capacity and state size, as expected; capacity is a record-count bound,
not a byte bound. JSON object overhead is appreciable even for scalar state.
Collection allocates 1289/6153/79113 requested bytes per dispatch for the respective
payload sizes. A full read adds a temporary owned copy of the retained history.

Initial same-machine review budgets for per-run medians are: unattached plain
5 ns and middleware 30 ns; disabled scalar 10 ns and disabled 64 KiB 100 ns;
no-reader collection 300 ns / 750 ns / 2500 ns for the three sizes; full-history
copying at 64 KiB 120 microseconds. These are workload-specific review triggers
above observed ranges, not CI wall-clock assertions or a universal percentage.
Retained-count bounds and disabled serialization are normal regression tests.

## Measured source identifiers

SHA-256 of the implementation and benchmarks:

```text
7eb238efae403cdffb1ce8fbca6f64a4d517ddc02e2eda797d6f38f0f20b349d  Cargo.lock
5456559c4d3f98d2591ea3ead819c50c959b7dff990864ba677abf8de4437792  tui-dispatch-core/src/store.rs
36796a096f017b62bf24a393f3903bfb08ac9d3f73211192b082e928e20cf291  tui-dispatch-core/src/observe.rs
92014793939807729978316d45d3973bd4fddfba3f90978af6cd41edd1084300  tui-dispatch-debug/src/observe.rs
467b5ea040616f94c9b41409cc2effa83bbbe69c3f5b87bff6e9a77ee0a1b69f  tui-dispatch-core/benches/debug_dispatch.rs
472a99fa314423908de2830ba4811a4c28956b1b98daad320b6da69300daf20d  tui-dispatch-debug/benches/debug_observation.rs
```
