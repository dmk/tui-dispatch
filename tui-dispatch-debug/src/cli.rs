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

    /// Wait N seconds before rendering once (lets async effects finish)
    #[arg(long = "debug-render-wait", default_value_t = 0)]
    pub render_wait: u64,

    /// Load initial state from a RON snapshot
    #[arg(long = "debug-state-in")]
    pub state_in: Option<PathBuf>,

    /// Load and replay actions from a RON snapshot
    #[arg(long = "debug-actions-in")]
    pub actions_in: Option<PathBuf>,

    /// Save dispatched actions to a RON snapshot
    #[arg(long = "debug-actions-out")]
    pub actions_out: Option<PathBuf>,

    /// Include action patterns when recording debug actions (comma-separated)
    #[arg(long = "debug-actions-include")]
    pub actions_include: Option<String>,

    /// Exclude action patterns when recording debug actions (comma-separated)
    #[arg(long = "debug-actions-exclude")]
    pub actions_exclude: Option<String>,
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
