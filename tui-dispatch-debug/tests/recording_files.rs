use serde_json::json;
use std::{
    fs, io,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
use tui_dispatch_core::DispatchError;
use tui_dispatch_debug::{
    observe::{DebugRecord, DispatchOutcome},
    recording::MAX_RECORD_BYTES,
    records::{SnapshotOutcome, SnapshotRecord},
};

mod support {
    use super::*;
    pub struct TempDir(PathBuf);
    impl TempDir {
        pub fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "td-recording-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        pub fn path(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}
mod debug_fixture {
    use super::*;
    pub fn record(sequence: u64, amount: i32, count: i32) -> DebugRecord {
        DebugRecord::dispatch(
            sequence,
            "Add",
            json!({"amount":amount}),
            json!({"count":count}),
            true,
        )
    }
}
use std::time::Duration;
use tui_dispatch_debug::recording::{RecordingReader, RecordingWriter};

#[tokio::test]
async fn following_and_offline_reading_preserve_order_and_payloads() {
    tokio::time::timeout(Duration::from_secs(3), async {
        let directory = support::TempDir::new();
        let path = directory.path("session.tuidbg");
        let first = debug_fixture::record(1, 2, 2);
        let second = debug_fixture::record(2, 3, 5);
        let mut writer = RecordingWriter::create(&path).unwrap();
        writer.append(&first).unwrap();
        writer.flush().unwrap();
        let mut follower = RecordingReader::follow(&path).unwrap();
        assert_eq!(follower.next().await.unwrap(), Some(first.clone().into()));
        // Catch up to the current EOF while the writer is still open. Following
        // must wait for a later append, rather than report permanent EOF.
        let mut next = Box::pin(follower.next());
        // Drive through any initial asynchronous file read too: a single Pending
        // poll could only mean the read was started, rather than EOF was reached.
        assert!(
            tokio::time::timeout(Duration::from_millis(20), next.as_mut())
                .await
                .is_err(),
            "live EOF must stay pending until the writer appends or finishes",
        );
        writer.append(&second).unwrap();
        writer.flush().unwrap();
        assert_eq!(next.await.unwrap(), Some(second.clone().into()));
        let mut end = Box::pin(follower.next());
        assert!(
            tokio::time::timeout(Duration::from_millis(20), end.as_mut())
                .await
                .is_err(),
            "catching up must not finish a still-open recording",
        );
        writer.finish().unwrap();
        assert_eq!(end.await.unwrap(), None);
        let records = RecordingReader::open(&path).unwrap().read_all().unwrap();
        assert_eq!(records, [first.into(), second.into()]);
    })
    .await
    .expect("following a recording must deliver flushed records");
}

#[test]
fn truncated_recording_reports_an_incomplete_tail_without_losing_complete_records() {
    let directory = support::TempDir::new();
    let path = directory.path("crashed.tuidbg");
    let first = debug_fixture::record(1, 2, 2);
    let mut writer = RecordingWriter::create(&path).unwrap();
    writer.append(&first).unwrap();
    writer.flush().unwrap();
    let complete_length = std::fs::metadata(&path).unwrap().len();
    writer.append(&debug_fixture::record(2, 3, 5)).unwrap();
    writer.flush().unwrap();
    drop(writer);
    // Keep only one byte of the next record, regardless of the chosen encoding.
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(complete_length + 1)
        .unwrap();
    let recovery = RecordingReader::open(&path).unwrap().recover().unwrap();
    assert_eq!(recovery.records, [first.into()]);
    assert!(recovery.incomplete_tail);
}

#[test]
fn empty_finished_and_unfinished_files_have_distinct_eof() {
    let dir = support::TempDir::new();
    let path = dir.path("empty");
    let writer = RecordingWriter::create(&path).unwrap();
    assert!(
        RecordingReader::open(&path)
            .unwrap()
            .recover()
            .unwrap()
            .incomplete_tail
    );
    assert_eq!(
        RecordingReader::open(&path)
            .unwrap()
            .read_all()
            .unwrap_err()
            .kind(),
        io::ErrorKind::UnexpectedEof
    );
    writer.finish().unwrap();
    assert!(RecordingReader::open(&path)
        .unwrap()
        .read_all()
        .unwrap()
        .is_empty());
    assert!(
        !RecordingReader::open(&path)
            .unwrap()
            .recover()
            .unwrap()
            .incomplete_tail
    );
}

#[test]
fn every_truncation_recovers_only_complete_records() {
    let dir = support::TempDir::new();
    let path = dir.path("full");
    let first = debug_fixture::record(1, 2, 2);
    let second = debug_fixture::record(2, 3, 5);
    let mut writer = RecordingWriter::create(&path).unwrap();
    writer.append(&first).unwrap();
    writer.flush().unwrap();
    let boundary = fs::metadata(&path).unwrap().len() as usize;
    writer.append(&second).unwrap();
    writer.flush().unwrap();
    let second_boundary = fs::metadata(&path).unwrap().len() as usize;
    writer.finish().unwrap();
    let bytes = fs::read(&path).unwrap();
    let cut = dir.path("cut");
    for length in 0..=bytes.len() {
        fs::write(&cut, &bytes[..length]).unwrap();
        if length < 8 {
            assert!(RecordingReader::open(&cut).is_err());
            continue;
        }
        let recovery = RecordingReader::open(&cut).unwrap().recover().unwrap();
        let expected = [SnapshotRecord::from(first.clone()), second.clone().into()];
        let count = usize::from(length >= boundary) + usize::from(length >= second_boundary);
        assert_eq!(recovery.records, expected[..count], "length={length}");
        assert_eq!(recovery.incomplete_tail, length < bytes.len());
    }
}

#[tokio::test]
async fn partial_prefix_and_payload_survive_cancelled_next_calls() {
    tokio::time::timeout(Duration::from_secs(3), async {
        use std::io::Write;
        let dir = support::TempDir::new();
        let path = dir.path("partial");
        drop(RecordingWriter::create(&path).unwrap());
        let record: SnapshotRecord = debug_fixture::record(1, 2, 2).into();
        let payload = serde_json::to_vec(&record).unwrap();
        let mut frame = (payload.len() as u32).to_le_bytes().to_vec();
        frame.extend_from_slice(&payload);
        let mut output = fs::OpenOptions::new().append(true).open(&path).unwrap();
        let mut follower = RecordingReader::follow(&path).unwrap();
        let mut start = 0;
        for end in [1, 3, 4, 10, frame.len() - 1] {
            output.write_all(&frame[start..end]).unwrap();
            assert!(
                tokio::time::timeout(Duration::from_millis(20), follower.next())
                    .await
                    .is_err()
            );
            start = end;
        }
        output.write_all(&frame[start..]).unwrap();
        assert_eq!(follower.next().await.unwrap(), Some(record));
        output.write_all(&0u32.to_le_bytes()).unwrap();
        assert_eq!(follower.next().await.unwrap(), None);
        assert_eq!(follower.next().await.unwrap(), None);
    })
    .await
    .unwrap();
}

#[test]
fn corruption_and_frame_limits_are_errors_not_recoverable_truncation() {
    let dir = support::TempDir::new();
    let path = dir.path("corrupt");
    drop(RecordingWriter::create(&path).unwrap());
    let header = fs::read(&path).unwrap();
    for tail in [
        ((MAX_RECORD_BYTES + 1) as u32).to_le_bytes().to_vec(),
        vec![1, 0, 0, 0, b'!'],
    ] {
        let mut bytes = header.clone();
        bytes.extend(tail);
        fs::write(&path, bytes).unwrap();
        assert_eq!(
            RecordingReader::open(&path)
                .unwrap()
                .recover()
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
    }
    let mut version = header;
    version[6] = 255;
    fs::write(&path, version).unwrap();
    assert!(RecordingReader::open(&path).is_err());
}

#[test]
fn oversize_append_leaves_complete_prefix_and_writer_usable() {
    let dir = support::TempDir::new();
    let path = dir.path("limit");
    let mut writer = RecordingWriter::create(&path).unwrap();
    let first = debug_fixture::record(1, 2, 2);
    writer.append(&first).unwrap();
    let mut large = first.clone();
    large.state = json!("x".repeat(MAX_RECORD_BYTES));
    assert!(writer.append(&large).is_err());
    writer.append(&first).unwrap();
    writer.finish().unwrap();
    assert_eq!(
        RecordingReader::open(path).unwrap().read_all().unwrap(),
        [first.clone().into(), first.into()]
    );
}

#[test]
fn all_metadata_and_owned_rejection_details_round_trip() {
    let dir = support::TempDir::new();
    let path = dir.path("outcomes");
    let mut writer = RecordingWriter::create(&path).unwrap();
    let mut expected: Vec<SnapshotRecord> = Vec::new();
    for outcome in [
        DispatchOutcome::Applied,
        DispatchOutcome::Cancelled,
        DispatchOutcome::Rejected(DispatchError::DepthExceeded {
            max_depth: 3,
            action: "Add",
        }),
        DispatchOutcome::Rejected(DispatchError::ActionBudgetExceeded {
            max_actions: 4,
            processed: 4,
            action: "Add",
        }),
    ] {
        let mut record = debug_fixture::record(7, 2, 2);
        record.action_id = 42;
        record.parent_action_id = Some(6);
        record.action = json!({"nested":[null, "λ\n", true]});
        record.outcome = outcome;
        record.serialization_errors = vec!["state: unavailable".into()];
        writer.append(&record).unwrap();
        expected.push(record.into());
    }
    // Arbitrary owned names need neither static strings nor leaked allocations.
    let mut record = expected[0].clone();
    record.outcome = SnapshotOutcome::DepthExceeded {
        max_depth: 3,
        action: "from disk".into(),
    };
    writer.append_record(&record).unwrap();
    expected.push(record);
    writer.finish().unwrap();
    assert_eq!(
        RecordingReader::open(path).unwrap().read_all().unwrap(),
        expected
    );
}

#[test]
fn creation_never_overwrites_and_files_are_private() {
    let dir = support::TempDir::new();
    let path = dir.path("existing");
    fs::write(&path, b"keep").unwrap();
    assert!(RecordingWriter::create(&path).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"keep");
    #[cfg(unix)]
    {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let link = dir.path("link");
        symlink(&path, &link).unwrap();
        assert!(RecordingWriter::create(link).is_err());
        let private = dir.path("private");
        RecordingWriter::create(&private).unwrap().finish().unwrap();
        assert_eq!(
            fs::metadata(private).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[tokio::test]
async fn slow_follower_replays_all_records_without_writer_backpressure() {
    let dir = support::TempDir::new();
    let path = dir.path("slow");
    let mut writer = RecordingWriter::create(&path).unwrap();
    let mut follower = RecordingReader::follow(&path).unwrap();
    for sequence in 1..=500 {
        writer
            .append(&debug_fixture::record(sequence, 1, sequence as i32))
            .unwrap();
    }
    writer.finish().unwrap();
    for sequence in 1..=500 {
        assert_eq!(follower.next().await.unwrap().unwrap().sequence, sequence);
    }
    assert!(follower.next().await.unwrap().is_none());
}

#[test]
fn standalone_binary_reads_finished_and_interrupted_recordings_with_bounded_history() {
    use std::process::Command;
    let dir = support::TempDir::new();
    for finished in [false, true] {
        let path = dir.path(if finished { "done" } else { "interrupted" });
        let mut writer = RecordingWriter::create(&path).unwrap();
        for sequence in 1..=5 {
            writer
                .append(&debug_fixture::record(sequence, 1, sequence as i32))
                .unwrap();
        }
        if finished {
            writer.finish().unwrap();
        } else {
            writer.flush().unwrap();
            drop(writer);
        }
        let output = Command::new(env!("CARGO_BIN_EXE_tui-dispatch-debug"))
            .args([
                "--recording",
                path.to_str().unwrap(),
                "--once",
                "--history-limit",
                "2",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let snapshot: tui_dispatch_debug::records::DebugSnapshot =
            serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(snapshot.state, json!({"count":5}));
        assert_eq!(
            snapshot
                .history
                .iter()
                .map(|r| r.sequence)
                .collect::<Vec<_>>(),
            [4, 5]
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stderr).contains("incomplete tail"),
            !finished
        );
    }
    for args in [
        vec!["--follow"],
        vec!["--recording", "file", "--once", "--follow"],
        vec!["--recording", "file", "--history-limit", "0"],
    ] {
        assert!(!Command::new(env!("CARGO_BIN_EXE_tui-dispatch-debug"))
            .args(args)
            .output()
            .unwrap()
            .status
            .success());
    }
}
