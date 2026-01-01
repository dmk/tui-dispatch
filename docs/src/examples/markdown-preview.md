# Markdown Preview Example

A markdown file viewer showcasing the debug layer and advanced tui-dispatch features.

## Running

```bash
# View README.md
cargo run -p markdown-preview

# View a specific file
cargo run -p markdown-preview -- path/to/file.md
```

## What It Shows

### Debug Layer (One-Line Setup)

The debug layer is set up with a single line:

```rust
let mut debug: DebugLayer<Action, _> = DebugLayer::simple();
```

Press **F12** to enter debug mode. The debug layer freezes the frame and provides inspection tools:

| Key | Action |
|-----|--------|
| `s` / `S` | Show state overlay (document stats, view metrics) |
| `y` / `Y` | Copy frame to clipboard (via OSC52) |
| `i` / `I` | Enable mouse capture for cell inspection |
| `F12` / `Esc` | Exit debug mode |

When mouse capture is enabled, click any cell to inspect its styling (symbol, foreground, background, modifiers).

### DebugState Implementation

To enable state inspection, implement the `DebugState` trait. You can use the derive macro:

```rust
#[derive(DebugState)]
struct AppState {
    #[debug(section = "Document")]
    file_path: String,
    #[debug(section = "Document")]
    total_lines: usize,

    #[debug(section = "AST Statistics")]
    heading_count: usize,
    #[debug(section = "AST Statistics")]
    link_count: usize,

    #[debug(skip)]
    internal_cache: Vec<u8>,
}
```

Or implement it manually for more control:

```rust
impl DebugState for AppState {
    fn debug_sections(&self) -> Vec<DebugSection> {
        vec![
            DebugSection::new("Document")
                .entry("file", &self.file_path)
                .entry("total_lines", self.stats.total_lines.to_string()),
            DebugSection::new("AST Statistics")
                .entry("headings", self.stats.heading_count.to_string())
                .entry("links", self.stats.link_count.to_string()),
            // ...
        ]
    }
}
```

### Feature Flags (CLI)

Feature flags via command line using `#[derive(FeatureFlags)]`:

```rust
#[derive(FeatureFlags)]
pub struct Features {
    #[flag(default = false)]
    pub line_numbers: bool,

    #[flag(default = true)]
    pub wrap_lines: bool,

    #[flag(default = true)]
    pub show_stats: bool,
}
```

Enable/disable via CLI:

```bash
# Enable line numbers
cargo run -p markdown-preview -- README.md --enable line_numbers

# Disable wrapping, enable line numbers
cargo run -p markdown-preview -- README.md --disable wrap_lines --enable line_numbers

# Multiple flags (comma-separated)
cargo run -p markdown-preview -- README.md --enable line_numbers,show_stats
```

### Search Functionality

The example includes vim-style search:

1. Press `/` to start search
2. Type query, press `Enter` to confirm
3. `n` / `N` to navigate matches
4. `Esc` to cancel

Search matches are highlighted in the document with the current match emphasized.

## Keybindings

### Normal Mode

| Key | Action |
|-----|--------|
| `j` / `↓` | Scroll down |
| `k` / `↑` | Scroll up |
| `Ctrl+d` / `PageDown` | Page down |
| `Ctrl+u` / `PageUp` | Page up |
| `g` / `Home` | Jump to top |
| `G` / `End` | Jump to bottom |
| `/` | Start search |
| `n` | Next match |
| `N` | Previous match |
| `r` | Reload file |
| `F12` | Enter debug mode |
| `q` | Quit |

### Search Mode

| Key | Action |
|-----|--------|
| Any character | Add to query |
| `Backspace` | Delete character |
| `Enter` | Submit search |
| `Esc` | Cancel search |

## Key Files

| File | Purpose |
|------|---------|
| `src/main.rs` | Entry point, debug layer setup, event handling |
| `src/action.rs` | Navigation, search, file actions |
| `src/state.rs` | AppState with markdown rendering, search state |
| `src/reducer.rs` | State mutations for scrolling, search, file ops |
| `src/features.rs` | CLI feature flags (line numbers, wrapping, stats) |
