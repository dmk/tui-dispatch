//! Keybindings system with context-aware key parsing and lookup

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::Hash;
use std::sync::OnceLock;

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

/// How checked registration handles keys already assigned in the same scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictPolicy {
    /// Return diagnostics without changing any bindings.
    Reject,
    /// Remove colliding keys from other commands; the new registration wins.
    Replace,
}

/// An invalid key string and its configuration location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidKeybinding {
    /// Context name, or `None` for global bindings.
    pub context: Option<&'static str>,
    /// Command containing the invalid key.
    pub command: String,
    /// Original key string.
    pub key: String,
}

/// Commands assigned to the same normalized key within one scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeybindingConflict {
    /// Context name, or `None` for global bindings.
    pub context: Option<&'static str>,
    /// Normalized key shared by the commands.
    pub key: KeyEvent,
    commands: Vec<String>,
}

impl KeybindingConflict {
    /// Conflicting command names in lexical order (without duplicates).
    pub fn commands(&self) -> impl Iterator<Item = &str> {
        self.commands.iter().map(String::as_str)
    }
}

/// All invalid keys and conflicting assignments found during validation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeybindingErrors {
    invalid: Vec<InvalidKeybinding>,
    conflicts: Vec<KeybindingConflict>,
}

impl KeybindingErrors {
    /// Invalid key strings, preserving their original spelling.
    pub fn invalid_keys(&self) -> impl Iterator<Item = &str> {
        self.invalid.iter().map(|entry| entry.key.as_str())
    }

    /// Invalid keys with their context and command locations.
    pub fn invalid_bindings(&self) -> impl Iterator<Item = &InvalidKeybinding> {
        self.invalid.iter()
    }

    /// Conflicting assignments, grouped by scope and normalized key.
    pub fn conflicts(&self) -> impl Iterator<Item = &KeybindingConflict> {
        self.conflicts.iter()
    }

    fn into_result(self) -> Result<(), Self> {
        if self.invalid.is_empty() && self.conflicts.is_empty() {
            Ok(())
        } else {
            Err(self)
        }
    }
}

impl fmt::Display for KeybindingErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid keybindings")?;
        for invalid in &self.invalid {
            write!(
                f,
                "; [{}] {}: invalid key {:?}",
                invalid.context.unwrap_or("global"),
                invalid.command,
                invalid.key
            )?;
        }
        for conflict in &self.conflicts {
            write!(
                f,
                "; [{}] {:?}: conflicting commands {}",
                conflict.context.unwrap_or("global"),
                conflict.key,
                conflict.commands.join(", ")
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for KeybindingErrors {}

/// Keybindings configuration with context support
///
/// Generic over the context type `C` which must implement `BindingContext`.
#[derive(Debug)]
pub struct Keybindings<C: BindingContext> {
    /// Global keybindings - checked as fallback for all contexts
    global: HashMap<String, Vec<String>>,
    /// Context-specific keybindings
    contexts: HashMap<C, HashMap<String, Vec<String>>>,
    /// Cached parsed bindings for fast lookup
    compiled: OnceLock<CompiledKeybindings<C>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ParsedKey {
    code: KeyCode,
    modifiers: KeyModifiers,
}

#[derive(Debug, Clone)]
struct CompiledKeybindings<C: BindingContext> {
    global: HashMap<ParsedKey, String>,
    contexts: HashMap<C, HashMap<ParsedKey, String>>,
}

impl<C: BindingContext> Default for Keybindings<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C: BindingContext> Clone for Keybindings<C> {
    fn clone(&self) -> Self {
        Self {
            global: self.global.clone(),
            contexts: self.contexts.clone(),
            compiled: OnceLock::new(),
        }
    }
}

#[cfg(feature = "serde")]
impl<C: BindingContext> serde::Serialize for Keybindings<C> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
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

#[cfg(feature = "serde")]
impl<'de, C: BindingContext> serde::Deserialize<'de> for Keybindings<C> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Deserialize as a map of string -> bindings
        let raw: HashMap<String, HashMap<String, Vec<String>>> =
            serde::Deserialize::deserialize(deserializer)?;

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
            compiled: OnceLock::new(),
        }
    }

    /// Add a global keybinding without validation, replacing this command's keys.
    ///
    /// Call [`Self::validate`] after loading configuration, or use [`Self::try_add_global`].
    /// Unchecked conflicts resolve to the lexically first command name.
    pub fn add_global(&mut self, command: impl Into<String>, keys: Vec<String>) {
        self.global.insert(command.into(), keys);
        self.invalidate_cache();
    }

    /// Add a context-specific keybinding without validation.
    ///
    /// Like [`Self::add_global`], this replaces the command's entire key list.
    pub fn add(&mut self, context: C, command: impl Into<String>, keys: Vec<String>) {
        self.contexts
            .entry(context)
            .or_default()
            .insert(command.into(), keys);
        self.invalidate_cache();
    }

    /// Validate all scopes without modifying the configuration.
    ///
    /// Context bindings intentionally shadow globals and do not conflict with them.
    /// Aliases for the same key on one command are allowed. Diagnostics are ordered
    /// by scope (globals first, then context name), command name, and key-list order.
    pub fn validate(&self) -> Result<(), KeybindingErrors> {
        let mut errors = KeybindingErrors::default();
        validate_scope(&self.global, None, &mut errors);
        let mut contexts: Vec<_> = self.contexts.iter().collect();
        contexts.sort_by_key(|(context, _)| context.name());
        for (context, bindings) in contexts {
            validate_scope(bindings, Some(context.name()), &mut errors);
        }
        errors.into_result()
    }

    /// Replace a global command's keys and validate the resulting global scope.
    ///
    /// Errors leave all bindings and cached lookups unchanged. `Replace` removes
    /// only colliding keys from other commands, preserving their remaining keys.
    pub fn try_add_global(
        &mut self,
        command: impl Into<String>,
        keys: impl IntoIterator<Item = impl Into<String>>,
        policy: ConflictPolicy,
    ) -> Result<(), KeybindingErrors> {
        let updated = checked_registration(&self.global, None, command.into(), keys, policy)?;
        self.global = updated;
        self.invalidate_cache();
        Ok(())
    }

    /// Replace a context command's keys and validate the resulting context scope.
    ///
    /// Has the same atomicity and replacement semantics as [`Self::try_add_global`].
    /// Global bindings and other contexts are unaffected.
    pub fn try_add(
        &mut self,
        context: C,
        command: impl Into<String>,
        keys: impl IntoIterator<Item = impl Into<String>>,
        policy: ConflictPolicy,
    ) -> Result<(), KeybindingErrors> {
        let empty = HashMap::new();
        let bindings = self.contexts.get(&context).unwrap_or(&empty);
        let updated =
            checked_registration(bindings, Some(context.name()), command.into(), keys, policy)?;
        self.contexts.insert(context, updated);
        self.invalidate_cache();
        Ok(())
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
        self.get_command_ref(&key, context).map(str::to_string)
    }

    /// Get command name for a key event by reference
    ///
    /// Returns a borrowed `&str` to avoid allocations on hot paths.
    pub fn get_command_ref(&self, key: &KeyEvent, context: C) -> Option<&str> {
        let parsed = ParsedKey::from_key_event(key)?;
        let compiled = self.compiled();

        if let Some(context_bindings) = compiled.contexts.get(&context) {
            if let Some(cmd) = context_bindings.get(&parsed) {
                return Some(cmd.as_str());
            }
        }

        compiled.global.get(&parsed).map(String::as_str)
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

    /// Merge user config onto defaults, replacing keys for matching command names.
    ///
    /// This is unchecked: call [`Self::validate`] on the result to detect collisions
    /// between different commands. Unchecked collisions use lexical command order.
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

        defaults.invalidate_cache();
        defaults
    }

    fn compiled(&self) -> &CompiledKeybindings<C> {
        self.compiled
            .get_or_init(|| CompiledKeybindings::build(self))
    }

    fn invalidate_cache(&mut self) {
        self.compiled = OnceLock::new();
    }
}

impl ParsedKey {
    fn from_key_event(key: &KeyEvent) -> Option<Self> {
        Some(Self {
            code: if key.code == KeyCode::Tab && key.modifiers.contains(KeyModifiers::SHIFT) {
                KeyCode::BackTab
            } else {
                normalize_code(key.code)?
            },
            modifiers: key.modifiers,
        })
    }

    fn from_key_string(key_str: &str) -> Option<Self> {
        let key = parse_key_string(key_str)?;
        Self::from_key_event(&key)
    }
}

impl<C: BindingContext> CompiledKeybindings<C> {
    fn build(bindings: &Keybindings<C>) -> Self {
        Self {
            global: compile_scope(&bindings.global),
            contexts: bindings
                .contexts
                .iter()
                .map(|(context, bindings)| (*context, compile_scope(bindings)))
                .collect(),
        }
    }
}

fn sorted_bindings(bindings: &HashMap<String, Vec<String>>) -> Vec<(&String, &Vec<String>)> {
    let mut entries: Vec<_> = bindings.iter().collect();
    entries.sort_by_key(|(command, _)| *command);
    entries
}

fn compile_scope(bindings: &HashMap<String, Vec<String>>) -> HashMap<ParsedKey, String> {
    let mut compiled = HashMap::new();
    for (command, keys) in sorted_bindings(bindings) {
        for key in keys {
            if let Some(parsed) = ParsedKey::from_key_string(key) {
                compiled.entry(parsed).or_insert_with(|| command.clone());
            }
        }
    }
    compiled
}

fn validate_scope(
    bindings: &HashMap<String, Vec<String>>,
    context: Option<&'static str>,
    errors: &mut KeybindingErrors,
) {
    let mut assignments = Vec::<KeybindingConflict>::new();
    let mut indices = HashMap::<ParsedKey, usize>::new();
    for (command, keys) in sorted_bindings(bindings) {
        for key in keys {
            if let Some(parsed) = ParsedKey::from_key_string(key) {
                let index = *indices.entry(parsed.clone()).or_insert_with(|| {
                    assignments.push(KeybindingConflict {
                        context,
                        key: KeyEvent::new(parsed.code, parsed.modifiers),
                        commands: Vec::new(),
                    });
                    assignments.len() - 1
                });
                let commands = &mut assignments[index].commands;
                if commands.last() != Some(command) {
                    commands.push(command.clone());
                }
            } else {
                errors.invalid.push(InvalidKeybinding {
                    context,
                    command: command.clone(),
                    key: key.clone(),
                });
            }
        }
    }
    errors.conflicts.extend(
        assignments
            .into_iter()
            .filter(|entry| entry.commands.len() > 1),
    );
}

fn checked_registration(
    bindings: &HashMap<String, Vec<String>>,
    context: Option<&'static str>,
    command: String,
    keys: impl IntoIterator<Item = impl Into<String>>,
    policy: ConflictPolicy,
) -> Result<HashMap<String, Vec<String>>, KeybindingErrors> {
    let keys: Vec<String> = keys.into_iter().map(Into::into).collect();
    let mut updated = bindings.clone();
    if policy == ConflictPolicy::Replace {
        let replacing: HashSet<_> = keys
            .iter()
            .filter_map(|key| ParsedKey::from_key_string(key))
            .collect();
        for existing in updated.values_mut() {
            existing.retain(|key| {
                ParsedKey::from_key_string(key).is_none_or(|parsed| !replacing.contains(&parsed))
            });
        }
    }
    updated.insert(command, keys);
    let mut errors = KeybindingErrors::default();
    validate_scope(&updated, context, &mut errors);
    errors.into_result()?;
    Ok(updated)
}

fn normalize_code(code: KeyCode) -> Option<KeyCode> {
    match code {
        KeyCode::Char(c) => normalize_char(c).map(KeyCode::Char),
        other => Some(other),
    }
}

fn normalize_char(c: char) -> Option<char> {
    if c.is_ascii() {
        return Some(c.to_ascii_lowercase());
    }
    let mut folded = c.to_lowercase();
    let first = folded.next()?;
    if folded.next().is_some() {
        None
    } else {
        Some(first)
    }
}

// Consume modifier prefixes while leaving literal '+' and '-' keys intact.
fn split_key_parts(mut key: &str) -> (Vec<&str>, &str) {
    let mut modifiers = Vec::new();
    while key.chars().count() > 1 {
        let Some(index) = key.find(['+', '-']) else {
            break;
        };
        modifiers.push(key[..index].trim());
        key = key[index + 1..].trim();
    }
    (modifiers, key)
}

/// Parse a key string like "q", "esc", "ctrl+p", "ctrl-p", "shift+tab".
///
/// Modifier names are case insensitive: ctrl/control, shift, and alt.
/// Unknown or empty modifier names and missing keys are rejected.
pub fn parse_key_string(key_str: &str) -> Option<KeyEvent> {
    let key_str = key_str.trim().to_lowercase();

    if key_str.is_empty() {
        return None;
    }

    // Check for modifiers
    let (parts, key_part) = split_key_parts(&key_str);
    let mut modifiers = KeyModifiers::empty();
    for part in parts {
        match part {
            "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
            "shift" => modifiers |= KeyModifiers::SHIFT,
            "alt" => modifiers |= KeyModifiers::ALT,
            _ => return None,
        }
    }

    // Parse the key code
    let code = match key_part {
        "esc" | "escape" => KeyCode::Esc,
        "enter" | "return" => KeyCode::Enter,
        "tab" if modifiers.contains(KeyModifiers::SHIFT) => KeyCode::BackTab,
        "tab" => KeyCode::Tab,
        "backtab" => {
            modifiers |= KeyModifiers::SHIFT;
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
        c if c.chars().count() == 1 => {
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
    if key_str == "shift+tab" || key_str == "shift-tab" || key_str == "backtab" {
        return "Shift+Tab".to_string();
    }

    // Check for modifiers
    let (parts, key_part) = split_key_parts(&key_str);
    let mut modifiers = Vec::new();
    for part in parts {
        match part {
            "ctrl" | "control" => modifiers.push("^"),
            "shift" => modifiers.push("Shift+"),
            "alt" => modifiers.push("Alt+"),
            _ => {}
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
        c if c.chars().count() == 1 => {
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
