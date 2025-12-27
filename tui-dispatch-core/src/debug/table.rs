//! Debug table types and builder
//!
//! Provides a builder pattern for constructing debug information tables
//! with sections and key-value entries.

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
}

impl DebugOverlay {
    /// Get the underlying table from the overlay
    pub fn table(&self) -> &DebugTableOverlay {
        match self {
            DebugOverlay::Inspect(table) | DebugOverlay::State(table) => table,
        }
    }

    /// Get the overlay kind as a string
    pub fn kind(&self) -> &'static str {
        match self {
            DebugOverlay::Inspect(_) => "inspect",
            DebugOverlay::State(_) => "state",
        }
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

        let state = DebugOverlay::State(table);
        assert_eq!(state.kind(), "state");
    }
}
