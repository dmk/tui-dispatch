//! Initial recording cost baseline. Disk I/O uses the OS page cache; no fsync.
use serde_json::json;
use std::{hint::black_box, path::PathBuf, time::Instant};
use tui_dispatch_debug::{
    observe::DebugRecord,
    recording::{RecordingReader, RecordingWriter},
};

fn count() -> usize {
    std::env::var("TUI_DEBUG_RECORDING_COUNT")
        .map(|v| v.parse().unwrap())
        .unwrap_or(512)
}
const SAMPLES: usize = 7;
fn summary(label: &str, values: &mut [f64]) {
    values.sort_by(f64::total_cmp);
    println!(
        "{label}: median={:.3} min={:.3} max={:.3}",
        values[values.len() / 2],
        values[0],
        values[values.len() - 1]
    );
}
fn path() -> PathBuf {
    std::env::temp_dir().join(format!("td-recording-bench-{}", std::process::id()))
}
fn record(bytes: usize) -> DebugRecord {
    DebugRecord::dispatch(
        1,
        "Add",
        json!({"amount":1,"payload":"a".repeat(bytes/16)}),
        json!({"count":1,"text":"s".repeat(bytes)}),
        true,
    )
}
fn throughput(bytes: usize, flush_every: usize) {
    let n = count();
    let path = path();
    let record = record(bytes);
    let mut writes = Vec::new();
    let mut reads = Vec::new();
    let mut file_bytes = 0;
    for sample in 0..=SAMPLES {
        let mut writer = RecordingWriter::create(&path).unwrap();
        let start = Instant::now();
        for index in 1..=n {
            writer.append(black_box(&record)).unwrap();
            if index % flush_every == 0 {
                writer.flush().unwrap();
            }
        }
        writer.flush().unwrap();
        let elapsed = start.elapsed().as_secs_f64() * 1e6 / n as f64;
        writer.finish().unwrap();
        file_bytes = std::fs::metadata(&path).unwrap().len();
        let mut reader = RecordingReader::open(&path).unwrap();
        let start = Instant::now();
        let mut count = 0;
        while let Some(record) = reader.read_next().unwrap() {
            black_box(record);
            count += 1;
        }
        assert_eq!(count, n);
        let read = start.elapsed().as_secs_f64() * 1e6 / n as f64;
        if sample > 0 {
            writes.push(elapsed);
            reads.push(read);
        }
        std::fs::remove_file(&path).unwrap();
    }
    println!(
        "state={bytes} payload={} flush_every={flush_every} records={n} file_bytes={file_bytes}",
        bytes / 16
    );
    summary("  append_flush_us/record", &mut writes);
    summary("  offline_us/record", &mut reads);
    println!(
        "  write_MiB/s={:.2}",
        file_bytes as f64 / (1024.0 * 1024.0) / (writes[SAMPLES / 2] * n as f64 / 1e6)
    );
}
async fn follow(bytes: usize) {
    let n = count();
    let path = path();
    let record = record(bytes);
    let mut writer = RecordingWriter::create(&path).unwrap();
    let mut follower = RecordingReader::follow(&path).unwrap();
    // Writer finishes its entire workload before a deliberately slow reader polls.
    for _ in 0..n {
        writer.append(&record).unwrap();
    }
    writer.finish().unwrap();
    let start = Instant::now();
    let mut count = 0;
    while let Some(record) = follower.next().await.unwrap() {
        black_box(record);
        count += 1;
    }
    assert_eq!(count, n);
    println!(
        "state={bytes} follow_catchup_us/record={:.3}",
        start.elapsed().as_secs_f64() * 1e6 / n as f64
    );
    std::fs::remove_file(&path).unwrap();
}
async fn live_latency() {
    let path = path();
    let record = record(4096);
    let mut writer = RecordingWriter::create(&path).unwrap();
    let mut follower = RecordingReader::follow(&path).unwrap();
    let mut latency = Vec::new();
    for _ in 0..50 {
        let mut next = Box::pin(follower.next());
        // Ensure the reader reaches live EOF before the timed append.
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(2), next.as_mut())
                .await
                .is_err()
        );
        let start = Instant::now();
        writer.append(&record).unwrap();
        writer.flush().unwrap();
        black_box(next.await.unwrap().unwrap());
        latency.push(start.elapsed().as_secs_f64() * 1e3);
    }
    writer.finish().unwrap();
    assert!(follower.next().await.unwrap().is_none());
    latency.sort_by(f64::total_cmp);
    println!(
        "live_append_flush_delivery_ms p50={:.3} p95={:.3} p99={:.3}",
        latency[24], latency[47], latency[49]
    );
    std::fs::remove_file(path).unwrap();
}
pub fn main() {
    println!("7 batches after one warmup; hot OS page cache; setup/open/finish excluded; append includes serialization; no fsync");
    for bytes in [0, 4096, 65536] {
        for flush in [1, 64] {
            throughput(bytes, flush);
        }
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        for bytes in [0, 4096, 65536] {
            follow(bytes).await;
        }
        live_latency().await;
    });
}
