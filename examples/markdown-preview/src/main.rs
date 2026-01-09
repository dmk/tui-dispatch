//! Markdown Preview - TUI markdown viewer with debug features
//!
//! Demonstrates tui-dispatch capabilities:
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
    event::{DisableMouseCapture, EnableMouseCapture, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
use tui_dispatch::{
    DispatchRuntime, EventKind, EventOutcome, FeatureFlags,
    debug::{DebugLayer, DebugSection, DebugState},
};

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
        vec![
            DebugSection::new("Document")
                .entry("file", &self.file_path)
                .entry("total_lines", self.stats.total_lines.to_string()),
            DebugSection::new("AST Statistics")
                .entry("headings", self.stats.heading_count.to_string())
                .entry("links", self.stats.link_count.to_string())
                .entry("code_blocks", self.stats.code_block_count.to_string())
                .entry("list_items", self.stats.list_item_count.to_string())
                .entry("paragraphs", self.stats.paragraph_count.to_string()),
            DebugSection::new("View")
                .entry("scroll_offset", self.scroll_offset.to_string())
                .entry("max_scroll", self.max_scroll().to_string())
                .entry("terminal_height", self.terminal_height.to_string()),
            DebugSection::new("Search")
                .entry("active", self.search.active.to_string())
                .entry(
                    "query",
                    if self.search.query.is_empty() {
                        "(none)"
                    } else {
                        &self.search.query
                    },
                )
                .entry("matches", self.search.matches.len().to_string())
                .entry("current", self.search.current_match.to_string()),
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

    // Debug layer - only active when --debug flag passed
    let debug: DebugLayer<Action> = DebugLayer::simple().active(debug_enabled);

    let mut runtime = DispatchRuntime::new(state, reducer).with_debug(debug);

    runtime
        .run(terminal, render_app, map_event, |action| {
            matches!(action, Action::Quit)
        })
        .await
}

fn map_event(event: &EventKind, state: &AppState) -> EventOutcome<Action> {
    if let EventKind::Resize(_, height) = event {
        return EventOutcome::action(Action::UiTerminalResize(*height)).with_render();
    }

    EventOutcome::from(handle_event(event, state))
}

fn handle_event(event: &EventKind, state: &AppState) -> Vec<Action> {
    match event {
        EventKind::Key(key) => {
            // Search mode
            if state.search.active {
                return match key.code {
                    KeyCode::Esc => vec![Action::SearchCancel],
                    KeyCode::Enter => vec![Action::SearchSubmit],
                    KeyCode::Backspace => vec![Action::SearchBackspace],
                    KeyCode::Char(c) => vec![Action::SearchInput(c)],
                    _ => vec![],
                };
            }

            // Normal mode
            match key.code {
                // Navigation
                KeyCode::Char('j') | KeyCode::Down => vec![Action::NavScroll(1)],
                KeyCode::Char('k') | KeyCode::Up => vec![Action::NavScroll(-1)],
                KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    vec![Action::NavScrollPage(1)]
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    vec![Action::NavScrollPage(-1)]
                }
                KeyCode::PageDown => vec![Action::NavScrollPage(1)],
                KeyCode::PageUp => vec![Action::NavScrollPage(-1)],
                KeyCode::Char('g') => vec![Action::NavJumpTop], // simplified, not gg
                KeyCode::Char('G') => vec![Action::NavJumpBottom],
                KeyCode::Home => vec![Action::NavJumpTop],
                KeyCode::End => vec![Action::NavJumpBottom],

                // Search
                KeyCode::Char('/') => vec![Action::SearchStart],
                KeyCode::Char('n') => vec![Action::SearchNext],
                KeyCode::Char('N') => vec![Action::SearchPrev],

                // File
                KeyCode::Char('r') => vec![Action::FileReload],

                // Quit
                KeyCode::Char('q') => vec![Action::Quit],

                _ => vec![],
            }
        }
        EventKind::Scroll { delta, .. } => {
            vec![Action::NavScroll((delta * 3) as i16)]
        }
        EventKind::Mouse(_) => vec![],
        _ => vec![],
    }
}
