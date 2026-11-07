//! Live debugger requests, applied by the app at runtime dispatch boundaries.
use crate::{debug::ComponentSnapshot, observe::DebugCollector};
use serde::{Deserialize, Serialize};
use std::{
    io,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use tokio::sync::Notify;

/// Identifies a component in the app's published frame. Bindings take priority
/// over transient mount/list IDs, so moving entities keep their own bounds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentTarget {
    pub raw_id: u32,
    pub bound_id: Option<String>,
}
impl ComponentTarget {
    pub(crate) fn matches(&self, component: &ComponentSnapshot) -> bool {
        match &self.bound_id {
            Some(id) => component.bound_id.as_ref() == Some(id),
            None => component.raw_id == self.raw_id,
        }
    }
}

/// Commands supported by the live socket connection, independent of app actions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DebugCommand {
    TogglePause,
    SetPaused(bool),
    Highlight(Option<ComponentTarget>),
}

/// `paused` is confirmed at a dispatch boundary. `pause_requested` may briefly
/// differ while the app is finishing its current synchronous dispatch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlStatus {
    pub paused: bool,
    pub pause_requested: bool,
    pub highlight: Option<ComponentTarget>,
}

#[derive(Default)]
struct State {
    attached: bool,
    generation: u64,
    revision: u64,
    paused: bool,
    requested: bool,
    highlight: Option<ComponentTarget>,
    pause_owner: Option<u64>,
    highlight_owner: Option<u64>,
}
pub(crate) struct RemoteShared {
    state: Mutex<State>,
    pub(crate) notify: Arc<Notify>,
}
impl RemoteShared {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State::default()),
            notify: Arc::new(Notify::new()),
        })
    }
    fn status(&self) -> Option<ControlStatus> {
        let state = self.state.lock().unwrap();
        state.attached.then(|| ControlStatus {
            paused: state.paused,
            pause_requested: state.requested,
            highlight: state.highlight.clone(),
        })
    }

    pub(crate) fn redraw(&self) {
        let mut state = self.state.lock().unwrap();
        state.revision = state.revision.wrapping_add(1);
        self.notify.notify_one();
    }
}

impl DebugCollector {
    /// Available only when the app attaches `DebugLayer::with_remote_debug`.
    pub fn control_status(&self) -> Option<ControlStatus> {
        self.remote()?.status()
    }
}

pub(crate) struct RemoteAttachment {
    pub(crate) collector: DebugCollector,
    pub(crate) shared: Arc<RemoteShared>,
    generation: u64,
    seen: u64,
}
impl RemoteAttachment {
    pub(crate) fn new(collector: DebugCollector) -> Self {
        let shared = collector.remote_or_init();
        let generation = {
            let mut state = shared.state.lock().unwrap();
            state.generation = state.generation.wrapping_add(1);
            state.revision = state.revision.wrapping_add(1);
            state.attached = true;
            state.paused = false;
            state.requested = false;
            state.highlight = None;
            state.pause_owner = None;
            state.highlight_owner = None;
            state.generation
        };
        Self {
            collector,
            shared,
            generation,
            seen: u64::MAX,
        }
    }
    pub(crate) fn pending(&mut self) -> Option<bool> {
        let state = self.shared.state.lock().unwrap();
        if state.generation != self.generation || self.seen == state.revision {
            return None;
        }
        self.seen = state.revision;
        Some(state.requested)
    }
    pub(crate) fn applied(&self, paused: bool) {
        let mut state = self.shared.state.lock().unwrap();
        if state.generation == self.generation {
            state.paused = paused;
        }
    }
    pub(crate) fn paused(&self) -> bool {
        let state = self.shared.state.lock().unwrap();
        state.attached && state.generation == self.generation && state.paused
    }
    pub(crate) fn highlight(&self) -> Option<ratatui::layout::Rect> {
        let target = self.shared.status()?.highlight?;
        self.collector.component_area(&target)
    }
}
impl Drop for RemoteAttachment {
    fn drop(&mut self) {
        let mut state = self.shared.state.lock().unwrap();
        if state.generation == self.generation {
            state.attached = false;
            state.requested = false;
            state.paused = false;
            state.highlight = None;
        }
    }
}

/// Connection ownership: disconnect resumes/clears only controls last written
/// by this connection. Snapshot-only readers cannot disturb another client.
pub(crate) struct ControlSession {
    collector: DebugCollector,
    id: u64,
}
impl ControlSession {
    pub(crate) fn new(collector: DebugCollector) -> io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let id = NEXT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map_err(|_| io::Error::other("debug control session IDs exhausted"))?;
        Ok(Self { collector, id })
    }
    pub(crate) fn command(&self, command: DebugCommand) -> io::Result<ControlStatus> {
        let shared = self.collector.remote().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Unsupported,
                "app has not attached remote debug controls",
            )
        })?;
        if let DebugCommand::Highlight(Some(target)) = &command {
            if !self
                .collector
                .components()
                .unwrap_or_default()
                .iter()
                .any(|component| target.matches(component))
            {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "component is no longer published",
                ));
            }
        }
        {
            let mut state = shared.state.lock().unwrap();
            if !state.attached {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "app runtime is no longer attached",
                ));
            }
            match command {
                DebugCommand::TogglePause => {
                    state.requested = !state.requested;
                    state.pause_owner = Some(self.id);
                }
                DebugCommand::SetPaused(paused) => {
                    state.requested = paused;
                    state.pause_owner = Some(self.id);
                }
                DebugCommand::Highlight(target) => {
                    state.highlight = target;
                    state.highlight_owner = Some(self.id);
                }
            }
            state.revision = state.revision.wrapping_add(1);
        }
        shared.notify.notify_one();
        shared
            .status()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "app runtime detached"))
    }
}
impl Drop for ControlSession {
    fn drop(&mut self) {
        let Some(shared) = self.collector.remote() else {
            return;
        };
        let mut state = shared.state.lock().unwrap();
        let mut changed = false;
        if state.pause_owner == Some(self.id) {
            state.requested = false;
            state.pause_owner = None;
            changed = true;
        }
        if state.highlight_owner == Some(self.id) {
            state.highlight = None;
            state.highlight_owner = None;
            changed = true;
        }
        if changed {
            state.revision = state.revision.wrapping_add(1);
            shared.notify.notify_one();
        }
    }
}
