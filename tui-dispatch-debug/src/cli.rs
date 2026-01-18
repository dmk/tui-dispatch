use clap::Args;
use std::path::PathBuf;

use crate::debug::ActionLoggerConfig;

/// Shared CLI flags for debug tooling.
#[derive(Args, Debug, Clone, Default)]
#[command(next_help_heading = "Debug")]
pub struct DebugCliArgs {
    /// Enable debug mode (F12 to toggle overlay)
    #[arg(long = "debug")]
    pub enabled: bool,

    /// Render a single frame and exit (after applying debug state/actions)
    #[arg(long = "debug-render-once")]
    pub render_once: bool,

    /// Load initial state from a JSON snapshot
    #[arg(long = "debug-state-in")]
    pub state_in: Option<PathBuf>,

    /// Load and replay actions from a JSON snapshot
    #[arg(long = "debug-actions-in")]
    pub actions_in: Option<PathBuf>,

    /// Save dispatched actions to a JSON snapshot
    #[arg(long = "debug-actions-out")]
    pub actions_out: Option<PathBuf>,

    /// Include action patterns when recording debug actions (comma-separated)
    #[arg(long = "debug-actions-include")]
    pub actions_include: Option<String>,

    /// Exclude action patterns when recording debug actions (comma-separated)
    #[arg(long = "debug-actions-exclude")]
    pub actions_exclude: Option<String>,

    /// Save JSON schema for state type to file
    #[arg(long = "debug-state-schema-out")]
    pub state_schema_out: Option<PathBuf>,

    /// Save JSON schema for action type to file
    #[arg(long = "debug-actions-schema-out")]
    pub actions_schema_out: Option<PathBuf>,

    /// Timeout in seconds for awaiting async actions during replay
    #[arg(long = "debug-replay-timeout", default_value_t = 30)]
    pub replay_timeout: u64,
}

impl DebugCliArgs {
    pub fn action_filter(&self) -> ActionLoggerConfig {
        match (
            self.actions_include.as_deref(),
            self.actions_exclude.as_deref(),
        ) {
            (None, None) => ActionLoggerConfig::default(),
            (Some(include), None) => {
                ActionLoggerConfig::with_patterns(split_patterns(include), Vec::new())
            }
            (include, Some(exclude)) => ActionLoggerConfig::new(include, Some(exclude)),
        }
    }

    pub fn auto_fetch(&self) -> bool {
        self.state_in.is_none() && self.actions_in.is_none()
    }
}

fn split_patterns(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|pattern| pattern.trim())
        .filter(|pattern| !pattern.is_empty())
        .map(|pattern| pattern.to_string())
        .collect()
}
