//! Debug table types and builder
//!
//! Provides a builder pattern for constructing debug information tables
//! with sections and key-value entries. Also includes action log overlay
//! for displaying recent actions.

use ratatui::layout::Rect;

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
    /// Action detail overlay - shows full details of a single action
    ActionDetail(ActionDetailOverlay),
    /// Components overlay - shows mounted component debug info
    Components(ComponentsOverlay),
}

/// Overlay for displaying detailed action information
#[derive(Debug, Clone)]
pub struct ActionDetailOverlay {
    /// Sequence number
    pub sequence: u64,
    /// Action name
    pub name: String,
    /// Full action parameters
    pub params: String,
    /// Elapsed time display
    pub elapsed: String,
}

/// A non-generic snapshot of a single mounted component's debug info.
#[derive(Debug, Clone)]
pub struct ComponentSnapshot {
    /// Internal mount handle id
    pub raw_id: u32,
    /// Short type name (last segment of the full path)
    pub type_name: String,
    /// Full qualified type name
    pub type_name_full: String,
    /// The component ID name it's bound to, if any
    pub bound_id: Option<String>,
    /// Last rendered area, if any
    pub last_area: Option<Rect>,
    /// Debug state key-value pairs
    pub debug_entries: Vec<(String, String)>,
}

impl ComponentSnapshot {
    /// Create from a `MountedComponentInfo` where `Id: ComponentId`.
    pub fn from_mounted_info<Id: tui_dispatch_core::ComponentId>(
        info: &tui_dispatch_components::MountedComponentInfo<Id>,
    ) -> Self {
        let type_name_full = info.type_name.to_string();
        let type_name = type_name_full
            .rsplit("::")
            .next()
            .unwrap_or(&type_name_full)
            .to_string();
        Self {
            raw_id: info.raw,
            type_name,
            type_name_full,
            bound_id: info.bound_id.map(|id| id.name().to_string()),
            last_area: info.last_area,
            debug_entries: info
                .debug_state
                .iter()
                .map(|e| (e.key.clone(), e.value.clone()))
                .collect(),
        }
    }
}

/// Overlay for displaying mounted components
#[derive(Debug, Clone)]
pub struct ComponentsOverlay {
    /// Title for the overlay
    pub title: String,
    /// Snapshots of all mounted components
    pub components: Vec<ComponentSnapshot>,
    /// Currently selected component index
    pub selected: usize,
    /// Set of expanded component indices (show debug entries)
    pub expanded: std::collections::HashSet<usize>,
}

impl ComponentsOverlay {
    /// Create from a vec of snapshots.
    pub fn new(title: impl Into<String>, components: Vec<ComponentSnapshot>) -> Self {
        Self {
            title: title.into(),
            components,
            selected: 0,
            expanded: std::collections::HashSet::new(),
        }
    }

    /// Scroll up (select previous component)
    pub fn scroll_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// Scroll down (select next component)
    pub fn scroll_down(&mut self) {
        if !self.components.is_empty() {
            self.selected = (self.selected + 1).min(self.components.len() - 1);
        }
    }

    /// Jump to the top
    pub fn scroll_to_top(&mut self) {
        self.selected = 0;
    }

    /// Jump to the bottom
    pub fn scroll_to_bottom(&mut self) {
        if !self.components.is_empty() {
            self.selected = self.components.len() - 1;
        }
    }

    /// Page up
    pub fn page_up(&mut self, page_size: usize) {
        self.selected = self.selected.saturating_sub(page_size);
    }

    /// Page down
    pub fn page_down(&mut self, page_size: usize) {
        if !self.components.is_empty() {
            self.selected = (self.selected + page_size).min(self.components.len() - 1);
        }
    }

    /// Toggle expansion of the currently selected component.
    pub fn toggle_expanded(&mut self) {
        if !self.expanded.remove(&self.selected) {
            self.expanded.insert(self.selected);
        }
    }

    /// Whether the given index is expanded.
    pub fn is_expanded(&self, index: usize) -> bool {
        self.expanded.contains(&index)
    }
}

impl DebugOverlay {
    /// Get the underlying table from the overlay (for Table/State/Inspect)
    pub fn table(&self) -> Option<&DebugTableOverlay> {
        match self {
            DebugOverlay::Inspect(table) | DebugOverlay::State(table) => Some(table),
            DebugOverlay::ActionLog(_)
            | DebugOverlay::ActionDetail(_)
            | DebugOverlay::Components(_) => None,
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

    /// Get the components overlay
    pub fn components(&self) -> Option<&ComponentsOverlay> {
        match self {
            DebugOverlay::Components(c) => Some(c),
            _ => None,
        }
    }

    /// Get the components overlay mutably
    pub fn components_mut(&mut self) -> Option<&mut ComponentsOverlay> {
        match self {
            DebugOverlay::Components(c) => Some(c),
            _ => None,
        }
    }

    /// Get the overlay kind as a string
    pub fn kind(&self) -> &'static str {
        match self {
            DebugOverlay::Inspect(_) => "inspect",
            DebugOverlay::State(_) => "state",
            DebugOverlay::ActionLog(_) => "action_log",
            DebugOverlay::ActionDetail(_) => "action_detail",
            DebugOverlay::Components(_) => "components",
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
    /// Action parameters (without the action name)
    pub params: String,
    /// Pretty action parameters for detail view
    pub params_detail: String,
    /// Elapsed time display (e.g., "2.3s")
    pub elapsed: String,
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
    /// Active action-log search query
    pub search_query: String,
    /// Matched row indices for the current search query
    pub search_matches: Vec<usize>,
    /// Current position within `search_matches`
    pub search_match_index: usize,
    /// Whether search input mode is active
    pub search_input_active: bool,
}

impl ActionLogOverlay {
    /// Create from an ActionLog reference
    pub fn from_log(log: &ActionLog, title: impl Into<String>) -> Self {
        let entries: Vec<_> = log
            .entries_rev()
            .map(|e| ActionLogDisplayEntry {
                sequence: e.sequence,
                name: e.name.to_string(),
                params: e.params.clone(),
                params_detail: e.params_pretty.clone(),
                elapsed: e.elapsed.clone(),
            })
            .collect();

        Self {
            title: title.into(),
            entries,
            selected: 0,
            scroll_offset: 0,
            search_query: String::new(),
            search_matches: Vec::new(),
            search_match_index: 0,
            search_input_active: false,
        }
    }

    /// Scroll up (select previous entry)
    pub fn scroll_up(&mut self) {
        if self.navigate_filtered(|current, _| current.saturating_sub(1)) {
            return;
        }
        if self.selected > 0 {
            self.selected -= 1;
            self.sync_search_index_from_selection();
        }
    }

    /// Scroll down (select next entry)
    pub fn scroll_down(&mut self) {
        if self.navigate_filtered(|current, max| current.saturating_add(1).min(max)) {
            return;
        }
        if self.selected + 1 < self.entries.len() {
            self.selected += 1;
            self.sync_search_index_from_selection();
        }
    }

    /// Jump to the top
    pub fn scroll_to_top(&mut self) {
        if self.navigate_filtered(|_, _| 0) {
            return;
        }
        self.selected = 0;
        self.sync_search_index_from_selection();
    }

    /// Jump to the bottom
    pub fn scroll_to_bottom(&mut self) {
        if self.navigate_filtered(|_, max| max) {
            return;
        }
        if !self.entries.is_empty() {
            self.selected = self.entries.len() - 1;
            self.sync_search_index_from_selection();
        }
    }

    /// Page up
    pub fn page_up(&mut self, page_size: usize) {
        if self.navigate_filtered(|current, _| current.saturating_sub(page_size)) {
            return;
        }
        self.selected = self.selected.saturating_sub(page_size);
        self.sync_search_index_from_selection();
    }

    /// Page down
    pub fn page_down(&mut self, page_size: usize) {
        if self.navigate_filtered(|current, max| current.saturating_add(page_size).min(max)) {
            return;
        }
        self.selected = (self.selected + page_size).min(self.entries.len().saturating_sub(1));
        self.sync_search_index_from_selection();
    }

    /// Calculate the scroll offset to keep the selected row visible.
    pub fn scroll_offset_for(&self, visible_rows: usize) -> usize {
        if visible_rows == 0 {
            return 0;
        }
        if self.selected >= visible_rows {
            self.selected - visible_rows + 1
        } else {
            0
        }
    }

    /// Get the currently selected entry
    pub fn get_selected(&self) -> Option<&ActionLogDisplayEntry> {
        self.entries.get(self.selected)
    }

    /// Create a detail overlay from the selected entry
    pub fn selected_detail(&self) -> Option<ActionDetailOverlay> {
        self.get_selected().map(|entry| ActionDetailOverlay {
            sequence: entry.sequence,
            name: entry.name.clone(),
            params: entry.params_detail.clone(),
            elapsed: entry.elapsed.clone(),
        })
    }

    /// Set the current search query and recompute matching rows.
    pub fn set_search_query(&mut self, query: impl Into<String>) {
        self.search_query = query.into();
        self.rebuild_search_matches();
    }

    /// Append a character to the active search query.
    pub fn push_search_char(&mut self, ch: char) {
        self.search_query.push(ch);
        self.rebuild_search_matches();
    }

    /// Pop the last search query character.
    pub fn pop_search_char(&mut self) -> bool {
        let popped = self.search_query.pop().is_some();
        if popped {
            self.rebuild_search_matches();
        }
        popped
    }

    /// Clear the active search query and matches.
    pub fn clear_search_query(&mut self) {
        self.search_query.clear();
        self.search_matches.clear();
        self.search_match_index = 0;
    }

    /// Move to the next match, wrapping at the end.
    pub fn search_next(&mut self) -> bool {
        if self.search_matches.is_empty() {
            return false;
        }
        self.search_match_index = (self.search_match_index + 1) % self.search_matches.len();
        self.selected = self.search_matches[self.search_match_index];
        true
    }

    /// Move to the previous match, wrapping at the beginning.
    pub fn search_prev(&mut self) -> bool {
        if self.search_matches.is_empty() {
            return false;
        }
        self.search_match_index = if self.search_match_index == 0 {
            self.search_matches.len() - 1
        } else {
            self.search_match_index - 1
        };
        self.selected = self.search_matches[self.search_match_index];
        true
    }

    /// Returns true when a search query is active.
    pub fn has_search_query(&self) -> bool {
        !self.search_query.is_empty()
    }

    // Returns `true` when filtered mode handled navigation, so callers should
    // skip unfiltered selection movement for this key event.
    fn navigate_filtered<F>(&mut self, advance: F) -> bool
    where
        F: FnOnce(usize, usize) -> usize,
    {
        if !self.has_search_query() {
            return false;
        }
        if self.search_matches.is_empty() {
            // Keep navigation scoped to filtered rows while a query is active.
            return true;
        }

        let max_match_index = self.search_matches.len() - 1;
        self.search_match_index =
            advance(self.search_match_index, max_match_index).min(max_match_index);
        self.selected = self.search_matches[self.search_match_index];
        true
    }

    /// Number of matching rows for the active query.
    pub fn search_match_count(&self) -> usize {
        self.search_matches.len()
    }

    /// Current search match position as `(index, total)` (1-based index).
    pub fn search_match_position(&self) -> Option<(usize, usize)> {
        if self.search_matches.is_empty() {
            None
        } else {
            Some((self.search_match_index + 1, self.search_matches.len()))
        }
    }

    /// Returns true if this row index is currently matched by the query.
    pub fn is_search_match(&self, row_index: usize) -> bool {
        self.search_matches.binary_search(&row_index).is_ok()
    }

    fn rebuild_search_matches(&mut self) {
        self.search_matches.clear();
        self.search_match_index = 0;

        let query = self.search_query.trim().to_ascii_lowercase();
        if query.is_empty() {
            return;
        }

        for (idx, entry) in self.entries.iter().enumerate() {
            let name = entry.name.to_ascii_lowercase();
            let params = entry.params.to_ascii_lowercase();
            let params_detail = entry.params_detail.to_ascii_lowercase();
            if name.contains(&query) || params.contains(&query) || params_detail.contains(&query) {
                // Preserves ascending order so membership checks can use binary search.
                self.search_matches.push(idx);
            }
        }

        if self.search_matches.is_empty() {
            return;
        }

        if let Some(position) = self
            .search_matches
            .iter()
            .position(|&idx| idx == self.selected)
        {
            self.search_match_index = position;
        } else {
            self.search_match_index = 0;
            self.selected = self.search_matches[0];
        }
    }

    fn sync_search_index_from_selection(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        if let Some(position) = self
            .search_matches
            .iter()
            .position(|&idx| idx == self.selected)
        {
            self.search_match_index = position;
        } else {
            self.search_match_index = self.search_match_index.min(self.search_matches.len() - 1);
            self.selected = self.search_matches[self.search_match_index];
        }
    }
}

/// Builder for constructing debug tables
///
/// # Example
///
/// ```
/// use tui_dispatch_debug::debug::{DebugTableBuilder, DebugTableRow};
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
            search_query: String::new(),
            search_matches: vec![],
            search_match_index: 0,
            search_input_active: false,
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
                    params: "".to_string(),
                    params_detail: "".to_string(),
                    elapsed: "0ms".to_string(),
                },
                ActionLogDisplayEntry {
                    sequence: 1,
                    name: "B".to_string(),
                    params: "x: 1".to_string(),
                    params_detail: "x: 1".to_string(),
                    elapsed: "1ms".to_string(),
                },
                ActionLogDisplayEntry {
                    sequence: 2,
                    name: "C".to_string(),
                    params: "y: 2".to_string(),
                    params_detail: "y: 2".to_string(),
                    elapsed: "2ms".to_string(),
                },
            ],
            selected: 0,
            scroll_offset: 0,
            search_query: String::new(),
            search_matches: vec![],
            search_match_index: 0,
            search_input_active: false,
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

    #[test]
    fn test_action_log_overlay_search_query_and_navigation() {
        let mut overlay = ActionLogOverlay {
            title: "Test".to_string(),
            entries: vec![
                ActionLogDisplayEntry {
                    sequence: 10,
                    name: "SearchStart".to_string(),
                    params: "query: \"foo\"".to_string(),
                    params_detail: "query: \"foo\"".to_string(),
                    elapsed: "0ms".to_string(),
                },
                ActionLogDisplayEntry {
                    sequence: 11,
                    name: "SearchSubmit".to_string(),
                    params: "query: \"foo\"".to_string(),
                    params_detail: "query: \"foo\"".to_string(),
                    elapsed: "1ms".to_string(),
                },
                ActionLogDisplayEntry {
                    sequence: 12,
                    name: "Connect".to_string(),
                    params: "host: \"localhost\"".to_string(),
                    params_detail: "host: \"localhost\"".to_string(),
                    elapsed: "2ms".to_string(),
                },
            ],
            selected: 0,
            scroll_offset: 0,
            search_query: String::new(),
            search_matches: vec![],
            search_match_index: 0,
            search_input_active: false,
        };

        overlay.set_search_query("search");
        assert!(overlay.has_search_query());
        assert_eq!(overlay.search_match_count(), 2);
        assert_eq!(overlay.selected, 0);
        assert_eq!(overlay.search_match_position(), Some((1, 2)));

        assert!(overlay.search_next());
        assert_eq!(overlay.selected, 1);
        assert_eq!(overlay.search_match_position(), Some((2, 2)));

        assert!(overlay.search_next());
        assert_eq!(overlay.selected, 0);
        assert_eq!(overlay.search_match_position(), Some((1, 2)));

        assert!(overlay.search_prev());
        assert_eq!(overlay.selected, 1);
        assert_eq!(overlay.search_match_position(), Some((2, 2)));
        assert_eq!(overlay.search_matches, vec![0, 1]);
    }

    #[test]
    fn test_action_log_overlay_search_edge_cases() {
        let mut overlay = ActionLogOverlay {
            title: "Test".to_string(),
            entries: vec![ActionLogDisplayEntry {
                sequence: 0,
                name: "Connect".to_string(),
                params: "host: \"example\"".to_string(),
                params_detail: "host: \"example\"".to_string(),
                elapsed: "0ms".to_string(),
            }],
            selected: 0,
            scroll_offset: 0,
            search_query: String::new(),
            search_matches: vec![],
            search_match_index: 0,
            search_input_active: false,
        };

        overlay.set_search_query("missing");
        assert_eq!(overlay.search_match_count(), 0);
        assert!(!overlay.search_next());
        assert!(!overlay.search_prev());
        assert_eq!(overlay.search_match_position(), None);

        overlay.set_search_query("connect");
        assert_eq!(overlay.search_match_count(), 1);
        assert_eq!(overlay.search_match_position(), Some((1, 1)));
        assert!(overlay.search_next());
        assert_eq!(overlay.search_match_position(), Some((1, 1)));

        assert!(overlay.pop_search_char());
        assert!(overlay.has_search_query());
        overlay.clear_search_query();
        assert!(!overlay.has_search_query());
        assert_eq!(overlay.search_match_count(), 0);
    }

    #[test]
    fn test_action_log_overlay_scroll_respects_filter() {
        let mut overlay = ActionLogOverlay {
            title: "Test".to_string(),
            entries: vec![
                ActionLogDisplayEntry {
                    sequence: 0,
                    name: "SearchStart".to_string(),
                    params: "".to_string(),
                    params_detail: "".to_string(),
                    elapsed: "0ms".to_string(),
                },
                ActionLogDisplayEntry {
                    sequence: 1,
                    name: "Connect".to_string(),
                    params: "".to_string(),
                    params_detail: "".to_string(),
                    elapsed: "1ms".to_string(),
                },
                ActionLogDisplayEntry {
                    sequence: 2,
                    name: "SearchSubmit".to_string(),
                    params: "".to_string(),
                    params_detail: "".to_string(),
                    elapsed: "2ms".to_string(),
                },
            ],
            selected: 0,
            scroll_offset: 0,
            search_query: String::new(),
            search_matches: vec![],
            search_match_index: 0,
            search_input_active: false,
        };

        overlay.set_search_query("search");
        assert_eq!(overlay.search_matches, vec![0, 2]);
        assert_eq!(overlay.selected, 0);

        overlay.scroll_down();
        assert_eq!(overlay.selected, 2);
        overlay.scroll_up();
        assert_eq!(overlay.selected, 0);
    }
}
