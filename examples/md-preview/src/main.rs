//! Markdown Preview - TUI markdown viewer with sync extensions
//!
//! Demonstrates tui-dispatch sync extensions (no EventBus):
//! - **Keybindings**: User-configurable key mappings
//! - **FeatureFlags**: Runtime feature toggles
//! - **DebugLayer**: State inspection overlay
//!
//! ## Feature Flags (CLI)
//! ```bash
//! mdpreview README.md --enable line_numbers --disable wrap_lines
//! ```
//! Available flags: `line_numbers`, `wrap_lines`, `show_stats`
//!
//! ## Debug Mode (--debug)
//! ```bash
//! mdpreview README.md --debug
//! ```
//! When enabled, press F12 to toggle debug overlay:
//! - S: Show state overlay (AST stats)
//! - Y: Copy frame to clipboard (OSC52)
//! - I: Mouse capture for cell inspection

mod action;
mod features;
mod reducer;
mod state;
mod ui;

use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
use tui_dispatch::{
    DefaultBindingContext, DispatchRuntime, EventKind, EventOutcome, FeatureFlags, Keybindings,
};
use tui_dispatch_debug::debug::{DebugLayer, DebugSection, DebugState, ron_string};

use crate::action::Action;
use crate::features::Features;
use crate::reducer::reducer;
use crate::state::AppState;
use crate::ui::render_app;

/// Markdown Preview - TUI markdown viewer
#[derive(Parser, Debug)]
#[command(name = "mdpreview")]
#[command(about = "Preview markdown files in the terminal")]
struct Args {
    /// Markdown file to view
    #[arg(default_value = "README.md")]
    file: String,

    /// Enable features (comma-separated: line_numbers,wrap_lines,show_stats)
    #[arg(long, value_delimiter = ',')]
    enable: Vec<String>,

    /// Disable features (comma-separated)
    #[arg(long, value_delimiter = ',')]
    disable: Vec<String>,

    /// Enable debug mode (F12 to toggle overlay, state inspection)
    #[arg(long)]
    debug: bool,
}

/// Implement DebugState for our AppState
impl DebugState for AppState {
    fn debug_sections(&self) -> Vec<DebugSection> {
        let query = if self.search.query.is_empty() {
            "(none)"
        } else {
            &self.search.query
        };

        vec![
            DebugSection::new("Document")
                .entry("file", ron_string(&self.file_path))
                .entry("total_lines", ron_string(&self.stats.total_lines)),
            DebugSection::new("AST Statistics")
                .entry("headings", ron_string(&self.stats.heading_count))
                .entry("links", ron_string(&self.stats.link_count))
                .entry("code_blocks", ron_string(&self.stats.code_block_count))
                .entry("list_items", ron_string(&self.stats.list_item_count))
                .entry("paragraphs", ron_string(&self.stats.paragraph_count)),
            DebugSection::new("View")
                .entry("scroll_offset", ron_string(&self.scroll_offset))
                .entry("max_scroll", ron_string(&self.max_scroll()))
                .entry("terminal_height", ron_string(&self.terminal_height)),
            DebugSection::new("Search")
                .entry("active", ron_string(&self.search.active))
                .entry("query", ron_string(&query))
                .entry("matches", ron_string(&self.search.matches.len()))
                .entry("current", ron_string(&self.search.current_match)),
        ]
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let args = Args::parse();

    // Check file exists
    if !std::path::Path::new(&args.file).exists() {
        eprintln!("Error: File '{}' not found", args.file);
        std::process::exit(1);
    }

    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Setup features from CLI args
    let mut features = Features::default();
    for name in &args.enable {
        features.enable(name);
    }
    for name in &args.disable {
        features.disable(name);
    }

    let result = run_app(&mut terminal, args.file, features, args.debug).await;

    // Cleanup
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

/// Create default keybindings - can be customized via config file
fn default_keybindings() -> Keybindings<DefaultBindingContext> {
    let mut kb = Keybindings::new();

    // Navigation
    kb.add_global("scroll_down", vec!["j".into(), "down".into()]);
    kb.add_global("scroll_up", vec!["k".into(), "up".into()]);
    kb.add_global("page_down", vec!["ctrl+d".into(), "pagedown".into()]);
    kb.add_global("page_up", vec!["ctrl+u".into(), "pageup".into()]);
    kb.add_global("jump_top", vec!["g".into(), "home".into()]);
    kb.add_global("jump_bottom", vec!["shift+g".into(), "end".into()]);

    // Search
    kb.add_global("search", vec!["/".into()]);
    kb.add_global("search_next", vec!["n".into()]);
    kb.add_global("search_prev", vec!["shift+n".into()]);

    // File
    kb.add_global("reload", vec!["r".into()]);
    kb.add_global("quit", vec!["q".into()]);

    kb
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    file_path: String,
    features: Features,
    debug_enabled: bool,
) -> io::Result<()> {
    let mut state = AppState::new(file_path, features);

    // Update terminal size in state
    let size = terminal.size()?;
    state.terminal_height = size.height;

    // Keybindings - user-configurable key mappings
    let keybindings = default_keybindings();

    // Debug layer - only active when --debug flag passed
    let debug: DebugLayer<Action> = DebugLayer::simple().active(debug_enabled);

    let mut runtime = DispatchRuntime::new(state, reducer).with_debug(debug);

    runtime
        .run(
            terminal,
            render_app,
            |event, state| map_event(event, state, &keybindings),
            |action| matches!(action, Action::Quit),
        )
        .await
}

fn map_event(
    event: &EventKind,
    state: &AppState,
    keybindings: &Keybindings<DefaultBindingContext>,
) -> EventOutcome<Action> {
    if let EventKind::Resize(_, height) = event {
        return EventOutcome::action(Action::UiTerminalResize(*height)).with_render();
    }

    EventOutcome::from(handle_event(event, state, keybindings))
}

fn handle_event(
    event: &EventKind,
    state: &AppState,
    keybindings: &Keybindings<DefaultBindingContext>,
) -> Vec<Action> {
    match event {
        EventKind::Key(key) => {
            // Search mode - direct key handling (not configurable)
            if state.search.active {
                return match key.code {
                    KeyCode::Esc => vec![Action::SearchCancel],
                    KeyCode::Enter => vec![Action::SearchSubmit],
                    KeyCode::Backspace => vec![Action::SearchBackspace],
                    KeyCode::Char(c) => vec![Action::SearchInput(c)],
                    _ => vec![],
                };
            }

            // Normal mode - use keybindings for configurable commands
            if let Some(cmd) = keybindings.get_command(*key, DefaultBindingContext) {
                return match cmd.as_str() {
                    "scroll_down" => vec![Action::NavScroll(1)],
                    "scroll_up" => vec![Action::NavScroll(-1)],
                    "page_down" => vec![Action::NavScrollPage(1)],
                    "page_up" => vec![Action::NavScrollPage(-1)],
                    "jump_top" => vec![Action::NavJumpTop],
                    "jump_bottom" => vec![Action::NavJumpBottom],
                    "search" => vec![Action::SearchStart],
                    "search_next" => vec![Action::SearchNext],
                    "search_prev" => vec![Action::SearchPrev],
                    "reload" => vec![Action::FileReload],
                    "quit" => vec![Action::Quit],
                    _ => vec![],
                };
            }

            vec![]
        }
        EventKind::Scroll { delta, .. } => {
            vec![Action::NavScroll((delta * 3) as i16)]
        }
        EventKind::Mouse(_) => vec![],
        _ => vec![],
    }
}
