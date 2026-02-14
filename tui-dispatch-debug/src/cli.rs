use clap::Args;
use std::path::PathBuf;

use crate::debug::ActionLoggerConfig;
use crate::pattern_utils::split_patterns_csv;

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

    /// Include action categories when recording debug actions (comma-separated)
    #[arg(long = "debug-actions-include-categories")]
    pub actions_include_categories: Option<String>,

    /// Exclude action categories when recording debug actions (comma-separated)
    #[arg(long = "debug-actions-exclude-categories")]
    pub actions_exclude_categories: Option<String>,

    /// Include exact action names when recording debug actions (comma-separated)
    #[arg(long = "debug-actions-include-names")]
    pub actions_include_names: Option<String>,

    /// Exclude exact action names when recording debug actions (comma-separated)
    #[arg(long = "debug-actions-exclude-names")]
    pub actions_exclude_names: Option<String>,

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
        let mut include_patterns = split_patterns(self.actions_include.as_deref());
        include_patterns.extend(category_filters(self.actions_include_categories.as_deref()));
        include_patterns.extend(name_filters(self.actions_include_names.as_deref()));

        let mut exclude_patterns = split_patterns(self.actions_exclude.as_deref());
        exclude_patterns.extend(category_filters(self.actions_exclude_categories.as_deref()));
        exclude_patterns.extend(name_filters(self.actions_exclude_names.as_deref()));

        if include_patterns.is_empty() && exclude_patterns.is_empty() {
            ActionLoggerConfig::default()
        } else {
            ActionLoggerConfig::with_patterns(include_patterns, exclude_patterns)
        }
    }

    pub fn auto_fetch(&self) -> bool {
        self.state_in.is_none() && self.actions_in.is_none()
    }
}

fn split_patterns(value: Option<&str>) -> Vec<String> {
    value.map(split_patterns_csv).unwrap_or_default()
}

fn category_filters(value: Option<&str>) -> Vec<String> {
    split_patterns(value)
        .into_iter()
        .map(|category| format!("cat:{}", category.to_ascii_lowercase()))
        .collect()
}

fn name_filters(value: Option<&str>) -> Vec<String> {
    split_patterns(value)
        .into_iter()
        .map(|name| format!("name:{name}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_filter_defaults() {
        let args = DebugCliArgs::default();
        let filter = args.action_filter();
        assert!(!filter.should_log("Tick"));
        assert!(filter.should_log("Connect"));
    }

    #[test]
    fn test_action_filter_include_disables_default_excludes() {
        let args = DebugCliArgs {
            actions_include: Some("Tick".to_string()),
            ..DebugCliArgs::default()
        };
        let filter = args.action_filter();
        assert!(filter.should_log("Tick"));
    }

    #[test]
    fn test_action_filter_category_and_name_flags() {
        let args = DebugCliArgs {
            actions_include_categories: Some("search".to_string()),
            actions_exclude_names: Some("SearchSubmit".to_string()),
            ..DebugCliArgs::default()
        };
        let filter = args.action_filter();
        assert!(filter.should_log("SearchStart"));
        assert!(!filter.should_log("SearchSubmit"));
        assert!(!filter.should_log("Connect"));
    }
}
