//! Read-only, pull-based debugger attachment over a private Unix socket.
//!
//! Collection is independent of the server and never waits for a client. Each
//! request copies one consistent retained history; there is no per-action queue.
//! The initial wire format is private to this crate and has no version negotiation.

use std::{
    fs,
    io::{self, Write},
    os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{UnixListener, UnixStream},
    sync::oneshot,
    task::{JoinHandle, JoinSet},
};
use tui_dispatch_core::DispatchError;

use crate::observe::{DebugCollector, DebugRecord, DispatchOutcome};

/// Maximum serialized snapshot size. Oversized replies close the connection.
pub const MAX_SNAPSHOT_BYTES: usize = 16 * 1024 * 1024;
/// Maximum simultaneous clients, including clients that never send a request.
pub const MAX_CLIENTS: usize = 8;
const IO_TIMEOUT: Duration = Duration::from_secs(5);
const SNAPSHOT_REQUEST: u8 = 1;

/// An atomic view of the last observed state and retained dispatch history.
///
/// State is the latest collected boundary, or the supplied initial state before
/// any collection. It may lag the app when collection is disabled. History count
/// is bounded by the collector; snapshots are limited to [`MAX_SNAPSHOT_BYTES`]
/// on the wire. Every request returns the full retained history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DebugSnapshot {
    pub state: Value,
    pub history: Vec<SnapshotRecord>,
}

/// Owned wire representation of a dispatch record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotRecord {
    pub sequence: u64,
    pub action_id: u64,
    pub parent_action_id: Option<u64>,
    pub name: String,
    pub action: Value,
    pub state: Value,
    pub changed: bool,
    pub outcome: SnapshotOutcome,
    pub serialization_errors: Vec<String>,
}

/// Dispatch outcomes with owned rejection details for use across processes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapshotOutcome {
    Applied,
    Cancelled,
    DepthExceeded {
        max_depth: usize,
        action: String,
    },
    ActionBudgetExceeded {
        max_actions: usize,
        processed: usize,
        action: String,
    },
    /// Preserves diagnostic text if core adds another outcome.
    Other(String),
}

impl From<DebugRecord> for SnapshotRecord {
    fn from(record: DebugRecord) -> Self {
        let outcome = match record.outcome {
            DispatchOutcome::Applied => SnapshotOutcome::Applied,
            DispatchOutcome::Cancelled => SnapshotOutcome::Cancelled,
            DispatchOutcome::Rejected(DispatchError::DepthExceeded { max_depth, action }) => {
                SnapshotOutcome::DepthExceeded {
                    max_depth,
                    action: action.into(),
                }
            }
            DispatchOutcome::Rejected(DispatchError::ActionBudgetExceeded {
                max_actions,
                processed,
                action,
            }) => SnapshotOutcome::ActionBudgetExceeded {
                max_actions,
                processed,
                action: action.into(),
            },
            other => SnapshotOutcome::Other(format!("{other:?}")),
        };
        Self {
            sequence: record.sequence,
            action_id: record.action_id,
            parent_action_id: record.parent_action_id,
            name: record.name,
            action: record.action,
            state: record.state,
            changed: record.changed,
            outcome,
            serialization_errors: record.serialization_errors,
        }
    }
}

impl DebugSnapshot {
    fn collect(collector: &DebugCollector) -> Self {
        // One lock acquisition; derive state from the same history, rather than
        // racing a second read against concurrent dispatches.
        let history = collector.records();
        let state = history
            .last()
            .map(|record| &record.state)
            .unwrap_or_else(|| collector.initial_state())
            .clone();
        Self {
            state,
            history: history.into_iter().map(Into::into).collect(),
        }
    }
}

struct SocketPath {
    path: PathBuf,
    device: u64,
    inode: u64,
}

impl Drop for SocketPath {
    fn drop(&mut self) {
        // Never unlink a file/socket that has replaced our directory entry.
        if let Ok(metadata) = fs::symlink_metadata(&self.path) {
            if metadata.file_type().is_socket()
                && metadata.dev() == self.device
                && metadata.ino() == self.inode
            {
                let _ = fs::remove_file(&self.path);
            }
        }
    }
}

/// A read-only attachment server. Keep it alive alongside the app's runtime.
///
/// Binding never removes an existing entry, including a stale socket or symlink.
/// Use an absolute path in a directory owned by the app's user. The socket has
/// mode 0600; its containing directory should be private too. Dropping the server
/// aborts its worker and removes its owned path; `shutdown` also waits for all
/// connections to close. Neither operation disables collection.
pub struct DebugServer {
    worker: JoinHandle<io::Result<()>>,
    stop: Option<oneshot::Sender<()>>,
    _path: SocketPath,
}

impl DebugServer {
    /// Bind a new socket at `path`; requires a running Tokio runtime.
    pub async fn bind(path: impl AsRef<Path>, collector: DebugCollector) -> io::Result<Self> {
        // Resolve relative paths now so later working-directory changes cannot
        // redirect cleanup. Do not canonicalize the final (possibly existing) entry.
        let path = std::path::absolute(path)?;
        let listener = std::os::unix::net::UnixListener::bind(&path)?;
        let metadata = fs::symlink_metadata(&path)?;
        let guard = SocketPath {
            path,
            device: metadata.dev(),
            inode: metadata.ino(),
        };
        fs::set_permissions(&guard.path, fs::Permissions::from_mode(0o600))?;
        listener.set_nonblocking(true)?;
        let listener = UnixListener::from_std(listener)?;
        let (stop, stopped) = oneshot::channel();
        let worker = tokio::spawn(serve(listener, collector, stopped));
        Ok(Self {
            worker,
            stop: Some(stop),
            _path: guard,
        })
    }

    /// Stop accepting, close all clients, and remove the socket path.
    pub async fn shutdown(mut self) -> io::Result<()> {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        (&mut self.worker).await.map_err(io::Error::other)?
    }
}

impl Drop for DebugServer {
    fn drop(&mut self) {
        self.worker.abort();
    }
}

async fn serve(
    listener: UnixListener,
    collector: DebugCollector,
    mut stopped: oneshot::Receiver<()>,
) -> io::Result<()> {
    let mut clients = JoinSet::new();
    let result = loop {
        tokio::select! {
            biased;
            _ = &mut stopped => break Ok(()),
            Some(_) = clients.join_next(), if !clients.is_empty() => {},
            connection = listener.accept() => {
                match connection {
                    Ok((stream, _)) if clients.len() < MAX_CLIENTS => {
                        let collector = collector.clone();
                        clients.spawn(async move { let _ = serve_client(stream, collector).await; });
                    }
                    Ok(_) => {}, // Drop excess connections; never queue snapshots for them.
                    Err(error) => break Err(error),
                }
            }
        }
    };
    clients.abort_all();
    while clients.join_next().await.is_some() {}
    result
}

// Stop serialization as soon as the frame limit is exceeded, rather than first
// allocating an arbitrarily large serialized reply. JSON cloning still scales
// with the app-configured history count and state size.
struct Frame(Vec<u8>);
impl Write for Frame {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_SNAPSHOT_BYTES - self.0.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "debug snapshot exceeds frame limit",
            ));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

async fn serve_client(mut stream: UnixStream, collector: DebugCollector) -> io::Result<()> {
    let mut frame = Frame(Vec::new());
    loop {
        if stream.read_u8().await? != SNAPSHOT_REQUEST {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unknown debugger request",
            ));
        }
        let snapshot = DebugSnapshot::collect(&collector);
        frame.0.clear();
        serde_json::to_writer(&mut frame, &snapshot)?;
        drop(snapshot);
        tokio::time::timeout(IO_TIMEOUT, async {
            stream.write_u32(frame.0.len() as u32).await?;
            stream.write_all(&frame.0).await
        })
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "debugger write timed out"))??;
    }
}

/// Sequential snapshot client. Reconnect by creating another client.
///
/// An I/O error, timeout, or cancelled `snapshot` call invalidates the connection;
/// subsequent calls fail rather than consuming a partially read reply.
pub struct DebugClient {
    stream: UnixStream,
    ready: bool,
    bytes: Vec<u8>,
}

impl DebugClient {
    pub async fn connect(path: impl AsRef<Path>) -> io::Result<Self> {
        Ok(Self {
            stream: UnixStream::connect(path).await?,
            ready: true,
            bytes: Vec::new(),
        })
    }

    /// Request a full snapshot with a five-second I/O deadline.
    pub async fn snapshot(&mut self) -> io::Result<DebugSnapshot> {
        if !self.ready {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "reconnect after an interrupted snapshot",
            ));
        }
        self.ready = false;
        let result = tokio::time::timeout(IO_TIMEOUT, async {
            self.stream.write_u8(SNAPSHOT_REQUEST).await?;
            let len = self.stream.read_u32().await? as usize;
            if len > MAX_SNAPSHOT_BYTES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "debug snapshot exceeds frame limit",
                ));
            }
            self.bytes.resize(len, 0);
            self.stream.read_exact(&mut self.bytes).await?;
            serde_json::from_slice(&self.bytes)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
        })
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "debugger snapshot timed out"))?;
        self.ready = result.is_ok();
        result
    }
}
