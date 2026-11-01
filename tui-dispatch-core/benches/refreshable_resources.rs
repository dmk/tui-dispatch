//! Run with `cargo bench -p tui-dispatch-core --bench refreshable_resources`.
//! Setup and warmup are excluded. Samples are batch averages, not request latency percentiles.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    hint::black_box,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed},
    time::Instant,
};
use tui_dispatch_core::RefreshableResource;

struct CountingAllocator;
static TRACK: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);

// SAFETY: every operation forwards its unchanged pointer/layout to System.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACK.load(Relaxed) {
            ALLOCATIONS.fetch_add(1, Relaxed);
            BYTES.fetch_add(layout.size(), Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

const ITERATIONS: usize = 1_000_000;
const SAMPLES: usize = 9;

fn measure(label: &str, bytes: usize, mut operation: impl FnMut()) {
    for _ in 0..10_000 {
        operation();
    }
    let mut samples = [0.0; SAMPLES];
    ALLOCATIONS.store(0, Relaxed);
    BYTES.store(0, Relaxed);
    TRACK.store(true, Relaxed);
    for sample in &mut samples {
        let start = Instant::now();
        for _ in 0..ITERATIONS {
            operation();
        }
        *sample = start.elapsed().as_nanos() as f64 / ITERATIONS as f64;
    }
    TRACK.store(false, Relaxed);
    samples.sort_by(f64::total_cmp);
    println!(
        "{label:20} payload={bytes:8} bytes  ns/op min={:.3} median={:.3} max={:.3}  allocs={} allocated_bytes={}",
        samples[0], samples[SAMPLES / 2], samples[SAMPLES - 1],
        ALLOCATIONS.load(Relaxed), BYTES.load(Relaxed),
    );
}

fn measure_owned_completions(bytes: usize, stale: bool) {
    // Empty Vecs need a larger batch to rise above clock resolution. Bound
    // allocated payloads to 128 MiB for the largest workload.
    let batch = if bytes == 0 { 16_384 } else { 16 };
    let mut samples = [0.0; SAMPLES];
    ALLOCATIONS.store(0, Relaxed);
    BYTES.store(0, Relaxed);
    // One warmup batch, then measured batches. Payload creation is excluded;
    // destruction of rejected/replaced payloads is part of complete().
    for sample in 0..=SAMPLES {
        let mut resource = RefreshableResource::<_, ()>::loaded(vec![42u8; bytes]);
        let completions: Vec<_> = (0..batch).map(|_| vec![43u8; bytes]).collect();
        let old = resource.begin();
        black_box(resource.begin());
        TRACK.store(sample > 0, Relaxed);
        let start = Instant::now();
        for payload in completions {
            let request = if stale { old } else { resource.begin() };
            black_box(
                black_box(&mut resource).complete(black_box(request), Ok(black_box(payload))),
            );
        }
        let elapsed = start.elapsed().as_nanos() as f64 / batch as f64;
        TRACK.store(false, Relaxed);
        if sample > 0 {
            samples[sample - 1] = elapsed;
        }
    }
    samples.sort_by(f64::total_cmp);
    let label = if stale {
        "reject owned result"
    } else {
        "replace owned data"
    };
    println!(
        "{label:20} payload={bytes:8} bytes  ns/op min={:.3} median={:.3} max={:.3}  allocs={} allocated_bytes={} setup_payload_bytes={}",
        samples[0], samples[SAMPLES / 2], samples[SAMPLES - 1],
        ALLOCATIONS.load(Relaxed), BYTES.load(Relaxed), (batch + 1) * bytes,
    );
}

fn main() {
    println!("{SAMPLES} samples x {ITERATIONS} operations; 10000 warmup operations");
    println!(
        "resource metadata={} bytes (excluding Vec descriptor and payload)",
        std::mem::size_of::<RefreshableResource<Vec<u8>, ()>>() - std::mem::size_of::<Vec<u8>>()
    );
    for bytes in [0, 64 * 1024, 8 * 1024 * 1024] {
        let mut resource = RefreshableResource::<_, ()>::loaded(vec![42u8; bytes]);
        // Touch the retained payload before measuring steady-state metadata updates.
        black_box(
            resource
                .data()
                .unwrap()
                .iter()
                .map(|&n| n as u64)
                .sum::<u64>(),
        );
        measure("begin + failure", bytes, || {
            let request = black_box(&mut resource).begin();
            black_box(resource.complete(black_box(request), black_box(Err(()))));
        });
        measure("begin + cancel", bytes, || {
            black_box(black_box(&mut resource).begin());
            black_box(resource.cancel());
        });
        let stale = resource.begin();
        let current = resource.begin();
        measure("stale completion", bytes, || {
            black_box(black_box(&mut resource).complete(black_box(stale), black_box(Err(()))));
        });
        assert!(resource.complete(current, Err(())));
        assert_eq!(resource.data().unwrap().len(), bytes);
        drop(resource);
        measure_owned_completions(bytes, true);
        measure_owned_completions(bytes, false);
    }
}
