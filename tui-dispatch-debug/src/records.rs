//! Owned debug records shared by socket snapshots and recording files.

use crate::observe::{DebugRecord, DispatchOutcome};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tui_dispatch_core::DispatchError;

/// An atomic view of the last observed state and retained dispatch history.
///
/// State is the latest collected boundary, or the supplied initial state before
/// any collection. It may lag the app when collection is disabled. History count
/// is bounded by the collector; snapshots are limited to `transport::MAX_SNAPSHOT_BYTES`
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

impl From<DispatchOutcome> for SnapshotOutcome {
    fn from(outcome: DispatchOutcome) -> Self {
        match outcome {
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
        }
    }
}

impl From<DebugRecord> for SnapshotRecord {
    fn from(record: DebugRecord) -> Self {
        Self {
            sequence: record.sequence,
            action_id: record.action_id,
            parent_action_id: record.parent_action_id,
            name: record.name,
            action: record.action,
            state: record.state,
            changed: record.changed,
            outcome: record.outcome.into(),
            serialization_errors: record.serialization_errors,
        }
    }
}

// Borrow JSON payloads when writing a collected record; do not clone snapshots.
#[derive(Serialize)]
pub(crate) struct RecordRef<'a> {
    sequence: u64,
    action_id: u64,
    parent_action_id: Option<u64>,
    name: &'a str,
    action: &'a Value,
    state: &'a Value,
    changed: bool,
    outcome: SnapshotOutcome,
    serialization_errors: &'a [String],
}

impl<'a> From<&'a DebugRecord> for RecordRef<'a> {
    fn from(record: &'a DebugRecord) -> Self {
        Self {
            sequence: record.sequence,
            action_id: record.action_id,
            parent_action_id: record.parent_action_id,
            name: &record.name,
            action: &record.action,
            state: &record.state,
            changed: record.changed,
            outcome: record.outcome.clone().into(),
            serialization_errors: &record.serialization_errors,
        }
    }
}
