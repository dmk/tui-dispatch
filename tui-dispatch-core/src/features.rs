//! Runtime feature flags for TUI applications
//!
//! Feature flags allow you to toggle functionality at runtime, useful for:
//! - Gradual feature rollouts
//! - A/B testing
//! - Debug-only features
//! - User preferences
//!
//! # Quick Start
//!
//! ```ignore
//! use tui_dispatch::FeatureFlags;
//!
//! #[derive(FeatureFlags)]
//! struct Features {
//!     #[flag(default = false)]
//!     new_search_ui: bool,
//!
//!     #[flag(default = true)]
//!     vim_bindings: bool,
//! }
//!
//! let mut features = Features::default();
//! assert!(!features.new_search_ui);
//! assert!(features.vim_bindings);
//!
//! features.enable("new_search_ui");
//! assert!(features.new_search_ui);
//! ```
//!
//! # Usage in Reducers
//!
//! ```ignore
//! fn reducer(state: &mut AppState, action: Action, features: &Features) -> bool {
//!     match action {
//!         Action::ShowSuggestions(s) if features.new_search_ui => {
//!             state.suggestions = s;
//!             true
//!         }
//!         Action::ShowSuggestions(_) => false, // Feature disabled
//!         // ...
//!     }
//! }
//! ```
//!
//! # Usage in Render
//!
//! ```ignore
//! if features.new_search_ui {
//!     render_new_search(frame, area, state);
//! } else {
//!     render_legacy_search(frame, area, state);
//! }
//! ```

use std::collections::HashMap;

/// Trait for feature flag containers
///
/// Implement this trait to create a type-safe feature flag system.
/// Use `#[derive(FeatureFlags)]` for automatic implementation.
///
/// # Example
///
/// ```
/// use tui_dispatch_core::FeatureFlags;
///
/// struct MyFeatures {
///     dark_mode: bool,
///     experimental: bool,
/// }
///
/// impl FeatureFlags for MyFeatures {
///     fn is_enabled(&self, name: &str) -> Option<bool> {
///         match name {
///             "dark_mode" => Some(self.dark_mode),
///             "experimental" => Some(self.experimental),
///             _ => None,
///         }
///     }
///
///     fn set(&mut self, name: &str, enabled: bool) -> bool {
///         match name {
///             "dark_mode" => { self.dark_mode = enabled; true }
///             "experimental" => { self.experimental = enabled; true }
///             _ => false,
///         }
///     }
///
///     fn all_flags() -> &'static [&'static str] {
///         &["dark_mode", "experimental"]
///     }
/// }
/// ```
pub trait FeatureFlags {
    /// Check if a feature is enabled by name
    ///
    /// Returns `None` if the feature doesn't exist.
    fn is_enabled(&self, name: &str) -> Option<bool>;

    /// Set a feature's enabled state
    ///
    /// Returns `false` if the feature doesn't exist.
    fn set(&mut self, name: &str, enabled: bool) -> bool;

    /// Get all available flag names
    fn all_flags() -> &'static [&'static str]
    where
        Self: Sized;

    /// Enable a feature by name
    ///
    /// Returns `false` if the feature doesn't exist.
    fn enable(&mut self, name: &str) -> bool {
        self.set(name, true)
    }

    /// Disable a feature by name
    ///
    /// Returns `false` if the feature doesn't exist.
    fn disable(&mut self, name: &str) -> bool {
        self.set(name, false)
    }

    /// Toggle a feature by name
    ///
    /// Returns the new state, or `None` if the feature doesn't exist.
    fn toggle(&mut self, name: &str) -> Option<bool> {
        let current = self.is_enabled(name)?;
        let new_state = !current;
        self.set(name, new_state);
        Some(new_state)
    }

    /// Get all flags as a map of name -> enabled
    fn to_map(&self) -> HashMap<String, bool>
    where
        Self: Sized,
    {
        Self::all_flags()
            .iter()
            .filter_map(|name| self.is_enabled(name).map(|v| ((*name).to_string(), v)))
            .collect()
    }

    /// Load flags from a map (e.g., from config file)
    ///
    /// Unknown flags are ignored. Returns the number of flags that were set.
    fn load_from_map(&mut self, map: &HashMap<String, bool>) -> usize {
        let mut count = 0;
        for (name, enabled) in map {
            if self.set(name, *enabled) {
                count += 1;
            }
        }
        count
    }
}

/// A dynamic feature flag store for cases where compile-time flags aren't needed
///
/// Use this when you want to define flags at runtime or load them from configuration.
///
/// # Example
///
/// ```
/// use tui_dispatch_core::DynamicFeatures;
///
/// let mut features = DynamicFeatures::new();
/// features.register("dark_mode", true);
/// features.register("experimental", false);
///
/// assert!(features.get("dark_mode"));
/// assert!(!features.get("experimental"));
///
/// features.toggle("experimental");
/// assert!(features.get("experimental"));
/// ```
#[derive(Debug, Clone, Default)]
pub struct DynamicFeatures {
    flags: HashMap<String, bool>,
}

impl DynamicFeatures {
    /// Create a new empty feature store
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new feature with a default value
    pub fn register(&mut self, name: impl Into<String>, default: bool) {
        self.flags.insert(name.into(), default);
    }

    /// Get a feature's value, returns `false` if not registered
    pub fn get(&self, name: &str) -> bool {
        self.flags.get(name).copied().unwrap_or(false)
    }

    /// Check if a feature is registered
    pub fn has(&self, name: &str) -> bool {
        self.flags.contains_key(name)
    }

    /// Get all registered flag names
    pub fn flag_names(&self) -> impl Iterator<Item = &str> {
        self.flags.keys().map(|s| s.as_str())
    }

    /// Enable a feature
    pub fn enable(&mut self, name: &str) -> bool {
        if let Some(v) = self.flags.get_mut(name) {
            *v = true;
            true
        } else {
            false
        }
    }

    /// Disable a feature
    pub fn disable(&mut self, name: &str) -> bool {
        if let Some(v) = self.flags.get_mut(name) {
            *v = false;
            true
        } else {
            false
        }
    }

    /// Toggle a feature
    pub fn toggle(&mut self, name: &str) -> Option<bool> {
        if let Some(v) = self.flags.get_mut(name) {
            *v = !*v;
            Some(*v)
        } else {
            None
        }
    }

    /// Load flags from a map, registering new ones if they don't exist
    pub fn load(&mut self, map: HashMap<String, bool>) {
        for (name, enabled) in map {
            self.flags.insert(name, enabled);
        }
    }

    /// Export all flags as a map
    pub fn export(&self) -> HashMap<String, bool> {
        self.flags.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Manual implementation for testing
    #[derive(Default)]
    struct TestFeatures {
        dark_mode: bool,
        vim_bindings: bool,
    }

    impl FeatureFlags for TestFeatures {
        fn is_enabled(&self, name: &str) -> Option<bool> {
            match name {
                "dark_mode" => Some(self.dark_mode),
                "vim_bindings" => Some(self.vim_bindings),
                _ => None,
            }
        }

        fn set(&mut self, name: &str, enabled: bool) -> bool {
            match name {
                "dark_mode" => {
                    self.dark_mode = enabled;
                    true
                }
                "vim_bindings" => {
                    self.vim_bindings = enabled;
                    true
                }
                _ => false,
            }
        }

        fn all_flags() -> &'static [&'static str] {
            &["dark_mode", "vim_bindings"]
        }
    }

    #[test]
    fn test_feature_flags_trait() {
        let mut features = TestFeatures::default();

        assert_eq!(features.is_enabled("dark_mode"), Some(false));
        assert_eq!(features.is_enabled("vim_bindings"), Some(false));
        assert_eq!(features.is_enabled("unknown"), None);

        features.enable("dark_mode");
        assert_eq!(features.is_enabled("dark_mode"), Some(true));

        features.disable("dark_mode");
        assert_eq!(features.is_enabled("dark_mode"), Some(false));

        let new_state = features.toggle("vim_bindings");
        assert_eq!(new_state, Some(true));
        assert_eq!(features.is_enabled("vim_bindings"), Some(true));
    }

    #[test]
    fn test_feature_flags_to_map() {
        let features = TestFeatures {
            dark_mode: true,
            ..Default::default()
        };

        let map = features.to_map();
        assert_eq!(map.get("dark_mode"), Some(&true));
        assert_eq!(map.get("vim_bindings"), Some(&false));
    }

    #[test]
    fn test_feature_flags_load_from_map() {
        let mut features = TestFeatures::default();
        let mut map = HashMap::new();
        map.insert("dark_mode".to_string(), true);
        map.insert("vim_bindings".to_string(), true);
        map.insert("unknown".to_string(), true); // Should be ignored

        let count = features.load_from_map(&map);
        assert_eq!(count, 2);
        assert!(features.dark_mode);
        assert!(features.vim_bindings);
    }

    #[test]
    fn test_dynamic_features() {
        let mut features = DynamicFeatures::new();
        features.register("dark_mode", true);
        features.register("experimental", false);

        assert!(features.get("dark_mode"));
        assert!(!features.get("experimental"));
        assert!(!features.get("unknown")); // Returns false for unregistered

        features.toggle("experimental");
        assert!(features.get("experimental"));

        features.disable("dark_mode");
        assert!(!features.get("dark_mode"));
    }

    #[test]
    fn test_dynamic_features_load() {
        let mut features = DynamicFeatures::new();
        let mut map = HashMap::new();
        map.insert("feature_a".to_string(), true);
        map.insert("feature_b".to_string(), false);

        features.load(map);

        assert!(features.get("feature_a"));
        assert!(!features.get("feature_b"));
        assert!(features.has("feature_a"));
        assert!(features.has("feature_b"));
    }

    #[test]
    fn test_dynamic_features_export() {
        let mut features = DynamicFeatures::new();
        features.register("a", true);
        features.register("b", false);

        let exported = features.export();
        assert_eq!(exported.len(), 2);
        assert_eq!(exported.get("a"), Some(&true));
        assert_eq!(exported.get("b"), Some(&false));
    }
}
