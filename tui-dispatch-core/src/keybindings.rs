//! Keybindings system with context-aware key parsing and lookup

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::hash::Hash;

/// Trait for user-defined keybinding contexts
///
/// Implement this trait for your own context enum, or use `#[derive(BindingContext)]`
/// from `tui-dispatch-macros` to auto-generate the implementation.
///
/// # Example
/// ```ignore
/// #[derive(BindingContext, Clone, Copy, PartialEq, Eq, Hash)]
/// pub enum MyContext {
///     Default,
///     Search,
///     Modal,
/// }
/// ```
pub trait BindingContext: Clone + Copy + Eq + Hash {
    /// Get the context name as a string (for config file lookup)
    fn name(&self) -> &'static str;

    /// Parse a context from its name
    fn from_name(name: &str) -> Option<Self>;

    /// Get all possible context values (for iteration/config loading)
    fn all() -> &'static [Self];
}

/// Keybindings configuration with context support
///
/// Generic over the context type `C` which must implement `BindingContext`.
#[derive(Debug, Clone)]
pub struct Keybindings<C: BindingContext> {
    /// Global keybindings - checked as fallback for all contexts
    global: HashMap<String, Vec<String>>,
    /// Context-specific keybindings
    contexts: HashMap<C, HashMap<String, Vec<String>>>,
}

impl<C: BindingContext> Default for Keybindings<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C: BindingContext> Serialize for Keybindings<C> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeMap;

        // Count total entries: global + all contexts
        let mut map = serializer.serialize_map(Some(1 + self.contexts.len()))?;

        // Serialize global bindings
        map.serialize_entry("global", &self.global)?;

        // Serialize context-specific bindings using context names
        for (context, bindings) in &self.contexts {
            map.serialize_entry(context.name(), bindings)?;
        }

        map.end()
    }
}

impl<'de, C: BindingContext> Deserialize<'de> for Keybindings<C> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Deserialize as a map of string -> bindings
        let raw: HashMap<String, HashMap<String, Vec<String>>> =
            HashMap::deserialize(deserializer)?;

        let mut keybindings = Keybindings::new();

        for (context_name, bindings) in raw {
            if context_name == "global" {
                keybindings.global = bindings;
            } else if let Some(context) = C::from_name(&context_name) {
                keybindings.contexts.insert(context, bindings);
            }
            // Silently ignore unknown contexts (allows forward compatibility)
        }

        Ok(keybindings)
    }
}

impl<C: BindingContext> Keybindings<C> {
    /// Create a new empty keybindings configuration
    pub fn new() -> Self {
        Self {
            global: HashMap::new(),
            contexts: HashMap::new(),
        }
    }

    /// Add a global keybinding
    pub fn add_global(&mut self, command: impl Into<String>, keys: Vec<String>) {
        self.global.insert(command.into(), keys);
    }

    /// Add a context-specific keybinding
    pub fn add(&mut self, context: C, command: impl Into<String>, keys: Vec<String>) {
        self.contexts
            .entry(context)
            .or_default()
            .insert(command.into(), keys);
    }

    /// Get bindings for a specific context
    pub fn get_context_bindings(&self, context: C) -> Option<&HashMap<String, Vec<String>>> {
        self.contexts.get(&context)
    }

    /// Get global bindings
    pub fn global_bindings(&self) -> &HashMap<String, Vec<String>> {
        &self.global
    }

    /// Get command name for a key event in the given context
    ///
    /// First checks context-specific bindings, then falls back to global
    pub fn get_command(&self, key: KeyEvent, context: C) -> Option<String> {
        // First try context-specific bindings
        if let Some(context_bindings) = self.contexts.get(&context) {
            if let Some(cmd) = self.match_key_in_bindings(key, context_bindings) {
                return Some(cmd);
            }
        }

        // Fall back to global bindings
        self.match_key_in_bindings(key, &self.global)
    }

    /// Helper to match a key against a set of bindings
    fn match_key_in_bindings(
        &self,
        key: KeyEvent,
        bindings: &HashMap<String, Vec<String>>,
    ) -> Option<String> {
        for (command, keys) in bindings {
            for key_str in keys {
                if let Some(parsed_key) = parse_key_string(key_str) {
                    // Compare code and modifiers (ignore kind and state)
                    // For character keys, compare case-insensitively
                    let codes_match = match (&parsed_key.code, &key.code) {
                        (KeyCode::Char(c1), KeyCode::Char(c2)) => {
                            c1.to_lowercase().to_string() == c2.to_lowercase().to_string()
                        }
                        _ => parsed_key.code == key.code,
                    };

                    if codes_match && parsed_key.modifiers == key.modifiers {
                        return Some(command.clone());
                    }
                }
            }
        }
        None
    }

    /// Get the first keybinding string for a command in the given context
    ///
    /// First checks context-specific bindings, then falls back to global
    pub fn get_first_keybinding(&self, command: &str, context: C) -> Option<String> {
        if let Some(context_bindings) = self.contexts.get(&context) {
            if let Some(keys) = context_bindings.get(command) {
                if let Some(first) = keys.first() {
                    return Some(first.clone());
                }
            }
        }

        self.global
            .get(command)
            .and_then(|keys| keys.first().cloned())
    }

    /// Merge user config onto defaults - user config overrides defaults
    pub fn merge(mut defaults: Self, user: Self) -> Self {
        // Merge global
        for (key, value) in user.global {
            defaults.global.insert(key, value);
        }

        // Merge contexts
        for (context, bindings) in user.contexts {
            let entry = defaults.contexts.entry(context).or_default();
            for (key, value) in bindings {
                entry.insert(key, value);
            }
        }

        defaults
    }
}

/// Parse a key string like "q", "esc", "ctrl+p", "shift+tab" into a KeyEvent
pub fn parse_key_string(key_str: &str) -> Option<KeyEvent> {
    let key_str = key_str.trim().to_lowercase();

    if key_str.is_empty() {
        return None;
    }

    // Special case: shift+tab should be BackTab
    if key_str == "shift+tab" || key_str == "backtab" {
        return Some(KeyEvent {
            code: KeyCode::BackTab,
            modifiers: KeyModifiers::SHIFT,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::empty(),
        });
    }

    // Check for modifiers
    let parts: Vec<&str> = key_str.split('+').collect();
    let mut modifiers = KeyModifiers::empty();
    let key_part = parts.last()?.trim();

    if parts.len() > 1 {
        for part in &parts[..parts.len() - 1] {
            match part.trim() {
                "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
                "shift" => modifiers |= KeyModifiers::SHIFT,
                "alt" => modifiers |= KeyModifiers::ALT,
                _ => {}
            }
        }
    }

    // Parse the key code
    let code = match key_part {
        "esc" | "escape" => KeyCode::Esc,
        "enter" | "return" => KeyCode::Enter,
        "tab" => KeyCode::Tab,
        "backtab" => {
            if modifiers.is_empty() {
                modifiers |= KeyModifiers::SHIFT;
            }
            KeyCode::BackTab
        }
        "backspace" => KeyCode::Backspace,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "delete" => KeyCode::Delete,
        "insert" => KeyCode::Insert,
        "f1" => KeyCode::F(1),
        "f2" => KeyCode::F(2),
        "f3" => KeyCode::F(3),
        "f4" => KeyCode::F(4),
        "f5" => KeyCode::F(5),
        "f6" => KeyCode::F(6),
        "f7" => KeyCode::F(7),
        "f8" => KeyCode::F(8),
        "f9" => KeyCode::F(9),
        "f10" => KeyCode::F(10),
        "f11" => KeyCode::F(11),
        "f12" => KeyCode::F(12),
        "space" => KeyCode::Char(' '),
        // Single character
        c if c.len() == 1 => {
            let ch = c.chars().next()?;
            KeyCode::Char(ch)
        }
        _ => return None,
    };

    Some(KeyEvent {
        code,
        modifiers,
        kind: crossterm::event::KeyEventKind::Press,
        state: crossterm::event::KeyEventState::empty(),
    })
}

/// Format a key string for display (e.g., "ctrl+p" -> "^P", "q" -> "q", "tab" -> "Tab")
pub fn format_key_for_display(key_str: &str) -> String {
    let key_str = key_str.trim().to_lowercase();

    // Handle special cases first
    if key_str == "shift+tab" || key_str == "backtab" {
        return "Shift+Tab".to_string();
    }

    // Check for modifiers
    let parts: Vec<&str> = key_str.split('+').collect();
    let mut modifiers = Vec::new();
    let key_part = parts.last().copied().unwrap_or(key_str.as_str());

    if parts.len() > 1 {
        for part in &parts[..parts.len() - 1] {
            match part.trim() {
                "ctrl" | "control" => modifiers.push("^"),
                "shift" => modifiers.push("Shift+"),
                "alt" => modifiers.push("Alt+"),
                _ => {}
            }
        }
    }

    // Format the key part
    let key_display = match key_part {
        "esc" | "escape" => "Esc".to_string(),
        "enter" | "return" => "Enter".to_string(),
        "tab" => "Tab".to_string(),
        "backspace" => "Backspace".to_string(),
        "up" => "Up".to_string(),
        "down" => "Down".to_string(),
        "left" => "Left".to_string(),
        "right" => "Right".to_string(),
        "home" => "Home".to_string(),
        "end" => "End".to_string(),
        "pageup" => "PgUp".to_string(),
        "pagedown" => "PgDn".to_string(),
        "delete" => "Del".to_string(),
        "insert" => "Ins".to_string(),
        "space" => "Space".to_string(),
        "f1" => "F1".to_string(),
        "f2" => "F2".to_string(),
        "f3" => "F3".to_string(),
        "f4" => "F4".to_string(),
        "f5" => "F5".to_string(),
        "f6" => "F6".to_string(),
        "f7" => "F7".to_string(),
        "f8" => "F8".to_string(),
        "f9" => "F9".to_string(),
        "f10" => "F10".to_string(),
        "f11" => "F11".to_string(),
        "f12" => "F12".to_string(),
        // Single character - capitalize for display
        c if c.len() == 1 => {
            let ch = c.chars().next().unwrap();
            // Keep special characters as-is, capitalize letters
            if ch.is_alphabetic() {
                ch.to_uppercase().collect::<String>()
            } else {
                ch.to_string()
            }
        }
        _ => key_part.to_string(),
    };

    // Combine modifiers with key
    if modifiers.is_empty() {
        key_display
    } else {
        format!("{}{}", modifiers.join(""), key_display)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers};

    // Test context for unit tests
    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    enum TestContext {
        Default,
        Search,
    }

    impl BindingContext for TestContext {
        fn name(&self) -> &'static str {
            match self {
                TestContext::Default => "default",
                TestContext::Search => "search",
            }
        }

        fn from_name(name: &str) -> Option<Self> {
            match name {
                "default" => Some(TestContext::Default),
                "search" => Some(TestContext::Search),
                _ => None,
            }
        }

        fn all() -> &'static [Self] {
            &[TestContext::Default, TestContext::Search]
        }
    }

    #[test]
    fn test_parse_simple_key() {
        let result = parse_key_string("q").unwrap();
        assert_eq!(result.code, KeyCode::Char('q'));
        assert_eq!(result.modifiers, KeyModifiers::empty());
    }

    #[test]
    fn test_parse_esc() {
        let result = parse_key_string("esc").unwrap();
        assert_eq!(result.code, KeyCode::Esc);
    }

    #[test]
    fn test_parse_ctrl_key() {
        let result = parse_key_string("ctrl+p").unwrap();
        assert_eq!(result.code, KeyCode::Char('p'));
        assert!(result.modifiers.contains(KeyModifiers::CONTROL));
    }

    #[test]
    fn test_parse_shift_tab() {
        let result = parse_key_string("shift+tab").unwrap();
        assert_eq!(result.code, KeyCode::BackTab);
        assert!(result.modifiers.contains(KeyModifiers::SHIFT));
    }

    #[test]
    fn test_parse_backtab() {
        let result = parse_key_string("backtab").unwrap();
        assert_eq!(result.code, KeyCode::BackTab);
        assert!(result.modifiers.contains(KeyModifiers::SHIFT));
    }

    #[test]
    fn test_parse_arrow_keys() {
        let result = parse_key_string("up").unwrap();
        assert_eq!(result.code, KeyCode::Up);

        let result = parse_key_string("down").unwrap();
        assert_eq!(result.code, KeyCode::Down);
    }

    #[test]
    fn test_get_command() {
        let mut bindings: Keybindings<TestContext> = Keybindings::new();
        bindings.add_global("quit", vec!["q".to_string()]);
        bindings.add(TestContext::Search, "clear", vec!["esc".to_string()]);

        let key_q = KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::empty(),
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::empty(),
        };

        // Global should work in any context
        assert_eq!(
            bindings.get_command(key_q, TestContext::Default),
            Some("quit".to_string())
        );
        assert_eq!(
            bindings.get_command(key_q, TestContext::Search),
            Some("quit".to_string())
        );

        // Context-specific
        let key_esc = KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::empty(),
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::empty(),
        };

        assert_eq!(
            bindings.get_command(key_esc, TestContext::Search),
            Some("clear".to_string())
        );
        assert_eq!(bindings.get_command(key_esc, TestContext::Default), None);
    }

    #[test]
    fn test_merge() {
        let mut defaults: Keybindings<TestContext> = Keybindings::new();
        defaults.add_global("quit", vec!["q".to_string()]);
        defaults.add_global("help", vec!["?".to_string()]);

        let mut user: Keybindings<TestContext> = Keybindings::new();
        user.add_global("quit", vec!["x".to_string()]); // Override

        let merged = Keybindings::merge(defaults, user);

        // User override should be present
        assert_eq!(
            merged.global_bindings().get("quit"),
            Some(&vec!["x".to_string()])
        );

        // Default should still be there
        assert_eq!(
            merged.global_bindings().get("help"),
            Some(&vec!["?".to_string()])
        );
    }

    #[test]
    fn test_format_key_for_display() {
        assert_eq!(format_key_for_display("q"), "Q");
        assert_eq!(format_key_for_display("ctrl+p"), "^P");
        assert_eq!(format_key_for_display("esc"), "Esc");
        assert_eq!(format_key_for_display("shift+tab"), "Shift+Tab");
    }
}
