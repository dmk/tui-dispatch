//! Bounded dispatch collection without an overlay, terminal, or connected client.

use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
};

use crate::debug::ComponentSnapshot;
use serde::Serialize;
use serde_json::Value;
use tui_dispatch_core::{Action, Middleware, Store, StoreWithMiddleware};

pub use tui_dispatch_core::observe::{DispatchObservation, DispatchObserver, DispatchOutcome};

/// An owned snapshot of one action's dispatch boundary.
#[derive(Debug, Clone, PartialEq)]
pub struct DebugRecord {
    /// Monotonically increasing within this collector, starting at one.
    pub sequence: u64,
    /// Action identity within this collector (currently equal to `sequence`).
    pub action_id: u64,
    /// The injecting action, if observed by this observer handle.
    pub parent_action_id: Option<u64>,
    /// The action's stable variant name.
    pub name: String,
    /// Serialized action, including its payload.
    pub action: Value,
    /// State immediately after this action, before injected actions run.
    pub state: Value,
    /// Whether this action's reducer reported a change, excluding children.
    pub changed: bool,
    /// Applied, cancelled, or rejected by dispatch limits.
    pub outcome: DispatchOutcome,
    /// Serialization failures; the corresponding value is `null`.
    pub serialization_errors: Vec<String>,
}

impl DebugRecord {
    /// Construct an applied dispatch record for a custom producer.
    /// [`DebugCollector::publish`] assigns collector-local sequence/action IDs.
    pub fn dispatch(
        sequence: u64,
        name: impl Into<String>,
        action: Value,
        state: Value,
        changed: bool,
    ) -> Self {
        Self {
            sequence,
            action_id: sequence,
            parent_action_id: None,
            name: name.into(),
            action,
            state,
            changed,
            outcome: DispatchOutcome::Applied,
            serialization_errors: Vec::new(),
        }
    }
}

struct History {
    records: VecDeque<DebugRecord>,
    next_sequence: u64,
    generation: u64,
}

struct Shared {
    enabled: AtomicBool,
    initial_state: Value,
    capacity: usize,
    history: Mutex<History>,
    components: Mutex<Option<Vec<ComponentSnapshot>>>,
    remote: OnceLock<Arc<crate::control::RemoteShared>>,
}

/// Shared, bounded history with no dependency on rendering.
///
/// Oldest records are evicted when capacity is reached. Capacity bounds record
/// count, not bytes: full JSON snapshots can be large. Clones share collection
/// state; each `observer()` has its own injection ancestry. Disabled or zero
/// capacity collection performs no action/state serialization.
#[derive(Clone)]
pub struct DebugCollector {
    shared: Arc<Shared>,
}

impl DebugCollector {
    /// Create a collector with an explicit initial snapshot and record capacity.
    pub fn new(initial_state: Value, capacity: usize) -> Self {
        Self {
            shared: Arc::new(Shared {
                enabled: AtomicBool::new(capacity > 0),
                initial_state,
                capacity,
                components: Mutex::new(None),
                remote: OnceLock::new(),
                history: Mutex::new(History {
                    records: VecDeque::new(),
                    next_sequence: 1,
                    generation: 0,
                }),
            }),
        }
    }

    /// Initial snapshot supplied at construction (not retained again per action).
    pub fn initial_state(&self) -> &Value {
        &self.shared.initial_state
    }

    /// Create a handle to attach to one store. Handles share history and IDs.
    pub fn observer(&self) -> DebugObserver {
        DebugObserver {
            collector: self.clone(),
            ancestors: Vec::new(),
            generation: 0,
        }
    }

    /// Copy the retained records in dispatch order.
    pub fn records(&self) -> Vec<DebugRecord> {
        self.shared
            .history
            .lock()
            .unwrap()
            .records
            .iter()
            .cloned()
            .collect()
    }

    /// Replace component telemetry with the latest complete rendered frame.
    /// Call on the UI thread after rendering, using a component registry or
    /// `ComponentHost::mounted_components()`. Socket workers only copy this
    /// owned snapshot; they never access UI objects on another thread.
    /// Disabled collection retains its last published frame.
    pub fn set_components(&self, components: Vec<ComponentSnapshot>) {
        if self.is_enabled() {
            *self.shared.components.lock().unwrap() = Some(components);
        }
    }

    /// Latest component frame, or `None` if the app has never published one.
    /// An empty published frame is distinct from unavailable telemetry.
    pub fn components(&self) -> Option<Vec<ComponentSnapshot>> {
        self.shared.components.lock().unwrap().clone()
    }

    pub(crate) fn remote(&self) -> Option<&Arc<crate::control::RemoteShared>> {
        self.shared.remote.get()
    }

    pub(crate) fn remote_or_init(&self) -> Arc<crate::control::RemoteShared> {
        self.shared
            .remote
            .get_or_init(crate::control::RemoteShared::new)
            .clone()
    }

    pub(crate) fn component_area(
        &self,
        target: &crate::control::ComponentTarget,
    ) -> Option<ratatui::layout::Rect> {
        self.shared
            .components
            .lock()
            .unwrap()
            .as_ref()?
            .iter()
            .find(|component| target.matches(component))
            .and_then(|component| component.last_area)
    }

    /// Publish an already serialized boundary from a custom producer.
    ///
    /// Prefer `observer()` for stores. Disabled collection ignores the record.
    /// The supplied sequence and action ID are replaced with collector-local IDs;
    /// an optional parent ID must already refer to this collector's ID space.
    pub fn publish(&self, mut record: DebugRecord) {
        let mut history = self.shared.history.lock().unwrap();
        if !self.shared.enabled.load(Ordering::Acquire) {
            return;
        }
        let sequence = history.next_sequence;
        let Some(next) = sequence.checked_add(1) else {
            self.shared.enabled.store(false, Ordering::Release);
            return;
        };
        history.next_sequence = next;
        record.sequence = sequence;
        record.action_id = sequence;
        if history.records.len() == self.shared.capacity {
            history.records.pop_front();
        }
        history.records.push_back(record);
    }

    /// Enable or disable new collection, retaining existing history.
    pub fn set_enabled(&self, enabled: bool) {
        let mut history = self.shared.history.lock().unwrap();
        self.shared
            .enabled
            .store(enabled && self.shared.capacity > 0, Ordering::Release);
        history.generation = history.generation.wrapping_add(1);
    }

    /// Whether new dispatches will be collected.
    pub fn is_enabled(&self) -> bool {
        self.shared.enabled.load(Ordering::Acquire)
    }
}

/// Store observation handle produced by [`DebugCollector::observer`].
///
/// State and actions must serialize; effects need no serialization or cloning.
/// Serialization failures are recorded explicitly and never cancel reduction.
pub struct DebugObserver {
    collector: DebugCollector,
    ancestors: Vec<u64>,
    generation: u64,
}

impl<S: Serialize, A: Action + Serialize> DispatchObserver<S, A> for DebugObserver {
    fn is_enabled(&self) -> bool {
        self.collector.is_enabled()
    }

    fn observe(&mut self, action: &A, state: &S, observation: DispatchObservation) {
        if !self.collector.is_enabled() {
            return;
        }
        // User serializers run outside the history lock. Readers only contend
        // with bounded history insertion, not full snapshot construction.
        let mut serialization_errors = Vec::new();
        let mut serialize = |value: Result<Value, serde_json::Error>, label: &str| {
            value.unwrap_or_else(|error| {
                serialization_errors.push(format!("{label}: {error}"));
                Value::Null
            })
        };
        let action_value = serialize(serde_json::to_value(action), "action");
        let state_value = serialize(serde_json::to_value(state), "state");
        let name = action.name().to_owned();
        let shared = &self.collector.shared;
        let mut history = shared.history.lock().unwrap();
        if !shared.enabled.load(Ordering::Acquire) {
            return;
        }
        if self.generation != history.generation {
            self.ancestors.clear();
            self.generation = history.generation;
        }
        let sequence = history.next_sequence;
        let Some(next) = sequence.checked_add(1) else {
            shared.enabled.store(false, Ordering::Release);
            return;
        };
        history.next_sequence = next;
        self.ancestors.truncate(observation.depth);
        let parent_action_id = observation
            .depth
            .checked_sub(1)
            .and_then(|index| self.ancestors.get(index))
            .copied();
        // Missing ancestors can occur if collection was enabled inside a chain.
        self.ancestors.resize(observation.depth, 0);
        self.ancestors.push(sequence);
        let parent_action_id = parent_action_id.filter(|id| *id != 0);
        let record = DebugRecord {
            sequence,
            action_id: sequence,
            parent_action_id,
            name,
            action: action_value,
            state: state_value,
            changed: observation.changed,
            outcome: observation.outcome,
            serialization_errors,
        };
        if history.records.len() == shared.capacity {
            history.records.pop_front();
        }
        history.records.push_back(record);
    }
}

/// Attach passive collection to a store before passing it to `Runtime::from_store`.
pub trait DebugStoreExt<S, A: Action>: Sized {
    /// Install an observer without changing middleware or dispatch behavior.
    fn with_observer(self, observer: impl DispatchObserver<S, A> + 'static) -> Self;
}

impl<S, A: Action, E> DebugStoreExt<S, A> for Store<S, A, E> {
    fn with_observer(mut self, observer: impl DispatchObserver<S, A> + 'static) -> Self {
        self.set_observer(observer);
        self
    }
}

impl<S, A: Action, E, M: Middleware<S, A>> DebugStoreExt<S, A> for StoreWithMiddleware<S, A, E, M> {
    fn with_observer(mut self, observer: impl DispatchObserver<S, A> + 'static) -> Self {
        self.set_observer(observer);
        self
    }
}
