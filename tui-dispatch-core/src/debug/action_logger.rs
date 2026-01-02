//! Action logging with pattern-based filtering and in-memory storage
//!
//! Provides configurable action logging using glob patterns to include/exclude
//! specific actions from logs. Supports both tracing output and in-memory
//! ring buffer storage for display in debug overlays.
//!
//! # Example
//!
//! ```ignore
//! use tui_dispatch_core::debug::{ActionLoggerConfig, ActionLoggerMiddleware, ActionLogConfig};
//!
//! // Log all actions except Tick and Render (tracing only)
//! let config = ActionLoggerConfig::default();
//! let middleware = ActionLoggerMiddleware::new(config);
//!
//! // Log with in-memory storage for debug overlay display
//! let config = ActionLogConfig::default();
//! let middleware = ActionLoggerMiddleware::with_log(config);
//!
//! // Access the action log
//! if let Some(log) = middleware.log() {
//!     for entry in log.recent(10) {
//!         println!("{}: {}", entry.elapsed_display(), entry.summary);
//!     }
//! }
//! ```

use crate::action::ActionSummary;
use crate::store::Middleware;
use std::collections::VecDeque;
use std::time::Instant;

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

// ============================================================================
// In-Memory Action Log
// ============================================================================

/// An entry in the action log
#[derive(Debug, Clone)]
pub struct ActionLogEntry {
    /// Action name (from Action::name())
    pub name: &'static str,
    /// Summary representation (from ActionSummary::summary())
    pub summary: String,
    /// Timestamp when the action was logged
    pub timestamp: Instant,
    /// Sequence number for ordering
    pub sequence: u64,
    /// Whether the action caused a state change (set after reducer runs)
    pub state_changed: Option<bool>,
}

impl ActionLogEntry {
    /// Create a new log entry
    pub fn new(name: &'static str, summary: String, sequence: u64) -> Self {
        Self {
            name,
            summary,
            timestamp: Instant::now(),
            sequence,
            state_changed: None,
        }
    }

    /// Time since this action was logged
    pub fn elapsed(&self) -> std::time::Duration {
        self.timestamp.elapsed()
    }

    /// Format the elapsed time for display (e.g., "2.3s", "150ms")
    pub fn elapsed_display(&self) -> String {
        let elapsed = self.elapsed();
        if elapsed.as_secs() >= 1 {
            format!("{:.1}s", elapsed.as_secs_f64())
        } else {
            format!("{}ms", elapsed.as_millis())
        }
    }
}

/// Configuration for the action log ring buffer
#[derive(Debug, Clone)]
pub struct ActionLogConfig {
    /// Maximum number of entries to keep
    pub capacity: usize,
    /// Filter config (reuses existing ActionLoggerConfig)
    pub filter: ActionLoggerConfig,
}

impl Default for ActionLogConfig {
    fn default() -> Self {
        Self {
            capacity: 100,
            filter: ActionLoggerConfig::default(),
        }
    }
}

impl ActionLogConfig {
    /// Create with custom capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            ..Default::default()
        }
    }

    /// Create with custom capacity and filter
    pub fn new(capacity: usize, filter: ActionLoggerConfig) -> Self {
        Self { capacity, filter }
    }
}

/// In-memory ring buffer for storing recent actions
///
/// Stores actions with timestamps and summaries for display in debug overlays.
/// Older entries are automatically discarded when capacity is reached.
#[derive(Debug, Clone)]
pub struct ActionLog {
    entries: VecDeque<ActionLogEntry>,
    config: ActionLogConfig,
    next_sequence: u64,
}

impl Default for ActionLog {
    fn default() -> Self {
        Self::new(ActionLogConfig::default())
    }
}

impl ActionLog {
    /// Create a new action log with configuration
    pub fn new(config: ActionLogConfig) -> Self {
        Self {
            entries: VecDeque::with_capacity(config.capacity),
            config,
            next_sequence: 0,
        }
    }

    /// Log an action (if it passes the filter)
    ///
    /// Returns the entry if it was logged, None if filtered out.
    pub fn log<A: ActionSummary>(&mut self, action: &A) -> Option<&ActionLogEntry> {
        let name = action.name();

        if !self.config.filter.should_log(name) {
            return None;
        }

        let summary = action.summary();
        let entry = ActionLogEntry::new(name, summary, self.next_sequence);
        self.next_sequence += 1;

        // Maintain capacity
        if self.entries.len() >= self.config.capacity {
            self.entries.pop_front();
        }

        self.entries.push_back(entry);
        self.entries.back()
    }

    /// Update the last entry with state_changed info (called after reducer)
    pub fn update_last_state_changed(&mut self, changed: bool) {
        if let Some(entry) = self.entries.back_mut() {
            entry.state_changed = Some(changed);
        }
    }

    /// Get all entries (oldest first)
    pub fn entries(&self) -> impl Iterator<Item = &ActionLogEntry> {
        self.entries.iter()
    }

    /// Get entries in reverse order (newest first)
    pub fn entries_rev(&self) -> impl Iterator<Item = &ActionLogEntry> {
        self.entries.iter().rev()
    }

    /// Get the most recent N entries (newest first)
    pub fn recent(&self, count: usize) -> impl Iterator<Item = &ActionLogEntry> {
        self.entries.iter().rev().take(count)
    }

    /// Number of entries currently stored
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the log is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Get configuration
    pub fn config(&self) -> &ActionLogConfig {
        &self.config
    }

    /// Get mutable configuration
    pub fn config_mut(&mut self) -> &mut ActionLogConfig {
        &mut self.config
    }
}

// ============================================================================
// Middleware
// ============================================================================

/// Middleware that logs actions with configurable pattern filtering.
///
/// Supports two modes:
/// - **Tracing only** (default): logs via `tracing::debug!()`
/// - **With storage**: also stores in ActionLog ring buffer for overlay display
///
/// # Example
///
/// ```ignore
/// use tui_dispatch_core::debug::{ActionLoggerConfig, ActionLoggerMiddleware, ActionLogConfig};
/// use tui_dispatch_core::{Store, StoreWithMiddleware};
///
/// // Tracing only
/// let middleware = ActionLoggerMiddleware::new(ActionLoggerConfig::default());
///
/// // With in-memory storage
/// let middleware = ActionLoggerMiddleware::with_log(ActionLogConfig::default());
///
/// // Access the log for display
/// if let Some(log) = middleware.log() {
///     for entry in log.recent(10) {
///         println!("{}", entry.summary);
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct ActionLoggerMiddleware {
    config: ActionLoggerConfig,
    log: Option<ActionLog>,
    /// Tracks whether the last action was logged (for state_changed updates)
    last_action_logged: bool,
    /// Whether the middleware is active (processes actions)
    /// When false, all methods become no-ops for zero overhead.
    active: bool,
}

impl ActionLoggerMiddleware {
    /// Create a new action logger middleware with tracing only (no in-memory storage)
    pub fn new(config: ActionLoggerConfig) -> Self {
        Self {
            config,
            log: None,
            last_action_logged: false,
            active: true,
        }
    }

    /// Create middleware with in-memory storage
    pub fn with_log(config: ActionLogConfig) -> Self {
        Self {
            config: config.filter.clone(),
            log: Some(ActionLog::new(config)),
            last_action_logged: false,
            active: true,
        }
    }

    /// Create with default config and in-memory storage
    pub fn with_default_log() -> Self {
        Self::with_log(ActionLogConfig::default())
    }

    /// Create with default config (excludes Tick and Render), tracing only
    pub fn default_filtering() -> Self {
        Self::new(ActionLoggerConfig::default())
    }

    /// Create with no filtering (logs all actions), tracing only
    pub fn log_all() -> Self {
        Self::new(ActionLoggerConfig::with_patterns(vec![], vec![]))
    }

    /// Set whether the middleware is active.
    ///
    /// When inactive (`false`), all methods become no-ops with zero overhead.
    /// This is useful for conditional logging based on CLI flags.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let middleware = ActionLoggerMiddleware::default_filtering()
    ///     .active(args.debug);  // Only log if --debug flag passed
    /// ```
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Check if the middleware is active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Get the action log (if storage is enabled)
    pub fn log(&self) -> Option<&ActionLog> {
        self.log.as_ref()
    }

    /// Get mutable action log
    pub fn log_mut(&mut self) -> Option<&mut ActionLog> {
        self.log.as_mut()
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

impl<A: ActionSummary> Middleware<A> for ActionLoggerMiddleware {
    fn before(&mut self, action: &A) {
        // Inactive: no-op
        if !self.active {
            return;
        }

        let name = action.name();

        // Always log to tracing if filter passes
        if self.config.should_log(name) {
            tracing::debug!(action = %name, "action");
        }

        // Log to in-memory buffer if enabled
        self.last_action_logged = false;
        if let Some(ref mut log) = self.log {
            if log.log(action).is_some() {
                self.last_action_logged = true;
            }
        }
    }

    fn after(&mut self, _action: &A, state_changed: bool) {
        // Inactive: no-op
        if !self.active {
            return;
        }

        // Only update state_changed if this action was actually logged
        if self.last_action_logged {
            if let Some(ref mut log) = self.log {
                log.update_last_state_changed(state_changed);
            }
        }
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

    // Test action for ActionLog tests
    #[derive(Clone, Debug)]
    enum TestAction {
        Tick,
        Connect,
    }

    impl crate::Action for TestAction {
        fn name(&self) -> &'static str {
            match self {
                TestAction::Tick => "Tick",
                TestAction::Connect => "Connect",
            }
        }
    }

    // Use default summary (Debug)
    impl crate::ActionSummary for TestAction {}

    #[test]
    fn test_action_log_basic() {
        let mut log = ActionLog::default();
        assert!(log.is_empty());

        log.log(&TestAction::Connect);
        assert_eq!(log.len(), 1);

        let entry = log.entries().next().unwrap();
        assert_eq!(entry.name, "Connect");
        assert_eq!(entry.sequence, 0);
    }

    #[test]
    fn test_action_log_filtering() {
        let mut log = ActionLog::default(); // Default excludes Tick

        log.log(&TestAction::Tick);
        assert!(log.is_empty()); // Tick should be filtered

        log.log(&TestAction::Connect);
        assert_eq!(log.len(), 1);
    }

    #[test]
    fn test_action_log_capacity() {
        let config = ActionLogConfig::new(
            3,
            ActionLoggerConfig::with_patterns(vec![], vec![]), // No filtering
        );
        let mut log = ActionLog::new(config);

        log.log(&TestAction::Connect);
        log.log(&TestAction::Connect);
        log.log(&TestAction::Connect);
        assert_eq!(log.len(), 3);

        log.log(&TestAction::Connect);
        assert_eq!(log.len(), 3); // Still 3, oldest was removed

        // First entry should now be sequence 1 (sequence 0 was removed)
        assert_eq!(log.entries().next().unwrap().sequence, 1);
    }

    #[test]
    fn test_action_log_state_changed() {
        let mut log = ActionLog::default();

        log.log(&TestAction::Connect);
        log.update_last_state_changed(true);

        let entry = log.entries().next().unwrap();
        assert_eq!(entry.state_changed, Some(true));
    }

    #[test]
    fn test_action_log_recent() {
        let config = ActionLogConfig::new(10, ActionLoggerConfig::with_patterns(vec![], vec![]));
        let mut log = ActionLog::new(config);

        for _ in 0..5 {
            log.log(&TestAction::Connect);
        }

        // Recent should return newest first
        let recent: Vec<_> = log.recent(3).collect();
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].sequence, 4); // Newest
        assert_eq!(recent[1].sequence, 3);
        assert_eq!(recent[2].sequence, 2);
    }

    #[test]
    fn test_action_log_entry_elapsed_display() {
        let entry = ActionLogEntry::new("Test", "Test".to_string(), 0);
        // Should show "0ms" or similar for a fresh entry
        let display = entry.elapsed_display();
        assert!(display.ends_with("ms") || display.ends_with("s"));
    }

    #[test]
    fn test_middleware_filtered_action_does_not_update_state_changed() {
        use crate::store::Middleware;

        // Default config filters out "Tick"
        let mut middleware = ActionLoggerMiddleware::with_default_log();

        // Log a Connect action first
        middleware.before(&TestAction::Connect);
        middleware.after(&TestAction::Connect, true);

        // Verify Connect was logged with state_changed = true
        let log = middleware.log().unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log.entries().next().unwrap().state_changed, Some(true));

        // Now dispatch a Tick (filtered out by default)
        middleware.before(&TestAction::Tick);
        middleware.after(&TestAction::Tick, false);

        // Log should still have only 1 entry (Tick was filtered)
        let log = middleware.log().unwrap();
        assert_eq!(log.len(), 1);

        // The Connect entry's state_changed should still be true
        // (not overwritten by the filtered Tick's state_changed=false)
        assert_eq!(log.entries().next().unwrap().state_changed, Some(true));
    }
}
