# Resource performance baseline

Roadmap item 04, measured 2026-11-01. This is an initial cost/scaling baseline:
`DataResource` has no equivalent retained-data/request-ID operation to compare.
The implementation is an uncommitted working-tree change based on
`61a1a8725ff6c3088ff3871d164955cb7214baf7`.

Measured source SHA-256 identifiers:

```text
f28536baf80d48827825ca02f30b939a74a5bd6e66c74c39edeab1b2deb77659  src/resource.rs
089462236e8789ce19fc9603686d994077c21a1f5fdff3506ed127745de8dfb6  benches/refreshable_resources.rs
```

Environment: Mac mini Mac16,10, Apple M4 (4 performance + 6 efficiency cores),
16 GiB RAM; macOS 26.6.2 (25G83), aarch64-apple-darwin.
Rust 1.95.0 (`59807616e`, 2026-04-14), LLVM 22.1.2; Cargo's optimized bench profile.
The machine was not running the verification suite during these measurements.

Run each command three times, sequentially:

```sh
cargo bench -p tui-dispatch-core --bench refreshable_resources --no-default-features --offline
cargo bench -p tui-dispatch-core --bench refreshable_resources --all-features --offline
```

[Raw results](results/refreshable_resources-2026-11-01.txt) include every run's
minimum, median, maximum, and allocation counters. Each metadata workload uses
10,000 warmup iterations and nine samples of 1,000,000 operations. Payloads are
allocated and touched before measurement. Owned-result workloads use one warmup
batch and nine measured batches: 16,384 completions for empty payloads, 16 for
nonempty payloads. Allocating/filling payloads is excluded, but dropping rejected
results or replaced data is included. Times use `std::time::Instant`, not virtual
time; they are batch-average operation costs, not delivery latency percentiles.

## Results

Ranges below are the minimum and maximum **per-run medians**, in ns/operation,
across three runs. “Failure” includes `begin()` and an accepted typed error;
“cancel” includes `begin()` and cancellation. Stale completion rejects a trivial
error while retaining the indicated data. Owned-result rows consume a `Vec<u8>`
of the indicated size; replacement also includes `begin()`.

| Workload | Retained/result size | Minimal features | All features |
| --- | ---: | ---: | ---: |
| Begin + failure | 0 | 8.16–9.54 | 8.43–10.20 |
| Begin + failure | 64 KiB | 8.16–8.62 | 8.18–8.83 |
| Begin + failure | 8 MiB | 8.25–8.83 | 8.74–8.84 |
| Begin + cancel | 0 | 8.38–8.67 | 8.36–8.63 |
| Begin + cancel | 64 KiB | 8.44–8.65 | 8.34–8.67 |
| Begin + cancel | 8 MiB | 8.35–8.48 | 8.38–8.48 |
| Stale completion | 0 | 0.77–0.78 | 0.77–0.78 |
| Stale completion | 64 KiB | 0.78–0.78 | 0.78–0.78 |
| Stale completion | 8 MiB | 0.77–0.78 | 0.78–0.78 |
| Reject owned result | 0 | 0.81–0.88 | 0.87–0.88 |
| Reject owned result | 64 KiB | 67.69–70.31 | 67.75–70.31 |
| Reject owned result | 8 MiB | 49.44–49.50 | 49.50–54.69 |
| Replace owned data | 0 | 2.24–2.27 | 2.11–2.25 |
| Replace owned data | 64 KiB | 67.69–70.31 | 67.69–67.69 |
| Replace owned data | 8 MiB | 31.25–39.06 | 36.44–44.25 |

The failure path corresponds to about 98–123 million operations/second and stale
rejection to about 1.28–1.30 billion/second in this tight loop. Metadata costs do
not grow with retained payload size. The first failure workload varies more
(individual batch averages 3.01–17.59 ns); subsequent medians overlap across
feature sets. These runs do not establish a meaningful feature-dependent change.
Owned-result timings include the system allocator's free behavior, which need not
grow monotonically with allocation size. Custom destructors may cost much more.

Every timed workload reported zero allocations and zero allocated bytes.
`RefreshableResource<Vec<u8>, ()>` occupies 48 bytes on this target, 24 beyond the
Vec descriptor. Retained buffers are neither cloned nor moved during refresh;
normal tests check pointer stability with a non-Clone 8 MiB payload and immediate
destruction of rejected results. One resource stores at most one value, one error,
and one pending ID; it has no completion queue. The owned-result benchmark prepares
at most 136 MiB of payloads (one retained + 16 incoming 8 MiB buffers), outside the
timer. This is an owned-payload bound, not an RSS measurement or a runtime queue
benchmark. End-to-end task delivery is outside this synchronous state type.

## Regression review budgets

For this machine/toolchain, investigate repeated run medians above 12 ns for
begin+failure, 10 ns for begin+cancel, 1 ns for stale-error rejection, 1.2 ns for
empty owned rejection, 3 ns for empty replacement, and 90 ns for either nonempty
owned-result path. These workload-specific review thresholds exceed the measured
variation; they are not CI wall-clock assertions or portable promises. Any
allocation in a measured operation, payload copying during refresh, or retained
obsolete completions needs review regardless of timing. The accepted cost is
constant metadata plus destruction of caller-owned values, which supports typed
errors and deterministic stale-result rejection without payload-size overhead.
