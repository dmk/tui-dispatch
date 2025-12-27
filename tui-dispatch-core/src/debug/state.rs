//! Debug state introspection trait
//!
//! Provides a trait for state types to expose their contents for debug overlays.

use super::table::{DebugTableBuilder, DebugTableOverlay};

/// A debug entry (key-value pair)
#[derive(Debug, Clone)]
pub struct DebugEntry {
    pub key: String,
    pub value: String,
}

impl DebugEntry {
    /// Create a new entry
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

/// A debug section with a title and entries
#[derive(Debug, Clone)]
pub struct DebugSection {
    pub title: String,
    pub entries: Vec<DebugEntry>,
}

impl DebugSection {
    /// Create a new section
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            entries: Vec::new(),
        }
    }

    /// Add an entry to the section
    pub fn entry(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.entries.push(DebugEntry::new(key, value));
        self
    }

    /// Add an entry (mutable)
    pub fn push_entry(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.entries.push(DebugEntry::new(key, value));
    }
}

/// Trait for types that can provide debug state information
///
/// Implement this trait to enable the state overlay in debug mode.
///
/// # Example
///
/// ```
/// use tui_dispatch_core::debug::{DebugState, DebugSection, DebugEntry};
///
/// struct AppState {
///     host: String,
///     connected: bool,
///     item_count: usize,
/// }
///
/// impl DebugState for AppState {
///     fn debug_sections(&self) -> Vec<DebugSection> {
///         vec![
///             DebugSection::new("Connection")
///                 .entry("host", &self.host)
///                 .entry("connected", self.connected.to_string()),
///             DebugSection::new("Data")
///                 .entry("items", self.item_count.to_string()),
///         ]
///     }
/// }
/// ```
pub trait DebugState {
    /// Return state as sections with key-value pairs
    fn debug_sections(&self) -> Vec<DebugSection>;

    /// Build a debug table overlay from the state
    ///
    /// Default implementation uses `debug_sections()`.
    fn build_debug_table(&self, title: impl Into<String>) -> DebugTableOverlay {
        let mut builder = DebugTableBuilder::new();
        for section in self.debug_sections() {
            builder.push_section(&section.title);
            for entry in section.entries {
                builder.push_entry(entry.key, entry.value);
            }
        }
        builder.finish(title)
    }
}

/// Blanket implementation for types implementing Debug
///
/// Provides a basic fallback that renders the Debug output.
/// Types should implement DebugState directly for better formatting.
impl<T: std::fmt::Debug> DebugState for DebugWrapper<'_, T> {
    fn debug_sections(&self) -> Vec<DebugSection> {
        vec![DebugSection::new("Debug Output").entry("value", format!("{:#?}", self.0))]
    }
}

/// Wrapper to use Debug impl as DebugState
///
/// Use this when you want the fallback Debug-based rendering:
///
/// ```
/// use tui_dispatch_core::debug::{DebugState, DebugWrapper};
///
/// #[derive(Debug)]
/// struct MyState { x: i32 }
///
/// let state = MyState { x: 42 };
/// let sections = DebugWrapper(&state).debug_sections();
/// ```
pub struct DebugWrapper<'a, T>(pub &'a T);

/// Implementation for unit type (no state to show)
impl DebugState for () {
    fn debug_sections(&self) -> Vec<DebugSection> {
        vec![]
    }
}

/// Implementation for tuples - combine multiple state sources
impl<A: DebugState, B: DebugState> DebugState for (A, B) {
    fn debug_sections(&self) -> Vec<DebugSection> {
        let mut sections = self.0.debug_sections();
        sections.extend(self.1.debug_sections());
        sections
    }
}

impl<A: DebugState, B: DebugState, C: DebugState> DebugState for (A, B, C) {
    fn debug_sections(&self) -> Vec<DebugSection> {
        let mut sections = self.0.debug_sections();
        sections.extend(self.1.debug_sections());
        sections.extend(self.2.debug_sections());
        sections
    }
}

/// Implementation for references
impl<T: DebugState> DebugState for &T {
    fn debug_sections(&self) -> Vec<DebugSection> {
        (*self).debug_sections()
    }
}

impl<T: DebugState> DebugState for &mut T {
    fn debug_sections(&self) -> Vec<DebugSection> {
        (**self).debug_sections()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestState {
        name: String,
        count: usize,
    }

    impl DebugState for TestState {
        fn debug_sections(&self) -> Vec<DebugSection> {
            vec![DebugSection::new("Test")
                .entry("name", &self.name)
                .entry("count", self.count.to_string())]
        }
    }

    #[test]
    fn test_debug_state_basic() {
        let state = TestState {
            name: "test".to_string(),
            count: 42,
        };

        let sections = state.debug_sections();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].title, "Test");
        assert_eq!(sections[0].entries.len(), 2);
        assert_eq!(sections[0].entries[0].key, "name");
        assert_eq!(sections[0].entries[0].value, "test");
    }

    #[test]
    fn test_build_debug_table() {
        let state = TestState {
            name: "foo".to_string(),
            count: 10,
        };

        let table = state.build_debug_table("State Info");
        assert_eq!(table.title, "State Info");
        assert_eq!(table.rows.len(), 3); // 1 section + 2 entries
    }

    #[test]
    fn test_tuple_debug_state() {
        struct StateA;
        struct StateB;

        impl DebugState for StateA {
            fn debug_sections(&self) -> Vec<DebugSection> {
                vec![DebugSection::new("A").entry("from", "A")]
            }
        }

        impl DebugState for StateB {
            fn debug_sections(&self) -> Vec<DebugSection> {
                vec![DebugSection::new("B").entry("from", "B")]
            }
        }

        let combined = (StateA, StateB);
        let sections = combined.debug_sections();
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].title, "A");
        assert_eq!(sections[1].title, "B");
    }

    #[test]
    fn test_debug_wrapper() {
        #[derive(Debug)]
        struct PlainStruct {
            x: i32,
        }

        let s = PlainStruct { x: 42 };
        let sections = DebugWrapper(&s).debug_sections();
        assert_eq!(sections.len(), 1);
        assert!(sections[0].entries[0].value.contains("42"));
    }
}
