# Reducer composition performance

Roadmap item 06, measured 2026-11-01 on a Mac mini Mac16,10, Apple M4
(4 performance + 6 efficiency cores), 16 GB RAM; macOS 26.6.2 (25G83),
aarch64-apple-darwin. Rust 1.95.0 (`59807616e`, LLVM 22.1.2), optimized Cargo
bench profile. The implementation is an uncommitted change based on
`591f7942c2e02ddb4cd5d5c4e9d2aa1ef6e8dc72`.

Measured source SHA-256 identifiers:

```text
7aaac0c50d1a9c2701fb80dd0d24fade599b0295b1bbee9fe56de3c45a58d5bc  src/store.rs
05f7de5a67ccaf8d15d4c0c6564d7f47c3745adbfa7335bedf2d80631de933d5  benches/reducer_composition.rs
```

Run each command three times, sequentially, without the verification suite:

```sh
cargo bench -p tui-dispatch-core --bench reducer_composition --no-default-features --offline
cargo bench -p tui-dispatch-core --bench reducer_composition --all-features --offline
```

The [before results](results/reducer-composition-before-2026-11-01.txt) were
captured before adding the helpers: explicit effect mapping with `collect`,
and flag OR plus `Vec::extend` for merging. Those manual operations remain in
the benchmark for direct comparisons in the
[after results](results/reducer-composition-after-2026-11-01.txt). Tagged mapping
was added afterward and has an initial cost/scaling baseline against equivalent
manual code, rather than a pre-implementation measurement.

Each workload has one warmup batch and nine measured batches of 128 independent
operations, using `Instant`. Input construction, payload filling, and output
destruction are excluded; consuming/deallocating input vectors and collecting
results into a preallocated output batch are included. These are steady-state
batch averages, not individual latency percentiles. Cold build/setup costs are
not included. A separate single-operation allocator probe reports allocations
(including reallocations), requested bytes, and live heap bytes including inputs.
For reallocations, peak bytes conservatively count old and new buffers together,
even if the allocator grows in place. These are owned heap bounds, not RSS or
runtime queue measurements. Timed batches do not update allocation counters.

## Results and review

Ranges are minimum–maximum per-run medians over three runs, in ns/operation.
Map inputs are `Vec<u8>` payload descriptors. Transparent wrapping uses a
one-variant enum with the same layout; tagged mapping widens each descriptor
from 24 to 32 bytes. Merge inputs are `u64` lists with exact capacity unless
marked as having spare capacity.

| Workload | Before, minimal | Helper, minimal | Helper, all features |
| --- | ---: | ---: | ---: |
| Transparent map, 16 empty payloads | 2.60–3.59 | 2.60–3.25 | 2.60–7.81 |
| Transparent map, 16 × 4 KiB payloads | 3.58–3.58 | 3.59–4.55 | 3.91–10.09 |
| Tagged map, 16 empty payloads | initial baseline | 25.06–28.65 | 24.73–87.23 |
| Tagged map, 256 empty payloads | initial baseline | 136.72–158.85 | 143.88–253.59 |
| Tagged map, 4096 empty payloads | initial baseline | 2352.54–2356.77 | 2209.96–2393.88 |
| Merge 0 + 0 | 3.91–4.56 | 4.55–5.21 | 4.55–5.20 |
| Merge 0 + 16 | 18.55–19.53 | 4.88–5.21 | 4.88–5.21 |
| Merge 16 + 16 | 34.18–35.16 | 35.16–40.04 | 34.83–42.97 |
| Merge 256 + 256 | 115.23–134.44 | 123.05–134.44 | 111.98–129.88 |
| Merge 4096 + 4096 | 1309.24–2088.54 | 1153.32–1612.63 | 1251.95–1437.50 |
| Merge 4096 + 4096, spare capacity | 548.83–697.92 | 497.72–679.03 | 458.34–518.23 |

The widening-map same-run manual medians for 16/256/4096 elements were
25.06–30.27 / 140.30–158.20 / 2303.06–2676.11 ns with minimal features and
24.74–87.24 / 145.51–271.16 / 2327.48–2514.00 ns with all features. Mapping
costs match the manual operations within the observed variation. Transparent
mapping can reuse the input allocation without visiting payload bytes; arbitrary
closures and different layouts can require per-element work and allocation.
Widening 4096 descriptors handles roughly 424–425 thousand lists/second with
minimal features. Growing merges of two 4096-element lists handle roughly
620–867 thousand pairs/second; spare capacity avoids allocation and reduces cost.

All-feature runs had substantial shared-machine variation, including an early
run where manual and helper tagged-map medians both rose to about 87 ns for 16
elements. The paired results do not show a repeatable feature-dependent cost.
Empty merging adds roughly 1 ns for choosing vector ownership; this is accepted
in exchange for eliminating the allocation and element copy for an empty left
result. Other merge timings overlap the baseline or same-run manual variation;
there is no measured substantial regression requiring an alternate algorithm.

Allocation counts and peaks were identical across feature sets and runs:

| Helper workload | New allocations | Requested bytes | Peak live bytes |
| --- | ---: | ---: | ---: |
| Transparent map, 4096 empty payloads | 0 | 0 | 98,304 |
| Tagged map, 4096 empty payloads | 1 | 131,072 | 229,376 |
| Tagged map, 16 × 4 KiB payloads | 1 | 512 | 66,432 |
| Merge 0 + 16 | 0 | 0 | 128 |
| Merge 4096 + 4096 | 1 | 65,536 | 131,072 |
| Merge 4096 + 4096, spare capacity | 0 | 0 | 98,304 |

Manual merge 0 + 16 allocates once (128 bytes, peak 256); the helper takes the
right vector directly. Other allocation counts match manual composition.
Payload buffers are moved, not cloned; normal regression tests check buffer
addresses and exactly-once destruction with non-Clone 64 KiB payloads. Composition
retains only the resulting effect list, with no hidden queue or per-effect storage.

## Regression review budgets

For this machine/toolchain with minimal features, investigate repeated run
medians above 12 ns for transparent maps, 40 ns for tagged maps through 16
elements (including 4 KiB payloads), 200 ns for 256 tagged elements, 3 µs for
4096 tagged elements, 7 ns for empty/one-empty merges, 50 ns for 16 + 16,
170 ns for 256 + 256, 2.2 µs for 4096 + 4096, and 850 ns with spare capacity.
These thresholds exceed the measured per-run variation and are review triggers,
not CI assertions or portable latency promises. Under all features, compare
against the paired manual operation when both show shared-machine slowdown;
investigate persistent helper-only increases outside the recorded variation.
Review new allocations, payload copies, or heap peaks beyond the recorded
operation-specific bounds regardless of timing.
