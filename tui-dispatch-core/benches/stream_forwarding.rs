//! Real-clock saturated stream delivery benchmark; one warmup and seven samples.
use std::{hint::black_box, time::Instant};
use tui_dispatch_core::{Action, Subscriptions};

#[derive(Clone, Debug)]
struct Item {
    started: Instant,
    payload: Vec<u8>,
}
impl Action for Item {
    fn name(&self) -> &'static str {
        "Item"
    }
}

fn main() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    for bytes in [0, 1024] {
        for sample in 0..8 {
            rt.block_on(async {
                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                let mut subs = Subscriptions::new(tx);
                let mut latencies = Vec::with_capacity(100_000);
                let start = Instant::now();
                subs.stream(
                    "bench",
                    tokio_stream::iter(0..100_000).map(move |_| Item {
                        started: Instant::now(),
                        payload: vec![42; bytes],
                    }),
                );
                for _ in 0..100_000 {
                    let item = rx.recv().await.unwrap();
                    latencies.push(item.started.elapsed().as_nanos());
                    black_box(item.payload);
                }
                let elapsed = start.elapsed();
                latencies.sort_unstable();
                if sample > 0 {
                    println!(
                        "legacy bytes={bytes} sample={sample} ns/item={:.2} p95_ns={} p99_ns={}",
                        elapsed.as_nanos() as f64 / 100_000.0,
                        latencies[95_000],
                        latencies[99_000]
                    );
                }
            });
        }
    }
}
use tokio_stream::StreamExt;
