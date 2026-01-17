//!
//! Debugging utilities for tui-dispatch.
//!
//! This crate will host headless debug sessions, snapshot tooling, and
//! action replay helpers built on top of tui-dispatch-core.

pub mod debug;
pub mod snapshot;

pub use snapshot::{
    load_ron, save_ron, ActionSnapshot, SnapshotError, SnapshotResult, StateSnapshot,
};
