# Feature Flags

Runtime feature flags for toggling functionality at runtime, useful for:
- Gradual feature rollouts
- A/B testing
- Debug-only features
- User preferences

## Quick Start

Use `#[derive(FeatureFlags)]` for type-safe flags:

```rust
use tui_dispatch::FeatureFlags;

#[derive(FeatureFlags)]
struct Features {
    #[flag(default = false)]
    new_search_ui: bool,

    #[flag(default = true)]
    vim_bindings: bool,
}

let mut features = Features::default();
assert!(!features.new_search_ui);
assert!(features.vim_bindings);

// Toggle by name
features.enable("new_search_ui");
assert!(features.new_search_ui);
```

## Attributes

| Attribute | Description |
|-----------|-------------|
| `#[flag(default = true)]` | Set default value (defaults to false) |

## Trait Methods

The `FeatureFlags` trait provides:

```rust
// Check if enabled
features.is_enabled("vim_bindings") // -> Some(true)
features.is_enabled("unknown")      // -> None

// Modify
features.enable("new_search_ui");
features.disable("vim_bindings");
features.toggle("new_search_ui"); // -> Some(new_state)

// All flag names
Features::all_flags() // -> &["new_search_ui", "vim_bindings"]

// Convert to/from maps (for config files)
let map = features.to_map();
features.load_from_map(&config_map);
```

## Usage in Reducers

Guard actions based on feature state:

```rust
fn reducer(state: &mut AppState, action: Action, features: &Features) -> bool {
    match action {
        Action::ShowSuggestions(s) if features.new_search_ui => {
            state.suggestions = s;
            true
        }
        Action::ShowSuggestions(_) => false, // Feature disabled
        // ...
    }
}
```

## Usage in Render

Conditionally render components:

```rust
if features.new_search_ui {
    render_new_search(frame, area, state);
} else {
    render_legacy_search(frame, area, state);
}
```

## Dynamic Features

For runtime-defined flags (e.g., loaded from config):

```rust
use tui_dispatch::DynamicFeatures;

let mut features = DynamicFeatures::new();
features.register("dark_mode", true);
features.register("experimental", false);

// Use same API
assert!(features.get("dark_mode"));
features.toggle("experimental");

// Load from config
let config: HashMap<String, bool> = load_config();
features.load(config);
```

## Loading from Config Files

Export and import flags:

```rust
// Save to config
let map = features.to_map();
save_to_file(map);

// Load from config
let config: HashMap<String, bool> = load_from_file();
features.load_from_map(&config);
```
