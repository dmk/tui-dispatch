//! Tests for #[derive(FeatureFlags)] macro

use tui_dispatch::features::FeatureFlags;
use tui_dispatch::FeatureFlags as FeatureFlagsMacro;

#[test]
fn test_basic_derive() {
    #[derive(FeatureFlagsMacro)]
    struct Features {
        dark_mode: bool,
        vim_bindings: bool,
    }

    let features = Features::default();
    assert!(!features.dark_mode);
    assert!(!features.vim_bindings);
}

#[test]
fn test_defaults() {
    #[derive(FeatureFlagsMacro)]
    struct Features {
        #[flag(default = false)]
        new_ui: bool,

        #[flag(default = true)]
        classic_mode: bool,
    }

    let features = Features::default();
    assert!(!features.new_ui);
    assert!(features.classic_mode);
}

#[test]
fn test_is_enabled() {
    #[derive(FeatureFlagsMacro)]
    struct Features {
        #[flag(default = true)]
        enabled_feature: bool,

        #[flag(default = false)]
        disabled_feature: bool,
    }

    let features = Features::default();
    assert_eq!(features.is_enabled("enabled_feature"), Some(true));
    assert_eq!(features.is_enabled("disabled_feature"), Some(false));
    assert_eq!(features.is_enabled("unknown"), None);
}

#[test]
fn test_enable_disable() {
    #[derive(FeatureFlagsMacro)]
    struct Features {
        test_feature: bool,
    }

    let mut features = Features::default();
    assert!(!features.test_feature);

    features.enable("test_feature");
    assert!(features.test_feature);

    features.disable("test_feature");
    assert!(!features.test_feature);
}

#[test]
fn test_toggle() {
    #[derive(FeatureFlagsMacro)]
    struct Features {
        toggle_me: bool,
    }

    let mut features = Features::default();
    assert!(!features.toggle_me);

    let result = features.toggle("toggle_me");
    assert_eq!(result, Some(true));
    assert!(features.toggle_me);

    let result = features.toggle("toggle_me");
    assert_eq!(result, Some(false));
    assert!(!features.toggle_me);

    // Unknown feature returns None
    let result = features.toggle("unknown");
    assert_eq!(result, None);
}

#[test]
fn test_all_flags() {
    #[derive(FeatureFlagsMacro)]
    struct Features {
        feature_a: bool,
        feature_b: bool,
        feature_c: bool,
    }

    let flags = Features::all_flags();
    assert_eq!(flags.len(), 3);
    assert!(flags.contains(&"feature_a"));
    assert!(flags.contains(&"feature_b"));
    assert!(flags.contains(&"feature_c"));
}

#[test]
fn test_to_map() {
    #[derive(FeatureFlagsMacro)]
    struct Features {
        #[flag(default = true)]
        enabled: bool,
        #[flag(default = false)]
        disabled: bool,
    }

    let features = Features::default();
    let map = features.to_map();

    assert_eq!(map.len(), 2);
    assert_eq!(map.get("enabled"), Some(&true));
    assert_eq!(map.get("disabled"), Some(&false));
}

#[test]
fn test_load_from_map() {
    use std::collections::HashMap;

    #[derive(FeatureFlagsMacro)]
    struct Features {
        feature_a: bool,
        feature_b: bool,
    }

    let mut features = Features::default();
    assert!(!features.feature_a);
    assert!(!features.feature_b);

    let mut map = HashMap::new();
    map.insert("feature_a".to_string(), true);
    map.insert("feature_b".to_string(), true);
    map.insert("unknown".to_string(), true); // Should be ignored

    let count = features.load_from_map(&map);
    assert_eq!(count, 2);
    assert!(features.feature_a);
    assert!(features.feature_b);
}

#[test]
fn test_direct_field_access() {
    #[derive(FeatureFlagsMacro)]
    struct Features {
        #[flag(default = true)]
        some_feature: bool,
    }

    let mut features = Features::default();

    // Can access field directly
    assert!(features.some_feature);

    // Can set field directly
    features.some_feature = false;
    assert!(!features.some_feature);
    assert_eq!(features.is_enabled("some_feature"), Some(false));
}

#[test]
fn test_set_returns_false_for_unknown() {
    #[derive(FeatureFlagsMacro)]
    struct Features {
        known: bool,
    }

    let mut features = Features::default();

    assert!(features.set("known", true));
    assert!(!features.set("unknown", true));
}
