//!
//! Debugging utilities for tui-dispatch.
//!
//! This crate will host headless debug sessions, snapshot tooling, and
//! action replay helpers built on top of tui-dispatch-core.

extern crate self as tui_dispatch_debug;

pub mod cli;
pub mod debug;
pub mod observe;
mod pattern_utils;
pub mod replay;
pub mod session;
pub mod snapshot;

pub use cli::DebugCliArgs;
pub use replay::ReplayItem;
pub use session::{DebugActionRecorder, DebugRunOutput, DebugSession, DebugSessionError};
pub use snapshot::{
    load_json, save_json, ActionSnapshot, SnapshotError, SnapshotResult, StateSnapshot,
};

/// Implementation details used by generated derive code.
#[doc(hidden)]
pub mod __macro_support {
    pub use crate::debug::{
        debug_string, debug_string_pretty, DebugEntry, DebugSection, DebugState,
    };
}

#[cfg(feature = "json-schema")]
pub use snapshot::{generate_schema, save_replay_schema, save_schema, schema_json, JsonSchema};
