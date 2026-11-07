#![cfg(unix)]

use serde::Serialize;
use serde_json::json;
use std::{
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{UnixListener, UnixStream},
};
use tui_dispatch_core::{Action, DispatchError, ReducerResult, Store};
use tui_dispatch_debug::{
    debug::ComponentSnapshot,
    observe::{DebugCollector, DebugRecord, DebugStoreExt, DispatchOutcome},
    transport::{DebugClient, DebugServer, SnapshotOutcome, MAX_CLIENTS, MAX_SNAPSHOT_BYTES},
};

#[tokio::test]
async fn component_frames_round_trip_without_dispatch_and_survive_reconnect() {
    bounded(async {
        let directory = Directory::new();
        let collector = DebugCollector::new(json!({"count":0}), 8);
        let server = DebugServer::bind(directory.socket(), collector.clone())
            .await
            .unwrap();
        let mut client = DebugClient::connect(directory.socket()).await.unwrap();
        assert!(client.snapshot().await.unwrap().components.is_none());
        let component = ComponentSnapshot {
            raw_id: 7,
            type_name: "Player".into(),
            type_name_full: "game::Player".into(),
            bound_id: Some("player".into()),
            last_area: Some(ratatui::layout::Rect::new(2, 3, 40, 12)),
            debug_entries: vec![("lives".into(), "3".into())],
        };
        collector.set_components(vec![component.clone()]);
        let snapshot = client.snapshot().await.unwrap();
        assert_eq!(snapshot.components, Some(vec![component.clone()]));
        assert!(snapshot.history.is_empty());
        assert_eq!(snapshot.state, json!({"count":0}));
        drop(client);
        let mut next = component;
        next.last_area = None;
        next.debug_entries[0].1 = "2".into();
        collector.set_components(vec![next.clone()]);
        let mut reconnected = DebugClient::connect(directory.socket()).await.unwrap();
        assert_eq!(
            reconnected.snapshot().await.unwrap().components,
            Some(vec![next.clone()])
        );
        collector.set_enabled(false);
        collector.set_components(Vec::new());
        assert_eq!(collector.components(), Some(vec![next]));
        collector.set_enabled(true);
        collector.set_components(Vec::new());
        assert_eq!(
            reconnected.snapshot().await.unwrap().components,
            Some(Vec::new())
        );
        server.shutdown().await.unwrap();
    })
    .await;
}

#[test]
fn older_socket_snapshots_without_component_telemetry_still_decode() {
    let snapshot: tui_dispatch_debug::records::DebugSnapshot = serde_json::from_value(json!({
        "state": {"count":0}, "history": [],
    }))
    .unwrap();
    assert!(snapshot.components.is_none());
}

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = PathBuf::from(format!(
            "/tmp/td-attach-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn socket(&self) -> PathBuf {
        self.0.join("app.sock")
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

async fn bounded(work: impl std::future::Future<Output = ()>) {
    tokio::time::timeout(Duration::from_secs(10), work)
        .await
        .expect("attachment must not hang");
}

#[derive(Clone, Debug, Serialize)]
struct Add(i32);
impl Action for Add {
    fn name(&self) -> &'static str {
        "Add"
    }
}

#[tokio::test]
async fn real_store_late_attachment_reconnect_and_shutdown_preserve_collection() {
    bounded(async {
        let directory = Directory::new();
        let path = directory.socket();
        let collector = DebugCollector::new(json!(0), 2);
        let mut store = Store::new(0, |state, action: Add| -> ReducerResult {
            *state += action.0;
            ReducerResult::changed()
        })
        .with_observer(collector.observer());
        store.dispatch(Add(2));
        let server = DebugServer::bind(&path, collector.clone()).await.unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let mut client = DebugClient::connect(&path).await.unwrap();
        let snapshot = client.snapshot().await.unwrap();
        assert_eq!(snapshot.state, json!(2));
        assert_eq!(snapshot.history[0].action, json!(2));
        assert_eq!(snapshot.history[0].sequence, 1);
        assert_eq!(snapshot.history[0].outcome, SnapshotOutcome::Applied);
        drop(client);
        store.dispatch(Add(3));
        store.dispatch(Add(4));
        let mut client = DebugClient::connect(&path).await.unwrap();
        let snapshot = client.snapshot().await.unwrap();
        assert_eq!(snapshot.state, json!(9));
        assert_eq!(
            snapshot
                .history
                .iter()
                .map(|r| r.sequence)
                .collect::<Vec<_>>(),
            [2, 3]
        );
        server.shutdown().await.unwrap();
        assert!(!path.exists());
        assert!(client.snapshot().await.is_err());
        store.dispatch(Add(1));
        assert_eq!(collector.records().last().unwrap().state, json!(10));
    })
    .await;
}

#[tokio::test]
async fn empty_disabled_and_zero_capacity_snapshots() {
    bounded(async {
        for capacity in [0, 2] {
            let directory = Directory::new();
            let collector = DebugCollector::new(json!({"initial":true}), capacity);
            let server = DebugServer::bind(directory.socket(), collector.clone())
                .await
                .unwrap();
            let mut client = DebugClient::connect(directory.socket()).await.unwrap();
            let snapshot = client.snapshot().await.unwrap();
            assert_eq!(snapshot.state, json!({"initial":true}));
            assert!(snapshot.history.is_empty());
            collector.set_enabled(false);
            collector.publish(DebugRecord::dispatch(
                99,
                "ignored",
                json!(null),
                json!(99),
                true,
            ));
            assert_eq!(client.snapshot().await.unwrap(), snapshot);
            server.shutdown().await.unwrap();
        }
    })
    .await;
}

#[tokio::test]
async fn round_trips_all_outcomes_parentage_and_serialization_errors() {
    bounded(async {
        let directory = Directory::new();
        let collector = DebugCollector::new(json!(0), 8);
        let outcomes = [
            DispatchOutcome::Applied,
            DispatchOutcome::Cancelled,
            DispatchOutcome::Rejected(DispatchError::DepthExceeded {
                max_depth: 3,
                action: "Add",
            }),
            DispatchOutcome::Rejected(DispatchError::ActionBudgetExceeded {
                max_actions: 8,
                processed: 8,
                action: "Add",
            }),
        ];
        for (index, outcome) in outcomes.into_iter().enumerate() {
            let mut record =
                DebugRecord::dispatch(99, "Add", json!({"nested":["λ",true]}), json!(index), false);
            record.outcome = outcome;
            record.parent_action_id = Some(1);
            record
                .serialization_errors
                .push("action: unavailable".into());
            collector.publish(record);
        }
        let server = DebugServer::bind(directory.socket(), collector.clone())
            .await
            .unwrap();
        let snapshot = DebugClient::connect(directory.socket())
            .await
            .unwrap()
            .snapshot()
            .await
            .unwrap();
        assert_eq!(snapshot.history[1].outcome, SnapshotOutcome::Cancelled);
        assert_eq!(
            snapshot.history[2].outcome,
            SnapshotOutcome::DepthExceeded {
                max_depth: 3,
                action: "Add".into()
            }
        );
        assert_eq!(
            snapshot.history[3].outcome,
            SnapshotOutcome::ActionBudgetExceeded {
                max_actions: 8,
                processed: 8,
                action: "Add".into()
            }
        );
        for (wire, original) in snapshot.history.iter().zip(collector.records()) {
            assert_eq!(wire.sequence, original.sequence);
            assert_eq!(wire.action_id, original.action_id);
            assert_eq!(wire.parent_action_id, original.parent_action_id);
            assert_eq!(wire.action, original.action);
            assert_eq!(wire.serialization_errors, original.serialization_errors);
        }
        server.shutdown().await.unwrap();
    })
    .await;
}

#[tokio::test]
async fn refuses_existing_files_symlinks_and_live_or_stale_sockets() {
    bounded(async {
        let directory = Directory::new();
        let path = directory.socket();
        let collector = DebugCollector::new(json!(null), 2);
        fs::write(&path, "keep me").unwrap();
        assert!(DebugServer::bind(&path, collector.clone()).await.is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "keep me");
        fs::remove_file(&path).unwrap();
        let target = directory.0.join("target");
        fs::write(&target, "target data").unwrap();
        symlink(&target, &path).unwrap();
        assert!(DebugServer::bind(&path, collector.clone()).await.is_err());
        assert!(fs::symlink_metadata(&path).unwrap().is_symlink());
        assert_eq!(fs::read_to_string(&target).unwrap(), "target data");
        fs::remove_file(&path).unwrap();
        let server = DebugServer::bind(&path, collector.clone()).await.unwrap();
        assert!(DebugServer::bind(&path, collector.clone()).await.is_err());
        DebugClient::connect(&path)
            .await
            .unwrap()
            .snapshot()
            .await
            .unwrap();
        server.shutdown().await.unwrap();
        drop(UnixListener::bind(&path).unwrap());
        assert!(DebugServer::bind(&path, collector).await.is_err());
        assert!(path.exists());
    })
    .await;
}

#[tokio::test]
async fn drop_cleans_owned_path_and_shutdown_preserves_replacements() {
    bounded(async {
        let directory = Directory::new();
        let path = directory.socket();
        let collector = DebugCollector::new(json!(null), 2);
        let server = DebugServer::bind(&path, collector.clone()).await.unwrap();
        drop(server);
        assert!(!path.exists());
        let server = DebugServer::bind(&path, collector.clone()).await.unwrap();
        fs::remove_file(&path).unwrap();
        fs::write(&path, "replacement").unwrap();
        server.shutdown().await.unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "replacement");
        fs::remove_file(&path).unwrap();
        let server = DebugServer::bind(&path, collector.clone()).await.unwrap();
        fs::remove_file(&path).unwrap();
        let replacement = DebugServer::bind(&path, collector).await.unwrap();
        server.shutdown().await.unwrap();
        DebugClient::connect(&path)
            .await
            .unwrap()
            .snapshot()
            .await
            .unwrap();
        replacement.shutdown().await.unwrap();
        assert!(!path.exists());
    })
    .await;
}

#[tokio::test]
async fn slow_and_malformed_clients_do_not_block_dispatch_or_other_readers() {
    bounded(async {
        let directory = Directory::new();
        let collector = DebugCollector::new(json!(null), 64);
        let large = json!({"data":"s".repeat(65536)});
        for index in 0..64 {
            collector.publish(DebugRecord::dispatch(
                index,
                "Add",
                json!(1),
                large.clone(),
                true,
            ));
        }
        let server = DebugServer::bind(directory.socket(), collector.clone())
            .await
            .unwrap();
        let mut slow = UnixStream::connect(directory.socket()).await.unwrap();
        slow.write_u8(1).await.unwrap();
        // Reading the header proves the server is delivering this snapshot; leave
        // its multi-megabyte body unread to exercise backpressure.
        assert!(slow.read_u32().await.unwrap() > 1_000_000);
        let mut malformed = UnixStream::connect(directory.socket()).await.unwrap();
        malformed.write_u8(255).await.unwrap();
        assert_eq!(malformed.read(&mut [0]).await.unwrap(), 0);
        for index in 0..10000 {
            collector.publish(DebugRecord::dispatch(
                index,
                "Add",
                json!(index),
                json!(index),
                true,
            ));
        }
        let mut normal = DebugClient::connect(directory.socket()).await.unwrap();
        let snapshot = normal.snapshot().await.unwrap();
        assert_eq!(snapshot.history.len(), 64);
        assert_eq!(snapshot.state, json!(9999));
        assert_eq!(snapshot.history.last().unwrap().sequence, 10064);
        assert_eq!(snapshot.state, snapshot.history.last().unwrap().state);
        server.shutdown().await.unwrap();
    })
    .await;
}

#[tokio::test]
async fn caps_idle_connections_without_growing_a_dispatch_queue() {
    bounded(async {
        let directory = Directory::new();
        let collector = DebugCollector::new(json!(0), 2);
        let server = DebugServer::bind(directory.socket(), collector.clone())
            .await
            .unwrap();
        let mut clients = Vec::new();
        for _ in 0..MAX_CLIENTS {
            let mut client = DebugClient::connect(directory.socket()).await.unwrap();
            client.snapshot().await.unwrap();
            clients.push(client);
        }
        assert!(DebugClient::connect(directory.socket())
            .await
            .unwrap()
            .snapshot()
            .await
            .is_err());
        for index in 0..1000 {
            collector.publish(DebugRecord::dispatch(
                index,
                "Add",
                json!(index),
                json!(index),
                true,
            ));
        }
        assert_eq!(collector.records().len(), 2);
        assert_eq!(clients[0].snapshot().await.unwrap().state, json!(999));
        server.shutdown().await.unwrap();
    })
    .await;
}

#[tokio::test]
async fn oversized_server_snapshot_is_rejected_and_server_remains_usable() {
    bounded(async {
        let directory = Directory::new();
        let collector = DebugCollector::new(json!("s".repeat(MAX_SNAPSHOT_BYTES)), 1);
        let server = DebugServer::bind(directory.socket(), collector.clone())
            .await
            .unwrap();
        assert!(DebugClient::connect(directory.socket())
            .await
            .unwrap()
            .snapshot()
            .await
            .is_err());
        collector.publish(DebugRecord::dispatch(
            1,
            "small",
            json!(null),
            json!(42),
            true,
        ));
        assert_eq!(
            DebugClient::connect(directory.socket())
                .await
                .unwrap()
                .snapshot()
                .await
                .unwrap()
                .state,
            json!(42)
        );
        server.shutdown().await.unwrap();
    })
    .await;
}

#[tokio::test]
async fn client_rejects_oversized_or_invalid_frames_and_poisoned_connections() {
    bounded(async {
        for oversized in [false, true] {
            let directory = Directory::new();
            let listener = UnixListener::bind(directory.socket()).unwrap();
            let peer = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                stream.read_u8().await.unwrap();
                if oversized {
                    stream
                        .write_u32(MAX_SNAPSHOT_BYTES as u32 + 1)
                        .await
                        .unwrap();
                } else {
                    stream.write_u32(1).await.unwrap();
                    stream.write_all(b"?").await.unwrap();
                }
            });
            let mut client = DebugClient::connect(directory.socket()).await.unwrap();
            assert_eq!(
                client.snapshot().await.unwrap_err().kind(),
                std::io::ErrorKind::InvalidData
            );
            assert_eq!(
                client.snapshot().await.unwrap_err().kind(),
                std::io::ErrorKind::BrokenPipe
            );
            peer.await.unwrap();
        }
    })
    .await;
}

#[tokio::test]
async fn cancelled_snapshot_requires_reconnect() {
    bounded(async {
        let directory = Directory::new();
        let listener = UnixListener::bind(directory.socket()).unwrap();
        let mut client = DebugClient::connect(directory.socket()).await.unwrap();
        let (mut peer, _) = listener.accept().await.unwrap();
        {
            let snapshot = client.snapshot();
            tokio::pin!(snapshot);
            tokio::select! {
                result = &mut snapshot => panic!("must await a reply: {result:?}"),
                request = peer.read_u8() => assert_eq!(request.unwrap(), 1),
            }
        }
        assert_eq!(
            client.snapshot().await.unwrap_err().kind(),
            std::io::ErrorKind::BrokenPipe
        );
    })
    .await;
}

#[tokio::test]
async fn standalone_binary_attaches_without_a_terminal() {
    bounded(async {
        let directory = Directory::new();
        let collector = DebugCollector::new(json!({"count":7}), 2);
        let server = DebugServer::bind(directory.socket(), collector)
            .await
            .unwrap();
        let path = directory.socket();
        let output = tokio::task::spawn_blocking(move || {
            std::process::Command::new(env!("CARGO_BIN_EXE_tui-dispatch-debug"))
                .arg(path)
                .arg("--once")
                .output()
                .unwrap()
        })
        .await
        .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["state"], json!({"count":7}));
        assert_eq!(value["history"], json!([]));
        server.shutdown().await.unwrap();
    })
    .await;
}
