# Recording performance

Roadmap item 09, measured 2026-11-04 on the same Mac mini Mac16,10 / Apple M4
(4 performance + 6 efficiency cores), 16 GB RAM as the [transport baseline](transport.md).
macOS 26.6.2 (25G83), aarch64-apple-darwin; Rust 1.95.0 (`59807616e`, LLVM 22.1.2),
optimized Cargo bench profile. Source: uncommitted item 09 changes based on
`ecb678b3c23782d07f2db7f50640521aad607e6f`.

## Reproduction and scope

Run each command three times sequentially without other build or verification
jobs. Raw results include exact commands, sample ranges, and feature sets:
[default timing](results/recording-initial-default.txt),
[all-feature timing](results/recording-initial-all.txt),
[default memory](results/recording-memory-default.txt), and
[all-feature memory](results/recording-memory-all.txt).

```sh
cargo bench --offline -p tui-dispatch-debug --bench debug_recording
cargo bench --offline -p tui-dispatch-debug --bench debug_recording --all-features
TUI_DEBUG_RECORDING_COUNT=512 cargo bench --offline -p tui-dispatch-debug --bench debug_recording_memory
TUI_DEBUG_RECORDING_COUNT=4096 cargo bench --offline -p tui-dispatch-debug --bench debug_recording_memory
# Repeat both memory counts with --all-features too.
```

Default features are empty (core debug is enabled by the dependency); all features
add tasks, subscriptions, and json-schema. Tokio fs/io-util/time run the follower;
neither configuration enables testing-time. This capability has no prior file
recording equivalent: these are initial cost/scaling baselines. Store and collector
implementations are unchanged, and recording does not install a dispatch hook or
background producer. The [existing dispatch comparisons](README.md) measure the
compiled-out, disabled, and collection-only paths separately; no file I/O is
included in those paths.

Each throughput case writes 512 records per batch, one warmup plus seven measured
batches. State is a counter plus 0/4096/65536 bytes; action is an increment plus
0/256/4096 bytes. Writer creation/header, reader opening/header, finish/close, and
file deletion are outside timing. Timed writing includes borrowed serialization,
frame-buffer growth, append, and flush every 1 or 64 records (plus final flush).
Timed offline reading streams and drops records, including decoding, allocation,
and clean EOF. Files use the OS page cache; there is no fsync. These are local
hot-read measurements, not cold-disk or power-loss durability throughput.

The async catch-up case opens a follower before writing, then leaves it unpolled
until the writer has finished 512 records. Its elapsed time includes all reads,
decoding, and record destruction. Fifty live samples first drive a follower to
EOF for 2 ms, then measure append + flush through delivery. The 10 ms EOF poll
explains the residual wait; these samples exclude CLI rendering. CLI refresh is
limited to 250 ms independently. Idle following schedules approximately 100 EOF
checks per second; it uses no file-watcher service.

Allocation accounting runs separately, repeating the full workload with 512 and
4096 records per batch. Its allocator counts process-wide owned heap, including
Tokio and user record data, and conservatively includes old plus new buffers at
reallocation peaks. It does not measure kernel page cache or RSS. Streaming reads
never accumulate the whole history. A stalled or disconnected follower adds no
writer queue: the file itself holds unread records.

## Results and review

Ranges below are min–max per-run medians across three runs, in microseconds per
record. The raw logs retain all seven within-run samples' extrema.

| State / payload | Flush every | Default write | All-feature write | Default offline | All-feature offline |
| --- | ---: | ---: | ---: | ---: | ---: |
| 0 / 0 B | 1 | 1.721–1.766 | 1.559–1.798 | 0.522–0.539 | 0.510–0.519 |
| 0 / 0 B | 64 | 0.530–0.548 | 0.478–0.545 | 0.519–0.528 | 0.503–0.509 |
| 4096 / 256 B | 1 | 5.098–5.618 | 5.039–5.447 | 1.501–1.517 | 1.449–1.733 |
| 4096 / 256 B | 64 | 5.072–5.167 | 4.902–5.388 | 1.469–1.547 | 1.456–1.595 |
| 65536 / 4096 B | 1 | 42.884–52.240 | 37.938–46.597 | 12.323–14.116 | 13.915–18.613 |
| 65536 / 4096 B | 64 | 39.606–50.080 | 37.593–173.145 | 12.476–14.112 | 12.466–25.610 |

Files contain 195 / 4547 / 69827 encoded bytes per record respectively,
including the four-byte length prefix, plus a 12-byte header/finish overhead.
Default throughput spans approximately 105–108 MiB/s for scalar records flushed
every append, 340–351 MiB/s for batched scalar flushes, 772–855 MiB/s at 4 KiB,
and 1275–1681 MiB/s at 64 KiB. Large records already bypass the small writer
buffer; grouping explicit flushes therefore offers little consistent benefit.

Async catch-up averages span 0.714–0.752 / 3.882–3.996 / 21.447–24.417 us per
record for default features, and 0.755–0.859 / 3.735–30.202 / 21.473–26.901 us
with all features, in increasing size order. Live EOF delivery p50 spans
8.222–8.857 ms, p95 9.071–9.477 ms, and p99 9.104–10.088 ms across both builds.
These waits are accepted for a 10 ms polling API; replay catch-up does not wait
between records already on disk.

The all-feature 64 KiB batched-write row includes one 173.145 us run; its other
medians were 37.593 and 38.088 us. Likewise, one 4 KiB catch-up sample reached
30.202 us while the other two were 3.735 and 3.770 us. These isolated excursions
are consistent with filesystem/scheduler variability during repeated large-file
workloads, rather than a repeatable feature-dependent penalty. All samples are
retained. Do not interpret page-cache throughput as a bound on a synchronous
append's worst-case latency. Full-snapshot serialization and explicit disk I/O
are accepted opt-in costs; applications needing a strict dispatch latency budget
must choose a separate recording worker and its backpressure policy.

| Records per batch | Largest single file | Default peak owned heap | All-feature peak owned heap |
| --- | ---: | ---: | ---: |
| 512 | 35,751,436 B | 345,729 B | 345,729 B |
| 4096 | 286,011,404 B | 345,729–346,012 B | 345,729–346,012 B |

Eight times as many on-disk records leaves peak owned heap essentially constant;
the observed variation is only 283 bytes. The reported
post-runtime live ownership is 1868 B in every run (process I/O/runtime caches,
not retained recording history). The frame buffers are independently capped at
16 MiB in normal tests. Caller-returned decoded records, `read_all`/`recover`
results, and the CLI's configurable retained history are separate ownership;
these APIs cannot bound an application's own retained data.

Initial same-machine review triggers use the median of three run medians:
scalar writes 3 us (flush each) / 1 us (flush every 64); 4 KiB writes 10 us;
64 KiB writes 80 us; offline reads 1 / 3 / 30 us; async catch-up 2 / 10 / 50 us;
live p95 15 ms. A repeatable peak above 400 KiB for this workload, or growth
proportional to record count, requires review. These are manual baseline budgets,
not CI timing assertions. Resource bounds, cancellation, payload preservation,
and truncation behavior are enforced by the normal regression suite.

## Measured source identifiers

SHA-256 of the implementation, lockfile and benchmark sources:

```text
27723d1b1f7763d1d0ef4f025672d8e6c2f451d27a43440bd7a4c7ad3b61bd55  Cargo.lock
72c4af7204dd69bab60db8509d2378fac17a77a6750938c269af92ac867f2caa  tui-dispatch-debug/Cargo.toml
899b7e443221d4f4f46589181d1054b54e73b4c3895f2aa4bf2d3559da9cc79a  tui-dispatch-debug/src/recording.rs
b90035d4c35627525e2292311d7a8e0e967ac35644839e1de919e010a47ae3be  tui-dispatch-debug/src/records.rs
b68e69409f0d0b96c04c6f9550c9a8ee937a74bf279cf1afdb1fa6aa8a9c5a96  tui-dispatch-debug/benches/debug_recording.rs
ac230a5cdafcd70b46b2e2a878982774707d135bb8c18eba7185d1d716868c92  tui-dispatch-debug/benches/debug_recording_memory.rs
```
