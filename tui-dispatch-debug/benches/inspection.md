# Debugger inspection performance

Roadmap item 10, measured 2026-11-06 on Mac mini Mac16,10 / Apple M4
(4 performance + 6 efficiency cores), 16 GB RAM, macOS Darwin 25.6.0,
aarch64-apple-darwin. Rust 1.95.0 (`59807616e`, LLVM 22.1.2), optimized Cargo
bench profile. Baseline commit: `7a51246e3465faee963b31a88cf12a0cfe795c73`;
results use the uncommitted item 10 changes based on that commit.

## Reproduction and scope

```sh
cargo bench --offline -p tui-dispatch-debug --bench debug_inspection
cargo bench --offline -p tui-dispatch-debug --bench debug_inspection --all-features
cargo bench --offline -p tui-dispatch-debug --bench debug_inspection_memory
cargo bench --offline -p tui-dispatch-debug --bench debug_inspection_memory --all-features
```

Timing and allocation probes run separately, sequentially without build/test jobs
competing for CPU. Default features are empty (the dependency enables core debug);
all features add tasks, subscriptions, and json-schema. Neither enables testing-time.
The new functionality is entirely consumer-side: core dispatch, collection,
serialization, socket polling, and recording production are unchanged. Existing
[dispatch](README.md), [transport](transport.md), and [recording](recording.md)
baselines describe those paths, including disabled, unattached, and slow clients.

The first benchmark section compares the existing display-only adapter before
and after the change using the same workloads. Before and after results each have three runs with seven samples per run,
after one warmup. The before comparison was repeated from a temporary `git
archive` of the baseline commit, with the same working-tree Cargo.lock copied
in and `--locked`; only the display benchmark and its manifest entry were added. Display update excludes caller cloning; search measures a
complete `/`, `a`, Backspace, Enter cycle; drawing uses a hot 120x30 TestBackend.
History is 64 or 1024 actions; latest state contains a scalar and a 0/4096/65536
byte string. Display payloads are scalar, matching the existing view comparison.
Terminal creation/setup and compilation are excluded; first-render/cold-terminal
cost and real terminal I/O are not measured.

New typed-view workloads retain 64 or 1024 dispatch records, each with a scalar
and a 0/4 KiB/64 KiB state string plus a 0/256 B/4 KiB action string. Cloning is
measured separately (including drop), as a reference cost avoided by sharing.
Typed update excludes input construction/cloning and includes releasing the
previous input; shared refresh includes the Arc clone and view refresh but
shares records with the source, as the CLI does. Both still build searchable
payload/metadata strings and the latest state tree; historical states are not
formatted. Opening and closing a detail includes navigation, one selected-record
and predecessor copy, JSON diff computation/formatting, and list restoration.
The selected diff changes one scalar beside the unchanged string; structured
array diffs change one index in 64/4096 scalar elements. Timings use real elapsed
Instant measurements, with 8 updates/detail cycles, 256 selected diffs, or 64
array diffs per sample. New typed inspection has no prior equivalent.

Memory probes move an input into the view and count allocator-requested owned
bytes (not RSS); no terminal/socket/watch snapshot copy is present. Peak includes
old plus new buffers conservatively during realloc. They report update allocation
counts, diff allocations, retained history, open details, and ownership after 16
refreshes with a slow viewer leaving a detail open. Every probe drops all view
ownership and asserts that live bytes return to baseline. Unit tests additionally
assert that Arc snapshots share record addresses, and that an open detail does
not retain its old full snapshot after replacement. Collection history remains
source-bounded, with no growing inspector queue.

Raw results: [before default](results/inspection-before-default.txt),
[before all features](results/inspection-before-all.txt),
[after default](results/inspection-after-default.txt),
[after all features](results/inspection-after-all.txt),
[memory default](results/inspection-memory-default.txt), and
[memory all features](results/inspection-memory-all.txt).

## Results and review

Ranges below are min–max per-run medians (microseconds). Each raw run also
reports the seven within-run sample extrema.

| History / state bytes | Operation | Before default | After default | Before all | After all |
| --- | --- | ---: | ---: | ---: | ---: |
| 64 / 0 | display_update | 12.19–32.93 | 12.18–32.77 | 12.29–28.85 | 11.70–33.00 |
| 64 / 0 | search | 44.69–85.45 | 45.55–96.34 | 44.36–78.70 | 43.13–85.31 |
| 64 / 0 | list_draw | 375.71–380.84 | 374.04–391.93 | 369.19–380.21 | 381.22–384.17 |
| 1024 / 4096 | display_update | 187.44–191.16 | 192.28–217.80 | 188.85–195.15 | 188.43–195.15 |
| 1024 / 4096 | search | 699.13–778.12 | 710.20–768.09 | 699.37–711.54 | 682.04–710.10 |
| 1024 / 4096 | list_draw | 2883.49–2925.84 | 2813.61–2895.04 | 2934.09–2966.55 | 3046.84–3135.40 |
| 1024 / 65536 | display_update | 206.15–208.16 | 207.79–214.89 | 207.88–213.06 | 204.44–211.01 |
| 1024 / 65536 | search | 681.55–701.12 | 700.84–716.73 | 687.75–716.46 | 693.08–724.45 |
| 1024 / 65536 | list_draw | 2886.20–3029.35 | 2838.20–2884.46 | 2910.92–3101.83 | 3072.23–3113.07 |

Initial typed inspection costs (combined default/all-feature ranges):

| History / state / payload bytes | Shared refresh | Selected diff | Open + close detail |
| --- | ---: | ---: | ---: |
| 64 / 0 / 0 | 50.91–53.70 | 0.26–0.27 | 10.92–11.52 |
| 1024 / 4096 / 256 | 1130.86–1199.83 | 0.35–0.36 | 169.85–183.99 |
| 1024 / 65536 / 4096 | 5147.83–5366.96 | 1.48–1.54 | 815.09–968.00 |

| History / state bytes | Retained bytes | Update peak | Update allocations | Open detail | After 16 refreshes |
| --- | ---: | ---: | ---: | ---: | ---: |
| 64 / 0 | 152,967 | 153,195 | 1,426 | 128,906 | 136,458 |
| 64 / 4096 | 536,449 | 540,520 | 1,685 | 473,414 | 504,006 |
| 64 / 65536 | 6,434,689 | 6,500,200 | 1,685 | 5,645,894 | 6,168,006 |
| 1024 / 0 | 2,253,447 | 2,253,675 | 22,546 | 1,831,946 | 1,952,778 |
| 1024 / 4096 | 8,143,489 | 8,147,560 | 26,645 | 6,845,898 | 7,335,370 |
| 1024 / 65536 | 98,828,929 | 98,894,440 | 26,645 | 82,059,978 | 90,413,770 |

Memory/allocation counts are identical across all three runs and both feature
sets. The single-scalar selected diff allocates nine times regardless of retained
history size or unchanged string size. Array diffs take 2.43–2.51 microseconds for
64 elements and 136.78–141.51 microseconds for 4096. Traversal scales with object
keys/array elements and compared string bytes; output clones changed subtrees
only. Changing a whole large value therefore has a larger cost than this sparse
diff workload.

Existing display/search ranges are broadly similar. The first scalar workloads
show wide process-start variability even after one warmup (roughly 12–33 us for
update, 43–96 us for search); retain the raw ranges rather than infer a feature
penalty from a single run. The all-feature 1024/4 KiB draw rises from 2.93–2.97 ms
to 3.05–3.14 ms; default rendering improves slightly, and 64 KiB all-feature ranges
overlap. Review found no additional diff/serialization work in rendering: the
render implementation is unchanged. The small all-feature draw increase remains
recorded and accepted at less than 0.21 ms per frame, within the 4 ms review
budget for this fixture. These measurements do not isolate a cause from code
layout/optimizer and machine variability, so this is not a claim of zero cost.

Shared refresh avoids the separately measured 1.89–2.46 ms clone/drop of the
1024-record 64 KiB snapshot and another entire retained history copy. Its
5.15–5.37 ms view refresh is accepted for the existing 250 ms polling interval:
it includes searchable payload/metadata formatting for 1024 actions. Opening a
large-state detail remains below 1 ms in these runs. No per-record state diff is
computed during refresh, and no new work is installed on dispatch/collection.
Retained memory scales with source history and payload/state sizes, not the
number of refreshes. List restoration/cloning and new formatted strings have
different spare capacity, explaining the retained-byte variation when opening
and refreshing details. A full 1024-record 64 KiB history still costs about
94.3 MiB including display strings; use a smaller history limit for large states.
The source and view share this history rather than keeping independent copies.

Initial same-machine review triggers (per-run medians, not CI timing assertions):
64-record display update/search/draw 50/150/600 us; 1024-record display
update/search/draw 300/1000/4000 us. Typed shared refresh: 100/2000/8000 us for
the three timing fixtures; selected diff: 0.5/1/3 us; open+close details:
25/300/1500 us. Array diff: 5 us at 64 elements, 250 us at 4096. Retained memory
for the largest fixture should remain below 110 MiB, including an open detail
after repeated refreshes. These workload-specific budgets are above observed
variation; review changes beyond them on the same machine/toolchain. Normal
tests enforce record sharing, replacement, selected detail pinning, and releasing
whole evicted snapshots without wall-clock assertions.

## Measured source identifiers

SHA-256 hashes of the lockfile, implementation, and benchmark source:

```text
27723d1b1f7763d1d0ef4f025672d8e6c2f451d27a43440bd7a4c7ad3b61bd55  Cargo.lock
8e00bc9224e1ddc67bf1291c5f2197a1d62d941fb9a28f74c6ebda0ba6321621  tui-dispatch-debug/src/debug/layer/inspector.rs
89c5e468decea18818b87bc728244231b0e2841c1285eee53c5029823112431b  tui-dispatch-debug/src/inspector.rs
6ec08f9c93851466680e5fbe44c2ccbacf0cb9ec395af5ea6a2bd2d314247d7c  tui-dispatch-debug/src/bin/tui-dispatch-debug.rs
4338899223fe2555a44c8a8e3e2e78c508c2cfbd61fe67bef2084103825ac139  tui-dispatch-debug/benches/debug_inspection.rs
105c0d2f52a4a8798dedca2eccfe38d3176e19193dbcefcc4661e6e6e39bcc7c  tui-dispatch-debug/benches/debug_inspection_memory.rs
```
