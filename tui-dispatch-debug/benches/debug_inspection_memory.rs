//! Owned heap accounting, separate from uninstrumented CPU timings.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::json;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicUsize, Ordering::Relaxed},
};
use tui_dispatch_core::EventKind;
use tui_dispatch_debug::{debug::DebugInspector, observe::DebugRecord, records::DebugSnapshot};
struct Counter;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
fn allocated(size: usize) {
    ALLOCS.fetch_add(1, Relaxed);
    PEAK.fetch_max(LIVE.fetch_add(size, Relaxed) + size, Relaxed);
}
// SAFETY: allocation pointers and layouts are forwarded unchanged to System.
unsafe impl GlobalAlloc for Counter {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            allocated(layout.size());
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let ptr = unsafe { System.realloc(ptr, layout, size) };
        if !ptr.is_null() {
            allocated(size);
            LIVE.fetch_sub(layout.size(), Relaxed);
        }
        ptr
    }
}
#[global_allocator]
static ALLOCATOR: Counter = Counter;
fn snapshot(count: u64, bytes: usize) -> DebugSnapshot {
    let history: Vec<tui_dispatch_debug::records::SnapshotRecord> = (1..=count)
        .map(|n| {
            DebugRecord::dispatch(
                n,
                "Add",
                json!({"amount":1,"text":"a".repeat(bytes/16)}),
                json!({"count":n,"text":"s".repeat(bytes)}),
                true,
            )
            .into()
        })
        .collect();
    DebugSnapshot {
        state: history.last().unwrap().state.clone(),
        history,
        components: None,
        control: None,
    }
}
fn main() {
    println!("owned bytes; input moved into view; no socket/watch copy or terminal; conservative realloc peaks");
    for count in [64, 1024] {
        for bytes in [0, 4096, 65536] {
            let baseline = LIVE.load(Relaxed);
            let data = snapshot(count, bytes);
            let mut view = DebugInspector::new();
            PEAK.store(LIVE.load(Relaxed), Relaxed);
            let allocs = ALLOCS.load(Relaxed);
            view.update_snapshot(data);
            let update_allocs = ALLOCS.load(Relaxed) - allocs;
            let retained = LIVE.load(Relaxed) - baseline;
            let update_peak = PEAK.load(Relaxed) - baseline;
            let allocs = ALLOCS.load(Relaxed);
            let diff = view.selected_diff().unwrap();
            let diff_allocs = ALLOCS.load(Relaxed) - allocs;
            drop(diff);
            view.handle_event(&EventKind::Key(KeyEvent::new(
                KeyCode::Enter,
                KeyModifiers::NONE,
            )));
            let open = LIVE.load(Relaxed) - baseline;
            // Simulate a slow viewer with an open detail while many snapshots replace history.
            for _ in 0..16 {
                view.update_snapshot(snapshot(count, bytes));
            }
            let refreshed_open = LIVE.load(Relaxed) - baseline;
            drop(view);
            assert_eq!(
                LIVE.load(Relaxed),
                baseline,
                "all inspector ownership must be released"
            );
            println!("history={count} state={bytes} retained={retained} update_peak={update_peak} update_allocs={update_allocs} diff_allocs={diff_allocs} open={open} after_16_refreshes={refreshed_open}");
        }
    }
}
