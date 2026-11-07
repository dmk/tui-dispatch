//! Dispatch baseline shared by debug-compiled-out and debug-disabled builds.
use std::{hint::black_box, time::Instant};
use tui_dispatch_core::{Action, NoopMiddleware, ReducerResult, Store, StoreWithMiddleware};

#[derive(Clone, Debug)]
struct Add(u64);
impl Action for Add {
    fn name(&self) -> &'static str {
        "Add"
    }
}
fn reduce(state: &mut u64, action: Add) -> ReducerResult {
    *state = state.wrapping_add(action.0);
    ReducerResult::changed()
}
fn measure(name: &str, mut dispatch: impl FnMut()) {
    const N: usize = 1_000_000;
    for _ in 0..N {
        dispatch();
    }
    let mut samples = Vec::new();
    for _ in 0..11 {
        let start = Instant::now();
        for _ in 0..N {
            dispatch();
        }
        samples.push(start.elapsed().as_secs_f64() * 1e9 / N as f64);
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "{name}: median={:.2} min={:.2} max={:.2} ns/dispatch",
        samples[5], samples[0], samples[10]
    );
}
fn main() {
    println!("debug_feature={}", cfg!(feature = "debug"));
    let mut store = Store::new(0u64, reduce);
    measure("plain", || {
        black_box(store.dispatch(black_box(Add(1))));
        black_box(store.state());
    });
    let mut store = StoreWithMiddleware::new(0u64, reduce, NoopMiddleware);
    measure("middleware", || {
        black_box(store.dispatch(black_box(Add(1))));
        black_box(store.state());
    });
}
