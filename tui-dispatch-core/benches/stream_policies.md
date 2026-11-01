# Stream delivery performance

Roadmap item 05, measured 2026-11-01 on Mac mini Mac16,10, Apple M4,
16 GiB RAM, Darwin 25.6.0 / aarch64-apple-darwin. Rust 1.95.0
(`59807616e`, LLVM 22.1.2); Cargo's optimized bench profile.
Before: `e07bd0c7f9d19594f3d1b35546d27cfb8d49cb4a`.
After: the uncommitted item 05 implementation based on that commit.

Measured implementation/benchmark SHA-256 identifiers:

```text
525474525d5c1f1006cabfadff1f37a89bb8c8db22ab19de778f81864d47d7a0 src/subscriptions.rs
3ffa1480f537c0be6c06dc7075856ca7eac1ac66255b04663f0e6e891238f866 src/action_channel.rs
d78b2efa1b04b7ba882fcf89d572093296c7a361a13856f20f6f897aec00fc78 benches/stream_forwarding.rs
d51ca78e779838de3bf4e0ab3d6328b046ac444b108a157998577a9e42a3e4d1 benches/stream_policies.rs
```

## Reproduce

Run each command three times sequentially, without verification running:

```sh
cargo bench -p tui-dispatch-core --bench stream_forwarding --features subscriptions --offline
cargo bench -p tui-dispatch-core --bench stream_forwarding --all-features --offline
cargo bench -p tui-dispatch-core --bench stream_policies --no-default-features --features subscriptions --offline
cargo bench -p tui-dispatch-core --bench stream_policies --no-default-features --features subscriptions,testing-time --offline
cargo bench -p tui-dispatch-core --bench stream_policies --all-features --offline
```

Each executable uses a current-thread Tokio runtime, one warmup and seven measured
samples per workload, each delivering 100,000 source items. Payload sizes are 0
and 1,024 bytes. All timings use real `Instant`, including `testing-time` builds.
The recorded repeat runs reused the Cargo-built executable directly. The original
forwarding baseline used the same benchmark under the name `stream_policies`;
copy `stream_forwarding.rs` and its manifest entry into the before checkout to
reproduce that comparison. The new policy workloads have no prior equivalent.

Raw data: [before forwarding](results/stream-forwarding-before-2026-11-01.txt),
[after forwarding](results/stream-forwarding-after-2026-11-01.txt),
[bounded/batched delivery](results/stream-policies-2026-11-01.txt).

## Existing unbounded forwarding

This benchmark has no allocation instrumentation. Channel/manager setup and latency
vector allocation are outside the timer; spawning the single subscription and
creating/destroying source payloads are included. Per-item latency starts at source
consumption and ends at receipt. Setup is amortized over 100,000 items. Cold-start
samples are warmed up, not treated as steady-state latency.

Ranges are the minimum–maximum of the three run medians, in ns/source item:

| Payload | Subscriptions before | Subscriptions after | All features before | All features after |
| --- | ---: | ---: | ---: | ---: |
| 0 bytes | 46.44–62.78 | 53.25–55.38 | 49.30–69.49 | 53.78–55.01 |
| 1 KiB | 83.94–85.89 | 95.74–98.27 | 91.75–94.13 | 95.70–97.05 |

There is measurable additional cost for pause/delivery synchronization and
cooperative scheduling of immediately-ready sources. The 1 KiB minimal build
adds about 10–14 ns/item across these run-median ranges. This cost is accepted:
returning from pause now establishes an enqueue boundary, and an infinite ready
source cannot monopolize the executor. No batch Vec is allocated on the legacy
stream path. An earlier implementation polled channel closure per item; review
removed that overhead by watching closure around the entire forwarding future.

These saturated **unbounded** workloads build a large backlog: per-sample p99
ranges from 2.36–6.60 ms before and 2.59–7.20 ms after. CPU warmup and allocator
state cause substantial variation, especially for the first empty-payload run.
This workload does not promise bounded latency or queue memory.

## Bounded delivery and batching

The instrumented benchmark varies channel capacity (1/64 actions), batch size
(1/32 items), and initial pause (off/on, with 256 source-item buffer slots).
The source is continuously ready; the consumer receives as quickly as scheduled.
The initially paused case fills the buffer before timing resume and delivery.
Setup time/allocations, steady delivery time/allocations, and peak additional live
heap bytes are reported separately. Peak heap includes setup, task state, channel,
batch containers and payloads; it excludes the preallocated latency vector and
runtime. This measures allocator-requested bytes, not RSS. Payload construction
and destruction during delivery are included. Atomic allocation and item-lifetime
counters add measurement overhead, so do not compare these times directly with
the uninstrumented forwarding benchmark.

Ranges below cover run medians for both initially active and initially paused
cases, across three runs, in ns/source item:

| Payload | Channel capacity | Batch size | Subscriptions | + testing-time | All features |
| --- | ---: | ---: | ---: | ---: | ---: |
| 0 | 1 | 1 | 401.54–413.74 | 429.37–457.36 | 437.22–461.36 |
| 0 | 1 | 32 | 147.68–152.65 | 149.46–162.16 | 151.65–158.91 |
| 0 | 64 | 1 | 225.00–239.17 | 234.50–243.53 | 233.88–265.09 |
| 0 | 64 | 32 | 142.59–146.72 | 142.00–146.98 | 147.70–149.02 |
| 1 KiB | 1 | 1 | 430.17–458.29 | 484.27–491.26 | 497.98–513.62 |
| 1 KiB | 1 | 32 | 172.02–182.71 | 179.54–185.57 | 180.60–187.09 |
| 1 KiB | 64 | 1 | 255.10–267.42 | 264.58–272.18 | 262.67–285.33 |
| 1 KiB | 64 | 32 | 180.28–191.07 | 183.11–191.41 | 190.75–200.22 |

Production throughput is about 2.18–7.01 million source items/second. Size-32
batches reduce action and Vec allocations from 100,000 to 3,125 per workload.
With empty payloads, initially active production runs report 100,012–100,013
steady allocations for size 1 versus 3,140–3,141 for size 32 (including queue
storage). Nonempty payloads add exactly 100,000 source allocations. Every workload
reports **zero payload clones**. The isolated `testing-time` build reproduces
the all-features allocation counts: e.g. capacity 1 / size 1 adds 103,125 steady
allocations for tracked task polling. Those allocations are absent in production.

Production p99 spans 0.75–1.33 microseconds for capacity 1 / size 1,
8.88–12.75 microseconds for capacity 1 / size 32, 14.04–17.88 microseconds
for capacity 64 / size 1, and 227–310 microseconds for capacity 64 / size 32.
Larger queues/batches improve throughput while retaining items longer. Instrumented
all-features runs include a 1.43 ms p99 outlier in the 1 KiB / capacity 64 /
size 32 workload; its run medians span 191–200 ns/item. These shared-machine tails
are recorded, not removed or interpreted as hard delivery guarantees.

Peak live source items are 2, 64, 65, and 1,952 respectively for active
(capacity, batch) combinations (1,1), (1,32), (64,1), and (64,32).
Initial pause reaches exactly 256 items and stops polling; the last combination
later reaches 2,080 during draining. Peak additional heap in production ranges
from 5,137 bytes to 2,228,081 bytes across all workloads, including payloads.
The bound scales with configured capacity and batch size, not the 100,000-item
source length. Normal tests independently assert polling bounds and destruction
of retained items on closure/cancellation/replacement.

## Review budgets

For this machine/toolchain, investigate repeated run medians above 85 ns/item
(empty) or 120 ns/item (1 KiB) for uninstrumented legacy forwarding.
For instrumented production, review medians above 440/490 ns for capacity 1 /
size 1 (empty/1 KiB), 165/200 ns for capacity 1 / size 32, 255/285 ns for
capacity 64 / size 1, and 160/210 ns for capacity 64 / size 32.
For tracked builds the corresponding thresholds are 490/570, 175/220,
285/310, and 170/220 ns. These workload-specific investigation thresholds exceed
observed median variability; they are not CI assertions or portable promises.

Review any payload clone, source polling past the documented storage bound,
or production allocation counts above this baseline regardless of elapsed time.
The accepted new-feature costs are bounded channel synchronization, one Vec per
action batch, a reusable source-item queue, and timer/watch bookkeeping. The
repeatable benchmarks remain separate from `make verify`.
