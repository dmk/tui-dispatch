//! Debug table types and builder
//!
//! Provides a builder pattern for constructing debug information tables
//! with sections and key-value entries. Also includes action log overlay
//! for displaying recent actions.

use super::action_logger::ActionLog;
use super::cell::CellPreview;

/// A row in a debug table - either a section header or a key-value entry
#[derive(Debug, Clone)]
pub enum DebugTableRow {
    /// Section header (e.g., "Connection", "Keys", "UI")
    Section(String),
    /// Key-value entry (e.g., "host" -> "localhost")
    Entry { key: String, value: String },
}

/// A debug table overlay with title, rows, and optional cell preview
#[derive(Debug, Clone)]
pub struct DebugTableOverlay {
    /// Title displayed at the top of the overlay
    pub title: String,
    /// Table rows (sections and entries)
    pub rows: Vec<DebugTableRow>,
    /// Optional cell preview for inspect overlays
    pub cell_preview: Option<CellPreview>,
}

impl DebugTableOverlay {
    /// Create a new overlay with the given title and rows
    pub fn new(title: impl Into<String>, rows: Vec<DebugTableRow>) -> Self {
        Self {
            title: title.into(),
            rows,
            cell_preview: None,
        }
    }

    /// Create a new overlay with cell preview
    pub fn with_cell_preview(
        title: impl Into<String>,
        rows: Vec<DebugTableRow>,
        preview: CellPreview,
    ) -> Self {
        Self {
            title: title.into(),
            rows,
            cell_preview: Some(preview),
        }
    }
}

/// Type of debug overlay
#[derive(Debug, Clone)]
pub enum DebugOverlay {
    /// Inspect overlay - shows info about a specific position/cell
    Inspect(DebugTableOverlay),
    /// State overlay - shows full application state
    State(DebugTableOverlay),
    /// Action log overlay - shows recent actions with timestamps
    ActionLog(ActionLogOverlay),
}

impl DebugOverlay {
    /// Get the underlying table from the overlay (for Table/State/Inspect)
    pub fn table(&self) -> Option<&DebugTableOverlay> {
        match self {
            DebugOverlay::Inspect(table) | DebugOverlay::State(table) => Some(table),
            DebugOverlay::ActionLog(_) => None,
        }
    }

    /// Get the action log overlay
    pub fn action_log(&self) -> Option<&ActionLogOverlay> {
        match self {
            DebugOverlay::ActionLog(log) => Some(log),
            _ => None,
        }
    }

    /// Get the action log overlay mutably
    pub fn action_log_mut(&mut self) -> Option<&mut ActionLogOverlay> {
        match self {
            DebugOverlay::ActionLog(log) => Some(log),
            _ => None,
        }
    }

    /// Get the overlay kind as a string
    pub fn kind(&self) -> &'static str {
        match self {
            DebugOverlay::Inspect(_) => "inspect",
            DebugOverlay::State(_) => "state",
            DebugOverlay::ActionLog(_) => "action_log",
        }
    }
}

// ============================================================================
// Action Log Overlay
// ============================================================================

/// A display-ready action log entry
#[derive(Debug, Clone)]
pub struct ActionLogDisplayEntry {
    /// Sequence number
    pub sequence: u64,
    /// Action name
    pub name: String,
    /// Summary text
    pub summary: String,
    /// Elapsed time display (e.g., "2.3s")
    pub elapsed: String,
    /// Whether state changed (if known)
    pub state_changed: Option<bool>,
}

/// Overlay for displaying the action log
#[derive(Debug, Clone)]
pub struct ActionLogOverlay {
    /// Title for the overlay
    pub title: String,
    /// Action entries to display
    pub entries: Vec<ActionLogDisplayEntry>,
    /// Currently selected entry index (for scrolling)
    pub selected: usize,
    /// Scroll offset for visible window
    pub scroll_offset: usize,
}

impl ActionLogOverlay {
    /// Create from an ActionLog reference
    pub fn from_log(log: &ActionLog, title: impl Into<String>) -> Self {
        let entries: Vec<_> = log
            .entries_rev()
            .map(|e| ActionLogDisplayEntry {
                sequence: e.sequence,
                name: e.name.to_string(),
                summary: e.summary.clone(),
                elapsed: e.elapsed_display(),
                state_changed: e.state_changed,
            })
            .collect();

        Self {
            title: title.into(),
            entries,
            selected: 0,
            scroll_offset: 0,
        }
    }

    /// Scroll up (select previous entry)
    pub fn scroll_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Scroll down (select next entry)
    pub fn scroll_down(&mut self) {
        if self.selected + 1 < self.entries.len() {
            self.selected += 1;
        }
    }

    /// Jump to the top
    pub fn scroll_to_top(&mut self) {
        self.selected = 0;
    }

    /// Jump to the bottom
    pub fn scroll_to_bottom(&mut self) {
        if !self.entries.is_empty() {
            self.selected = self.entries.len() - 1;
        }
    }

    /// Page up
    pub fn page_up(&mut self, page_size: usize) {
        self.selected = self.selected.saturating_sub(page_size);
    }

    /// Page down
    pub fn page_down(&mut self, page_size: usize) {
        self.selected = (self.selected + page_size).min(self.entries.len().saturating_sub(1));
    }
}

/// Builder for constructing debug tables
///
/// # Example
///
/// ```
/// use tui_dispatch_core::debug::{DebugTableBuilder, DebugTableRow};
///
/// let table = DebugTableBuilder::new()
///     .section("Connection")
///     .entry("host", "localhost")
///     .entry("port", "6379")
///     .section("Status")
///     .entry("connected", "true")
///     .finish("Connection Info");
///
/// assert_eq!(table.title, "Connection Info");
/// assert_eq!(table.rows.len(), 5);
/// ```
#[derive(Debug, Default)]
pub struct DebugTableBuilder {
    rows: Vec<DebugTableRow>,
    cell_preview: Option<CellPreview>,
}

impl DebugTableBuilder {
    /// Create a new empty builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a section header
    pub fn section(mut self, title: impl Into<String>) -> Self {
        self.rows.push(DebugTableRow::Section(title.into()));
        self
    }

    /// Add a key-value entry
    pub fn entry(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.rows.push(DebugTableRow::Entry {
            key: key.into(),
            value: value.into(),
        });
        self
    }

    /// Add a section header (mutable reference version)
    pub fn push_section(&mut self, title: impl Into<String>) {
        self.rows.push(DebugTableRow::Section(title.into()));
    }

    /// Add a key-value entry (mutable reference version)
    pub fn push_entry(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.rows.push(DebugTableRow::Entry {
            key: key.into(),
            value: value.into(),
        });
    }

    /// Set the cell preview for inspect overlays
    pub fn cell_preview(mut self, preview: CellPreview) -> Self {
        self.cell_preview = Some(preview);
        self
    }

    /// Set the cell preview (mutable reference version)
    pub fn set_cell_preview(&mut self, preview: CellPreview) {
        self.cell_preview = Some(preview);
    }

    /// Build the final table overlay with the given title
    pub fn finish(self, title: impl Into<String>) -> DebugTableOverlay {
        DebugTableOverlay {
            title: title.into(),
            rows: self.rows,
            cell_preview: self.cell_preview,
        }
    }

    /// Build as an inspect overlay
    pub fn finish_inspect(self, title: impl Into<String>) -> DebugOverlay {
        DebugOverlay::Inspect(self.finish(title))
    }

    /// Build as a state overlay
    pub fn finish_state(self, title: impl Into<String>) -> DebugOverlay {
        DebugOverlay::State(self.finish(title))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_basic() {
        let table = DebugTableBuilder::new()
            .section("Test")
            .entry("key1", "value1")
            .entry("key2", "value2")
            .finish("Test Table");

        assert_eq!(table.title, "Test Table");
        assert_eq!(table.rows.len(), 3);
        assert!(table.cell_preview.is_none());
    }

    #[test]
    fn test_builder_multiple_sections() {
        let table = DebugTableBuilder::new()
            .section("Section 1")
            .entry("a", "1")
            .section("Section 2")
            .entry("b", "2")
            .finish("Multi-Section");

        assert_eq!(table.rows.len(), 4);

        match &table.rows[0] {
            DebugTableRow::Section(s) => assert_eq!(s, "Section 1"),
            _ => panic!("Expected section"),
        }
        match &table.rows[2] {
            DebugTableRow::Section(s) => assert_eq!(s, "Section 2"),
            _ => panic!("Expected section"),
        }
    }

    #[test]
    fn test_overlay_kinds() {
        let table = DebugTableBuilder::new().finish("Test");

        let inspect = DebugOverlay::Inspect(table.clone());
        assert_eq!(inspect.kind(), "inspect");
        assert!(inspect.table().is_some());
        assert!(inspect.action_log().is_none());

        let state = DebugOverlay::State(table);
        assert_eq!(state.kind(), "state");

        let action_log = ActionLogOverlay {
            title: "Test".to_string(),
            entries: vec![],
            selected: 0,
            scroll_offset: 0,
        };
        let log_overlay = DebugOverlay::ActionLog(action_log);
        assert_eq!(log_overlay.kind(), "action_log");
        assert!(log_overlay.table().is_none());
        assert!(log_overlay.action_log().is_some());
    }

    #[test]
    fn test_action_log_overlay_scrolling() {
        let mut overlay = ActionLogOverlay {
            title: "Test".to_string(),
            entries: vec![
                ActionLogDisplayEntry {
                    sequence: 0,
                    name: "A".to_string(),
                    summary: "A".to_string(),
                    elapsed: "0ms".to_string(),
                    state_changed: None,
                },
                ActionLogDisplayEntry {
                    sequence: 1,
                    name: "B".to_string(),
                    summary: "B".to_string(),
                    elapsed: "1ms".to_string(),
                    state_changed: Some(true),
                },
                ActionLogDisplayEntry {
                    sequence: 2,
                    name: "C".to_string(),
                    summary: "C".to_string(),
                    elapsed: "2ms".to_string(),
                    state_changed: Some(false),
                },
            ],
            selected: 0,
            scroll_offset: 0,
        };

        assert_eq!(overlay.selected, 0);

        overlay.scroll_down();
        assert_eq!(overlay.selected, 1);

        overlay.scroll_down();
        assert_eq!(overlay.selected, 2);

        overlay.scroll_down(); // Should not go past end
        assert_eq!(overlay.selected, 2);

        overlay.scroll_up();
        assert_eq!(overlay.selected, 1);

        overlay.scroll_to_top();
        assert_eq!(overlay.selected, 0);

        overlay.scroll_to_bottom();
        assert_eq!(overlay.selected, 2);
    }
}
