//! Action logging with pattern-based filtering
//!
//! Provides configurable action logging using glob patterns to include/exclude
//! specific actions from logs.
//!
//! # Example
//!
//! ```ignore
//! use tui_dispatch_core::debug::{ActionLoggerConfig, ActionLoggerMiddleware};
//!
//! // Log all actions except Tick and Render
//! let config = ActionLoggerConfig::default();
//!
//! // Log only Search* and Connect* actions
//! let config = ActionLoggerConfig::new(Some("Search*,Connect*"), None);
//!
//! // Log Did* actions but exclude DidFail*
//! let config = ActionLoggerConfig::new(Some("Did*"), Some("DidFail*"));
//!
//! let middleware = ActionLoggerMiddleware::new(config);
//! ```

use crate::Action;
use crate::store::Middleware;

/// Configuration for action logging with glob pattern filtering.
///
/// Patterns support:
/// - `*` matches any sequence of characters
/// - `?` matches any single character
/// - Literal text matches exactly
///
/// # Examples
///
/// - `Search*` matches SearchAddChar, SearchDeleteChar, etc.
/// - `Did*` matches DidConnect, DidScanKeys, etc.
/// - `*Error*` matches any action containing "Error"
/// - `Tick` matches only Tick
#[derive(Debug, Clone)]
pub struct ActionLoggerConfig {
    /// If non-empty, only log actions matching these patterns
    pub include_patterns: Vec<String>,
    /// Exclude actions matching these patterns (applied after include)
    pub exclude_patterns: Vec<String>,
}

impl Default for ActionLoggerConfig {
    fn default() -> Self {
        Self {
            include_patterns: Vec::new(),
            // By default, exclude noisy high-frequency actions
            exclude_patterns: vec!["Tick".to_string(), "Render".to_string()],
        }
    }
}

impl ActionLoggerConfig {
    /// Create a new config from comma-separated pattern strings
    ///
    /// # Arguments
    /// - `include`: comma-separated glob patterns (or None for all)
    /// - `exclude`: comma-separated glob patterns (or None for default excludes)
    ///
    /// # Example
    /// ```
    /// use tui_dispatch_core::debug::ActionLoggerConfig;
    ///
    /// let config = ActionLoggerConfig::new(Some("Search*,Connect"), Some("Tick,Render"));
    /// assert!(config.should_log("SearchAddChar"));
    /// assert!(config.should_log("Connect"));
    /// assert!(!config.should_log("Tick"));
    /// ```
    pub fn new(include: Option<&str>, exclude: Option<&str>) -> Self {
        let include_patterns = include
            .map(|s| s.split(',').map(|p| p.trim().to_string()).collect())
            .unwrap_or_default();

        let exclude_patterns = exclude
            .map(|s| s.split(',').map(|p| p.trim().to_string()).collect())
            .unwrap_or_else(|| vec!["Tick".to_string(), "Render".to_string()]);

        Self {
            include_patterns,
            exclude_patterns,
        }
    }

    /// Create a config with specific pattern vectors
    pub fn with_patterns(include: Vec<String>, exclude: Vec<String>) -> Self {
        Self {
            include_patterns: include,
            exclude_patterns: exclude,
        }
    }

    /// Check if an action name should be logged based on include/exclude patterns
    pub fn should_log(&self, action_name: &str) -> bool {
        // If include patterns specified, action must match at least one
        if !self.include_patterns.is_empty() {
            let matches_include = self
                .include_patterns
                .iter()
                .any(|p| glob_match(p, action_name));
            if !matches_include {
                return false;
            }
        }

        // Check exclude patterns
        let matches_exclude = self
            .exclude_patterns
            .iter()
            .any(|p| glob_match(p, action_name));

        !matches_exclude
    }
}

/// Middleware that logs actions with configurable pattern filtering.
///
/// Uses `tracing::debug!` for output, so actions are only logged when
/// the tracing subscriber is configured to capture debug level messages.
///
/// # Example
///
/// ```ignore
/// use tui_dispatch_core::debug::{ActionLoggerConfig, ActionLoggerMiddleware};
/// use tui_dispatch_core::{Store, StoreWithMiddleware};
///
/// let config = ActionLoggerConfig::new(Some("User*"), None);
/// let middleware = ActionLoggerMiddleware::new(config);
/// let store = StoreWithMiddleware::new(state, reducer, middleware);
/// ```
#[derive(Debug, Clone)]
pub struct ActionLoggerMiddleware {
    config: ActionLoggerConfig,
}

impl ActionLoggerMiddleware {
    /// Create a new action logger middleware with the given config
    pub fn new(config: ActionLoggerConfig) -> Self {
        Self { config }
    }

    /// Create with default config (excludes Tick and Render)
    pub fn default_filtering() -> Self {
        Self::new(ActionLoggerConfig::default())
    }

    /// Create with no filtering (logs all actions)
    pub fn log_all() -> Self {
        Self::new(ActionLoggerConfig::with_patterns(vec![], vec![]))
    }

    /// Get a reference to the config
    pub fn config(&self) -> &ActionLoggerConfig {
        &self.config
    }

    /// Get a mutable reference to the config
    pub fn config_mut(&mut self) -> &mut ActionLoggerConfig {
        &mut self.config
    }
}

impl<A: Action> Middleware<A> for ActionLoggerMiddleware {
    fn before(&mut self, action: &A) {
        let name = action.name();
        if self.config.should_log(name) {
            tracing::debug!(action = %name, "action");
        }
    }

    fn after(&mut self, _action: &A, _state_changed: bool) {
        // No-op - we log before dispatch only
    }
}

/// Simple glob pattern matching supporting `*` and `?`.
///
/// - `*` matches zero or more characters
/// - `?` matches exactly one character
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    glob_match_impl(&pattern, &text)
}

fn glob_match_impl(pattern: &[char], text: &[char]) -> bool {
    let mut pi = 0;
    let mut ti = 0;
    let mut star_pi = None;
    let mut star_ti = 0;

    while ti < text.len() {
        if pi < pattern.len() && (pattern[pi] == '?' || pattern[pi] == text[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < pattern.len() && pattern[pi] == '*' {
            star_pi = Some(pi);
            star_ti = ti;
            pi += 1;
        } else if let Some(spi) = star_pi {
            pi = spi + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }

    while pi < pattern.len() && pattern[pi] == '*' {
        pi += 1;
    }

    pi == pattern.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_match_exact() {
        assert!(glob_match("Tick", "Tick"));
        assert!(!glob_match("Tick", "Tock"));
        assert!(!glob_match("Tick", "TickTock"));
    }

    #[test]
    fn test_glob_match_star() {
        assert!(glob_match("Search*", "SearchAddChar"));
        assert!(glob_match("Search*", "SearchDeleteChar"));
        assert!(glob_match("Search*", "Search"));
        assert!(!glob_match("Search*", "StartSearch"));

        assert!(glob_match("*Search", "StartSearch"));
        assert!(glob_match("*Search*", "StartSearchNow"));

        assert!(glob_match("Did*", "DidConnect"));
        assert!(glob_match("Did*", "DidScanKeys"));
    }

    #[test]
    fn test_glob_match_question() {
        assert!(glob_match("Tick?", "Ticks"));
        assert!(!glob_match("Tick?", "Tick"));
        assert!(!glob_match("Tick?", "Tickss"));
    }

    #[test]
    fn test_glob_match_combined() {
        assert!(glob_match("*Add*", "SearchAddChar"));
        assert!(glob_match("Connection*Add*", "ConnectionFormAddChar"));
    }

    #[test]
    fn test_action_logger_config_include() {
        let config = ActionLoggerConfig::new(Some("Search*,Connect"), None);
        assert!(config.should_log("SearchAddChar"));
        assert!(config.should_log("Connect"));
        assert!(!config.should_log("Tick"));
        assert!(!config.should_log("LoadKeys"));
    }

    #[test]
    fn test_action_logger_config_exclude() {
        let config = ActionLoggerConfig::new(None, Some("Tick,Render,LoadValue*"));
        assert!(!config.should_log("Tick"));
        assert!(!config.should_log("Render"));
        assert!(!config.should_log("LoadValueDebounced"));
        assert!(config.should_log("SearchAddChar"));
        assert!(config.should_log("Connect"));
    }

    #[test]
    fn test_action_logger_config_include_and_exclude() {
        // Include Did* but exclude DidFail*
        let config = ActionLoggerConfig::new(Some("Did*"), Some("DidFail*"));
        assert!(config.should_log("DidConnect"));
        assert!(config.should_log("DidScanKeys"));
        assert!(!config.should_log("DidFailConnect"));
        assert!(!config.should_log("DidFailScanKeys"));
        assert!(!config.should_log("SearchAddChar")); // Not in include
    }

    #[test]
    fn test_action_logger_config_default() {
        let config = ActionLoggerConfig::default();
        assert!(!config.should_log("Tick"));
        assert!(!config.should_log("Render"));
        assert!(config.should_log("Connect"));
        assert!(config.should_log("SearchAddChar"));
    }
}
