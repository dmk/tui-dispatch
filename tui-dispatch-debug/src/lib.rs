//!
//! Debugging utilities for tui-dispatch.
//!
//! This crate will host headless debug sessions, snapshot tooling, and
//! action replay helpers built on top of tui-dispatch-core.

pub mod cli;
pub mod debug;
pub mod session;
pub mod snapshot;

pub use cli::DebugCliArgs;
pub use session::{DebugActionRecorder, DebugRunOutput, DebugSession, DebugSessionError};
pub use snapshot::{
    load_ron, save_ron, ActionSnapshot, SnapshotError, SnapshotResult, StateSnapshot,
};
