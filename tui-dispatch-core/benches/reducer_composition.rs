//! Release-mode reducer composition benchmark. Setup is excluded from timing.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    hint::black_box,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed},
    time::Instant,
};
use tui_dispatch_core::ReducerResult;

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

#[derive(Debug)]
enum Parent {
    Child(Vec<u8>),
}

struct Tagged {
    payload: Vec<u8>,
    tag: u64,
}

fn tagged(payload: Vec<u8>) -> Tagged {
    Tagged { payload, tag: 42 }
}

// The pre-helper implementation stays available for equivalent comparisons.
fn manual_map(result: ReducerResult<Vec<u8>>) -> ReducerResult<Parent> {
    let mut mapped =
        ReducerResult::effects(result.effects.into_iter().map(Parent::Child).collect());
    mapped.changed = result.changed;
    mapped
}

fn manual_merge<E>(mut left: ReducerResult<E>, right: ReducerResult<E>) -> ReducerResult<E> {
    left.changed |= right.changed;
    left.effects.extend(right.effects);
    left
}

fn measure<I, O>(label: &str, mut setup: impl FnMut() -> I, mut operation: impl FnMut(I) -> O) {
    // Allocation probes are separate from timing; input construction and output
    // destruction are excluded from each timed batch. Warmup is one full batch.
    const BATCH: usize = 128;
    const SAMPLES: usize = 9;
    let mut samples = [0.0; SAMPLES];
    let mut outputs = Vec::with_capacity(BATCH);
    for sample in 0..=SAMPLES {
        let inputs: Vec<_> = (0..BATCH).map(|_| setup()).collect();
        let start = Instant::now();
        for input in inputs {
            outputs.push(black_box(operation(black_box(input))));
        }
        let ns = start.elapsed().as_nanos() as f64 / BATCH as f64;
        if sample > 0 {
            samples[sample - 1] = ns;
        }
        outputs.clear();
    }
    samples.sort_by(f64::total_cmp);

    LIVE.store(0, Relaxed);
    TRACK.store(true, Relaxed);
    let input = setup();
    let input_bytes = LIVE.load(Relaxed);
    ALLOCS.store(0, Relaxed);
    BYTES.store(0, Relaxed);
    PEAK.store(input_bytes, Relaxed);
    let output = operation(black_box(input));
    let allocs = ALLOCS.load(Relaxed);
    let bytes = BYTES.load(Relaxed);
    let peak = PEAK.load(Relaxed);
    drop(output);
    TRACK.store(false, Relaxed);
    assert_eq!(LIVE.load(Relaxed), 0);
    println!(
        "{label:42} ns/op min={:.2} median={:.2} max={:.2} allocs={allocs} allocated_bytes={bytes} input_live_bytes={input_bytes} peak_live_bytes={peak}",
        samples[0], samples[SAMPLES / 2], samples[SAMPLES - 1],
    );
}

fn main() {
    println!("9 samples x 128 operations; 1 warmup batch; separate allocation probe");
    for count in [0, 1, 16, 256, 4096] {
        for payload in [0, 4096] {
            // Cap payload setup at 16 MiB per batch.
            if count * payload > 128 * 1024 {
                continue;
            }
            let setup = || {
                ReducerResult::effects((0..count).map(|_| vec![42; payload]).collect())
                    .mark_changed()
            };
            measure(
                &format!("manual map n={count} payload={payload}"),
                setup,
                |r| {
                    let result = manual_map(r);
                    if let Some(Parent::Child(bytes)) = result.effects.first() {
                        black_box(bytes.as_ptr());
                    }
                    result
                },
            );
            measure(
                &format!("helper map n={count} payload={payload}"),
                setup,
                |r| {
                    let result = r.map_effect(Parent::Child);
                    if let Some(Parent::Child(bytes)) = result.effects.first() {
                        black_box(bytes.as_ptr());
                    }
                    result
                },
            );
            // Unlike the transparent enum above, this widens each effect's
            // descriptor and measures element movement plus output allocation.
            measure(
                &format!("manual tagged n={count} payload={payload}"),
                setup,
                |r| {
                    let mut result =
                        ReducerResult::effects(r.effects.into_iter().map(tagged).collect());
                    result.changed = r.changed;
                    if let Some(effect) = result.effects.first() {
                        black_box((effect.payload.as_ptr(), effect.tag));
                    }
                    result
                },
            );
            measure(
                &format!("helper tagged n={count} payload={payload}"),
                setup,
                |r| {
                    let result = r.map_effect(tagged);
                    if let Some(effect) = result.effects.first() {
                        black_box((effect.payload.as_ptr(), effect.tag));
                    }
                    result
                },
            );
        }
    }
    for (left, right, spare) in [
        (0, 0, false),
        (0, 16, false),
        (16, 0, false),
        (16, 16, false),
        (256, 256, false),
        (4096, 4096, false),
        (4096, 4096, true),
    ] {
        let setup = || {
            let mut effects = Vec::with_capacity(if spare { left + right } else { left });
            effects.extend(0..left as u64);
            (
                ReducerResult::effects(effects),
                ReducerResult::effects((0..right as u64).collect()).mark_changed(),
            )
        };
        measure(
            &format!("manual merge {left}+{right} spare={spare}"),
            setup,
            |(left, right)| manual_merge(left, right),
        );
        measure(
            &format!("helper merge {left}+{right} spare={spare}"),
            setup,
            |(left, right)| left.merge(right),
        );
    }
}
