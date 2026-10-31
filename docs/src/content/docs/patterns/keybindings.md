---
title: Keybindings
description: Configuration-driven keybinding system with context-aware mapping
---

tui-dispatch provides a configuration-driven keybinding system that supports context-aware key mapping, user customization via config files, and shared bindings across multiple widgets.

## When to Use Keybindings

**Good fit:**
- Apps with multiple widgets that share keybindings (avoid duplication)
- User-configurable keybindings (load from TOML/JSON/YAML)
- Context-aware behavior (same key does different things in different modes)

**Overkill for:**
- Simple apps with few keys
- Apps where all key handling is in one place

## Basic Usage

```rust
use tui_dispatch::prelude::*;

// Define your contexts
#[derive(BindingContext, Clone, Copy, PartialEq, Eq, Hash)]
enum Context {
    List,
    Editor,
    Search,
}

// Build keybindings
let mut bindings = Keybindings::new();

// Global bindings work in all contexts
bindings.add_global("quit", vec!["q".into(), "ctrl+c".into()]);
bindings.add_global("help", vec!["?".into(), "f1".into()]);

// Context-specific bindings
bindings.add(Context::List, "select", vec!["enter".into()]);
bindings.add(Context::List, "delete", vec!["d".into(), "delete".into()]);

bindings.add(Context::Editor, "save", vec!["enter".into(), "ctrl+s".into()]);
bindings.add(Context::Editor, "cancel", vec!["esc".into()]);

bindings.add(Context::Search, "submit", vec!["enter".into()]);
bindings.add(Context::Search, "clear", vec!["esc".into()]);
```

## Querying Bindings

Use `get_command()` to look up what command a key triggers in a given context:

```rust
fn handle_key(key: KeyEvent, context: Context, bindings: &Keybindings<Context>) -> Option<Action> {
    let command = bindings.get_command(&key, context)?;

    match command.as_str() {
        "quit" => Some(Action::Quit),
        "select" => Some(Action::Select),
        "delete" => Some(Action::Delete),
        "save" => Some(Action::Save),
        "submit" => Some(Action::SearchSubmit),
        _ => None,
    }
}
```

The lookup checks context-specific bindings first, then falls back to global bindings.

## Validation and Conflicts

Use `validate()` after loading or merging configuration. It reports all invalid key
strings and keys assigned to multiple commands within the same scope:

```rust
if let Err(errors) = bindings.validate() {
    for invalid in errors.invalid_bindings() {
        eprintln!("{:?}/{}: invalid key {:?}", invalid.context, invalid.command, invalid.key);
    }
    for conflict in errors.conflicts() {
        eprintln!("{:?}: {:?}", conflict.key, conflict.commands().collect::<Vec<_>>());
    }
}
```

Diagnostics include context names (`None` means global) and use stable ordering.
Aliases such as `ctrl+s` and `control-s` count as the same key. Repeating a key on
one command is allowed. Context bindings may shadow global bindings, and separate
contexts may use the same key for different commands.

For checked registration, use `try_add_global()` or `try_add(context, ...)`:

```rust
bindings.try_add_global("save", ["ctrl+s", "f2"], ConflictPolicy::Reject)?;
bindings.try_add_global("search", ["ctrl-s"], ConflictPolicy::Replace)?;
// ctrl+s now runs search; f2 still runs save.
bindings.try_add(Context::Editor, "select", ["ctrl+s"], ConflictPolicy::Reject)?;
```

Both methods replace the named command's entire key list and validate the resulting
scope. `Reject` reports conflicts; `Replace` removes only colliding keys from other
commands in that scope, so the latest successful registration wins. Errors leave
all bindings unchanged. An empty key list clears a command's keys.

`add`, `add_global`, deserialization, and `merge` remain unchecked. `merge` replaces
lists by command name, so different commands can still collide. Until those
conflicts are resolved, lookup deterministically uses the lexically first command
name within the scope; invalid key strings are skipped. Call `validate()` before
using configuration to surface these problems.

## BindingContext Derive Macro

The `#[derive(BindingContext)]` macro generates the required trait implementation:

```rust
#[derive(BindingContext, Clone, Copy, PartialEq, Eq, Hash)]
enum AppContext {
    Normal,
    Insert,
    Visual,
}

// Generates:
// - AppContext::Normal.name() -> "normal"
// - AppContext::from_name("insert") -> Some(AppContext::Insert)
// - AppContext::all() -> &[Normal, Insert, Visual]
```

Variant names are automatically converted to snake_case for serialization.

## Config File Loading

Keybindings implement `Serialize` and `Deserialize`, so you can load from config files:

**config.toml:**
```toml
[global]
quit = ["q", "ctrl+c"]
help = ["?"]

[list]
select = ["enter", "space"]
delete = ["d"]

[editor]
save = ["ctrl+s"]
```

**Loading:**
```rust
use std::fs;

let config_str = fs::read_to_string("config.toml")?;
let user_bindings: Keybindings<Context> = toml::from_str(&config_str)?;
```

## Merging Defaults with User Overrides

Use `merge()` to combine default bindings with user customizations:

```rust
fn load_bindings() -> Keybindings<Context> {
    // Start with defaults
    let mut defaults = Keybindings::new();
    defaults.add_global("quit", vec!["q".into()]);
    defaults.add(Context::List, "select", vec!["enter".into()]);

    // Load user config (if exists)
    let user_bindings = match fs::read_to_string("~/.config/myapp/keys.toml") {
        Ok(content) => toml::from_str(&content).unwrap_or_default(),
        Err(_) => Keybindings::new(),
    };

    // User bindings override defaults
    Keybindings::merge(defaults, user_bindings)
}
```

## Key Format

The `parse_key_string()` function accepts these formats:

| Format | Examples |
|--------|----------|
| Single character | `"a"`, `"1"`, `"/"` |
| Special keys | `"esc"`, `"enter"`, `"tab"`, `"space"`, `"backspace"` |
| Arrow keys | `"up"`, `"down"`, `"left"`, `"right"` |
| Navigation | `"home"`, `"end"`, `"pageup"`, `"pagedown"` |
| Function keys | `"f1"` through `"f12"` |
| With modifiers | `"ctrl+c"`, `"ctrl-c"`, `"shift+tab"`, `"alt+x"`, `"ctrl+shift+s"` |

Modifiers are case insensitive: `ctrl` (or `control`), `shift`, and `alt`.
Unknown modifiers and missing keys are invalid. Literal `+` and `-` keys are
supported, including `ctrl++` and `ctrl+-`.

For displaying keys in UI (help text, status bar):

```rust
let display = format_key_for_display("ctrl+c"); // "^C"
let display = format_key_for_display("shift+tab"); // "S-Tab"
```

## Getting Bindings for Display

To show users what keys are bound to a command:

```rust
// Get first binding for a command (for hints)
if let Some(key) = bindings.get_first_keybinding("save", Context::Editor) {
    status_bar.add_hint(&key, "save");
}

// Get all bindings for a context
let editor_bindings = bindings.get_context_bindings(Context::Editor);
for (command, keys) in editor_bindings {
    println!("{}: {}", command, keys.join(", "));
}
```

## Integration Example

Here's a complete example showing keybindings in a multi-pane app:

```rust
use crossterm::event::{KeyCode, KeyEvent};
use tui_dispatch::prelude::*;

#[derive(BindingContext, Clone, Copy, PartialEq, Eq, Hash)]
enum Pane {
    Sidebar,
    Content,
    Search,
}

#[derive(Clone, Debug)]
enum Action {
    Quit,
    FocusSidebar,
    FocusContent,
    Select,
    Expand,
    Collapse,
    SearchSubmit(String),
    SearchClear,
}

struct App {
    focus: Pane,
    bindings: Keybindings<Pane>,
}

impl App {
    fn new() -> Self {
        let mut bindings = Keybindings::new();

        // Global
        bindings.add_global("quit", vec!["q".into(), "ctrl+c".into()]);
        bindings.add_global("focus_sidebar", vec!["1".into()]);
        bindings.add_global("focus_content", vec!["2".into()]);

        // Sidebar (tree navigation)
        bindings.add(Pane::Sidebar, "select", vec!["enter".into()]);
        bindings.add(Pane::Sidebar, "expand", vec!["right".into(), "l".into()]);
        bindings.add(Pane::Sidebar, "collapse", vec!["left".into(), "h".into()]);

        // Content
        bindings.add(Pane::Content, "select", vec!["enter".into()]);

        // Search
        bindings.add(Pane::Search, "submit", vec!["enter".into()]);
        bindings.add(Pane::Search, "clear", vec!["esc".into()]);

        Self {
            focus: Pane::Sidebar,
            bindings,
        }
    }

    fn handle_key(&self, key: KeyEvent, search_query: &str) -> Option<Action> {
        let cmd = self.bindings.get_command(&key, self.focus)?;

        match cmd.as_str() {
            "quit" => Some(Action::Quit),
            "focus_sidebar" => Some(Action::FocusSidebar),
            "focus_content" => Some(Action::FocusContent),
            "select" => Some(Action::Select),
            "expand" => Some(Action::Expand),
            "collapse" => Some(Action::Collapse),
            "submit" => Some(Action::SearchSubmit(search_query.to_string())),
            "clear" => Some(Action::SearchClear),
            _ => None,
        }
    }
}
```

## See Also

- [Core Concepts: Keybindings](/tui-dispatch/getting-started/core-concepts/#keybindings) - Keybindings overview
- [Debug Layer](/tui-dispatch/debugging/debug-layer/) - Uses keybindings internally for F12/S/A keys
