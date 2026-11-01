//! Real-clock stream cost/scaling baseline. Setup and steady delivery are separate.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    hint::black_box,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed},
    time::{Duration, Instant},
};
use tokio_stream::StreamExt;
use tui_dispatch_core::{Action, ActionChannel, BatchConfig, PausePolicy, Subscriptions};

struct Allocator;
static TRACK: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static ITEMS: AtomicUsize = AtomicUsize::new(0);
static PEAK_ITEMS: AtomicUsize = AtomicUsize::new(0);
static CLONES: AtomicUsize = AtomicUsize::new(0);
// SAFETY: unchanged pointers/layouts are forwarded to System.
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            let live = LIVE.fetch_add(layout.size(), Relaxed) + layout.size();
            if TRACK.load(Relaxed) {
                ALLOCS.fetch_add(1, Relaxed);
                BYTES.fetch_add(layout.size(), Relaxed);
                PEAK.fetch_max(live, Relaxed);
            }
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;

#[derive(Debug)]
struct Item {
    started: Instant,
    payload: Vec<u8>,
}
impl Item {
    fn new(bytes: usize) -> Self {
        PEAK_ITEMS.fetch_max(ITEMS.fetch_add(1, Relaxed) + 1, Relaxed);
        Self {
            started: Instant::now(),
            payload: vec![42; bytes],
        }
    }
}
impl Clone for Item {
    fn clone(&self) -> Self {
        CLONES.fetch_add(1, Relaxed);
        PEAK_ITEMS.fetch_max(ITEMS.fetch_add(1, Relaxed) + 1, Relaxed);
        Self {
            started: self.started,
            payload: self.payload.clone(),
        }
    }
}
impl Drop for Item {
    fn drop(&mut self) {
        ITEMS.fetch_sub(1, Relaxed);
    }
}
#[derive(Clone, Debug)]
struct Batch(Vec<Item>);
impl Action for Batch {
    fn name(&self) -> &'static str {
        "Batch"
    }
}

const N: usize = 100_000;
fn main() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    for bytes in [0, 1024] {
        for capacity in [1, 64] {
            for size in [1, 32] {
                for paused in [false, true] {
                    for sample in 0..8 {
                        rt.block_on(async {
                            let mut latency = Vec::with_capacity(N);
                            let base_live = LIVE.load(Relaxed);
                            PEAK.store(base_live, Relaxed);
                            ALLOCS.store(0, Relaxed);
                            BYTES.store(0, Relaxed);
                            PEAK_ITEMS.store(0, Relaxed);
                            CLONES.store(0, Relaxed);
                            TRACK.store(true, Relaxed);
                            let setup = Instant::now();
                            let (tx, mut rx) = ActionChannel::bounded(capacity);
                            let mut subs = Subscriptions::with_channel(tx);
                            if paused { subs.pause(); }
                            subs.stream_batched("bench", tokio_stream::iter(0..N).map(move |_| Item::new(bytes)),
                                BatchConfig::new(size, Duration::from_secs(60)),
                                PausePolicy::Buffer { capacity: 256 }, Batch);
                            if paused {
                                while ITEMS.load(Relaxed) < 256 { tokio::task::yield_now().await; }
                                assert_eq!(ITEMS.load(Relaxed), 256);
                            }
                            let setup_ns = setup.elapsed().as_nanos();
                            let setup_allocs = ALLOCS.load(Relaxed);
                            let setup_bytes = BYTES.load(Relaxed);
                            let started = Instant::now();
                            if paused { subs.resume(); }
                            while latency.len() < N {
                                let batch = rx.recv().await.unwrap();
                                for item in batch.0 {
                                    latency.push(item.started.elapsed().as_nanos());
                                    black_box(&item.payload);
                                }
                            }
                            let elapsed = started.elapsed();
                            TRACK.store(false, Relaxed);
                            assert_eq!(ITEMS.load(Relaxed), 0);
                            assert_eq!(CLONES.load(Relaxed), 0);
                            latency.sort_unstable();
                            if sample > 0 {
                                println!("bytes={bytes} capacity={capacity} batch={size} paused={paused} sample={sample} setup_ns={setup_ns} setup_allocs={setup_allocs} setup_bytes={setup_bytes} ns/item={:.2} p95_ns={} p99_ns={} allocs={} allocated_bytes={} peak_extra_bytes={} peak_items={} clones={}",
                                    elapsed.as_nanos() as f64 / N as f64, latency[N*95/100], latency[N*99/100],
                                    ALLOCS.load(Relaxed)-setup_allocs, BYTES.load(Relaxed)-setup_bytes,
                                    PEAK.load(Relaxed).saturating_sub(base_live), PEAK_ITEMS.load(Relaxed), CLONES.load(Relaxed));
                            }
                        });
                    }
                }
            }
        }
    }
}
