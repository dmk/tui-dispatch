//! Initial dispatch collection cost/scaling baseline, no transport involved.
use serde::Serialize;
use serde_json::json;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    hint::black_box,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed},
    time::Instant,
};
use tui_dispatch_core::{Action, ReducerResult, Store};
use tui_dispatch_debug::observe::{DebugCollector, DebugStoreExt};

struct CountingAllocator;
static TRACK: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn allocated(bytes: usize) {
    ALLOCS.fetch_add(1, Relaxed);
    BYTES.fetch_add(bytes, Relaxed);
    let live = LIVE.fetch_add(bytes, Relaxed) + bytes;
    PEAK.fetch_max(live, Relaxed);
}

// SAFETY: all operations forward unchanged pointers and layouts to System.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if TRACK.load(Relaxed) && !ptr.is_null() {
            allocated(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if TRACK.load(Relaxed) {
            LIVE.fetch_sub(layout.size(), Relaxed);
        }
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let result = unsafe { System.realloc(ptr, layout, new_size) };
        if TRACK.load(Relaxed) && !result.is_null() {
            // Count old and new buffers together as a conservative peak: the
            // allocator may move rather than grow the original allocation.
            allocated(new_size);
            LIVE.fetch_sub(layout.size(), Relaxed);
        }
        result
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[derive(Clone, Debug, Serialize)]
struct Add {
    amount: u64,
    payload: String,
}
impl Action for Add {
    fn name(&self) -> &'static str {
        "Add"
    }
}
#[derive(Serialize)]
struct State {
    count: u64,
    data: String,
}
fn reduce(state: &mut State, action: Add) -> ReducerResult {
    state.count = state.count.wrapping_add(action.amount);
    ReducerResult::changed()
}
fn run(bytes: usize, capacity: usize, enabled: bool, read_every: usize) {
    const N: usize = 256;
    const SAMPLES: usize = 9;
    let mut samples = [0.0; SAMPLES];
    let collector = DebugCollector::new(json!(null), capacity);
    collector.set_enabled(enabled);
    let mut store = Store::new(
        State {
            count: 0,
            data: "s".repeat(bytes),
        },
        reduce,
    )
    .with_observer(collector.observer());
    let action = Add {
        amount: 1,
        payload: "p".repeat(bytes / 16),
    };
    for sample in 0..=SAMPLES {
        let start = Instant::now();
        for index in 0..N {
            black_box(store.dispatch(black_box(action.clone())));
            if read_every > 0 && (index + 1) % read_every == 0 {
                black_box(collector.records());
            }
        }
        if sample > 0 {
            samples[sample - 1] = start.elapsed().as_secs_f64() * 1e9 / N as f64;
        }
    }
    samples.sort_by(f64::total_cmp);
    // Separate allocation probe: construct and destroy all tracked ownership in
    // the same region, then count steady-state dispatch/read after warmup.
    LIVE.store(0, Relaxed);
    TRACK.store(true, Relaxed);
    let probe = DebugCollector::new(json!(null), capacity);
    probe.set_enabled(enabled);
    let mut probe_store = Store::new(
        State {
            count: 0,
            data: "s".repeat(bytes),
        },
        reduce,
    )
    .with_observer(probe.observer());
    let probe_action = Add {
        amount: 1,
        payload: "p".repeat(bytes / 16),
    };
    for _ in 0..N {
        black_box(probe_store.dispatch(probe_action.clone()));
    }
    ALLOCS.store(0, Relaxed);
    BYTES.store(0, Relaxed);
    let retained = LIVE.load(Relaxed);
    PEAK.store(retained, Relaxed);
    for index in 0..N {
        black_box(probe_store.dispatch(probe_action.clone()));
        if read_every > 0 && (index + 1) % read_every == 0 {
            black_box(probe.records());
        }
    }
    let allocs = ALLOCS.load(Relaxed) as f64 / N as f64;
    let allocated = BYTES.load(Relaxed) / N;
    let peak = PEAK.load(Relaxed);
    drop((probe_action, probe_store, probe));
    TRACK.store(false, Relaxed);
    assert_eq!(LIVE.load(Relaxed), 0);
    println!("state={bytes} payload={} capacity={capacity} enabled={enabled} read_every={read_every}: median={:.2} min={:.2} max={:.2} ns/dispatch allocs/op={allocs:.2} bytes/op={allocated} retained={retained} peak={peak}", bytes / 16, samples[4], samples[0], samples[8]);
}
fn main() {
    println!("9 x 256 dispatches; one warmup; reads clone full history; separate allocator probe");
    for bytes in [0, 4096, 65536] {
        run(bytes, 64, false, 0);
        for capacity in [1, 64] {
            run(bytes, capacity, true, 0);
        }
        run(bytes, 64, true, 1);
        run(bytes, 64, true, 256);
    }
}
