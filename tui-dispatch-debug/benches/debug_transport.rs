//! Real-time Unix snapshot transport baseline; run without other build jobs.
#[cfg(unix)]
mod unix {
    use serde::Serialize;
    use serde_json::json;
    use std::{
        hint::black_box,
        time::{Duration, Instant},
    };
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::UnixStream,
    };
    use tui_dispatch_core::{Action, ReducerResult, Store};
    use tui_dispatch_debug::{
        observe::{DebugCollector, DebugStoreExt},
        transport::{DebugClient, DebugServer},
    };

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

    async fn run(bytes: usize, capacity: usize, mode: &str) {
        const N: usize = 256;
        let directory = format!("/tmp/td-bench-{}", std::process::id());
        std::fs::create_dir(&directory).unwrap();
        let path = format!("{directory}/app.sock");
        let collector = DebugCollector::new(json!(null), capacity);
        collector.set_enabled(mode != "disabled");
        let mut store = Store::new(
            State {
                count: 0,
                data: "s".repeat(bytes),
            },
            |state, action: Add| -> ReducerResult {
                state.count = state.count.wrapping_add(action.amount);
                ReducerResult::changed()
            },
        )
        .with_observer(collector.observer());
        let action = Add {
            amount: 1,
            payload: "p".repeat(bytes / 16),
        };
        for _ in 0..N {
            black_box(store.dispatch(action.clone()));
        }
        let server = if mode == "no-server" || mode == "disabled" {
            None
        } else {
            Some(DebugServer::bind(&path, collector.clone()).await.unwrap())
        };
        let mut slow = None;
        let mut reader = None;
        if mode == "slow" {
            let mut stream = UnixStream::connect(&path).await.unwrap();
            stream.write_u8(1).await.unwrap();
            black_box(stream.read_u32().await.unwrap());
            slow = Some(stream);
        }
        if mode == "reader" {
            let mut client = DebugClient::connect(&path).await.unwrap();
            client.snapshot().await.unwrap();
            reader = Some(tokio::spawn(async move {
                loop {
                    black_box(client.snapshot().await.unwrap());
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            }));
        }
        let mut dispatch = Vec::new();
        for _ in 0..9 {
            let start = Instant::now();
            for _ in 0..N {
                black_box(store.dispatch(action.clone()));
            }
            dispatch.push(start.elapsed().as_secs_f64() * 1e9 / N as f64);
            // Allow a ready reader to request another snapshot between batches.
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        dispatch.sort_by(f64::total_cmp);
        assert!(collector.records().len() <= capacity);
        if let Some(reader) = reader {
            reader.abort();
            let _ = reader.await;
        }
        drop(slow);
        println!("state={bytes} payload={} capacity={capacity} mode={mode}: dispatch_ns median={:.2} min={:.2} max={:.2}", bytes / 16, dispatch[4], dispatch[0], dispatch[8]);
        if server.is_some() {
            let mut client = DebugClient::connect(&path).await.unwrap();
            client.snapshot().await.unwrap();
            let mut latency = Vec::new();
            let mut wire_bytes = 0;
            for _ in 0..100 {
                let start = Instant::now();
                let snapshot = client.snapshot().await.unwrap();
                latency.push(start.elapsed().as_secs_f64() * 1e6);
                if wire_bytes == 0 {
                    wire_bytes = serde_json::to_vec(&snapshot).unwrap().len();
                }
                black_box(snapshot);
            }
            latency.sort_by(f64::total_cmp);
            println!(
                "  snapshot_us p50={:.2} p95={:.2} p99={:.2} bytes={wire_bytes}",
                latency[49], latency[94], latency[98]
            );
        }
        if let Some(server) = server {
            server.shutdown().await.unwrap();
        }
        std::fs::remove_dir(&directory).unwrap();
    }

    pub fn main() {
        println!("2 Tokio worker threads; 9 x 256 dispatches; 100 sequential full-snapshot round trips; one warmup");
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            // Isolate one case per process when measuring RSS, so allocator
            // caches from earlier workloads do not dominate its high-water mark.
            if let Ok(case) = std::env::var("TUI_DEBUG_TRANSPORT_CASE") {
                let parts = case.split(',').collect::<Vec<_>>();
                assert_eq!(parts.len(), 3, "case must be bytes,capacity,mode");
                run(
                    parts[0].parse().unwrap(),
                    parts[1].parse().unwrap(),
                    parts[2],
                )
                .await;
                return;
            }
            for bytes in [0, 4096, 65536] {
                for capacity in [1, 64] {
                    for mode in ["disabled", "no-server", "idle", "reader", "slow"] {
                        run(bytes, capacity, mode).await;
                    }
                }
            }
        });
    }
}

pub fn main() {
    #[cfg(unix)]
    unix::main();
}
