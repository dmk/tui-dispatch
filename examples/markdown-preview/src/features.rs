//! Feature flags for markdown preview

use tui_dispatch::FeatureFlags;

/// Feature flags for the markdown viewer
#[derive(FeatureFlags)]
pub struct Features {
    /// Show line numbers in the gutter
    #[flag(default = false)]
    pub line_numbers: bool,

    /// Wrap long lines instead of truncating
    #[flag(default = true)]
    pub wrap_lines: bool,

    /// Show document statistics in status bar
    #[flag(default = true)]
    pub show_stats: bool,
}
