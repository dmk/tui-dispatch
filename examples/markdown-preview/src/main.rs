//! Markdown Preview - TUI markdown viewer with debug features
//!
//! Demonstrates tui-dispatch debug capabilities:
//! - F12: Freeze frame, inspect UI
//! - S: Show state overlay (AST stats)
//! - Y: Copy frame to clipboard (OSC52)
//! - Mouse click: Inspect cell styling

mod action;
mod reducer;
mod state;

use std::io::{self, Write};
use std::time::Duration;

use base64::Engine;
use clap::Parser;
use crossterm::{
    event::{
        DisableMouseCapture, EnableMouseCapture, KeyCode, KeyModifiers, MouseButton, MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tui_dispatch::{
    EventKind, RawEvent, Store,
    debug::{
        DebugAction, DebugConfig, DebugLayer, DebugSection, DebugSideEffect, DebugState,
        DebugTableBuilder, inspect_cell,
    },
    keybindings::{BindingContext, Keybindings},
    process_raw_event, spawn_event_poller,
};

use crate::action::Action;
use crate::reducer::reducer;
use crate::state::AppState;

/// Markdown Preview - TUI markdown viewer
#[derive(Parser, Debug)]
#[command(name = "mdpreview")]
#[command(about = "Preview markdown files in the terminal")]
struct Args {
    /// Markdown file to view
    #[arg(default_value = "README.md")]
    file: String,
}

/// Keybinding context for debug layer
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Context {
    Normal,
    Debug,
}

impl BindingContext for Context {
    fn name(&self) -> &'static str {
        match self {
            Context::Normal => "normal",
            Context::Debug => "debug",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "normal" => Some(Context::Normal),
            "debug" => Some(Context::Debug),
            _ => None,
        }
    }

    fn all() -> &'static [Self] {
        &[Context::Normal, Context::Debug]
    }
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

    let result = run_app(&mut terminal, args.file).await;

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
) -> io::Result<()> {
    // Action channel
    let (action_tx, mut action_rx) = mpsc::unbounded_channel::<Action>();

    // Store
    let mut store = Store::new(AppState::new(file_path), reducer);

    // Update terminal size in state
    let size = terminal.size()?;
    store.state_mut().terminal_height = size.height;

    // Debug layer with keybindings
    let mut keybindings = Keybindings::new();
    keybindings.add(
        Context::Debug,
        "debug.toggle",
        vec!["F12".into(), "Esc".into()],
    );
    keybindings.add(Context::Debug, "debug.state", vec!["s".into(), "S".into()]);
    keybindings.add(Context::Debug, "debug.copy", vec!["y".into(), "Y".into()]);
    keybindings.add(Context::Debug, "debug.mouse", vec!["i".into(), "I".into()]);
    let config = DebugConfig::new(keybindings, Context::Debug);
    let mut debug: DebugLayer<Action, Context> = DebugLayer::new(config);

    // Event poller
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<RawEvent>();
    let cancel_token = CancellationToken::new();
    let _event_handle = spawn_event_poller(
        event_tx,
        Duration::from_millis(10),
        Duration::from_millis(16),
        cancel_token.clone(),
    );

    let mut should_render = true;

    loop {
        // Render
        if should_render {
            terminal.draw(|frame| {
                let state = store.state();
                debug.render(frame, |f, area| {
                    render_app(f, area, state);
                });
            })?;
            should_render = false;
        }

        // Wait for events
        tokio::select! {
            Some(raw_event) = event_rx.recv() => {
                let event_kind = process_raw_event(raw_event);

                // Handle resize
                if let EventKind::Resize(_, height) = event_kind {
                    store.state_mut().terminal_height = height;
                    should_render = true;
                    continue;
                }

                // Handle debug mode first
                if debug.is_enabled() {
                    if let Some(action) = handle_debug_event(&event_kind, &mut debug, store.state()) {
                        match action {
                            DebugEventResult::Action(debug_action) => {
                                if let Some(effect) = debug.handle_action(debug_action) {
                                    handle_side_effect(effect);
                                }
                            }
                            DebugEventResult::Mouse(x, y) => {
                                // Cell inspection on mouse click
                                if let Some(ref snapshot) = debug.freeze().snapshot
                                    && let Some(cell) = inspect_cell(snapshot, x, y)
                                {
                                    let overlay = DebugTableBuilder::new()
                                        .section("Cell Info")
                                        .entry("position", format!("({}, {})", x, y))
                                        .entry("symbol", format!("'{}'", cell.symbol))
                                        .entry("fg", format!("{:?}", cell.fg))
                                        .entry("bg", format!("{:?}", cell.bg))
                                        .entry("modifier", format!("{:?}", cell.modifier))
                                        .cell_preview(cell)
                                        .finish_inspect("Cell Inspector");
                                    debug.freeze_mut().set_overlay(overlay);
                                }
                            }
                        }
                    }
                    // Always render after debug mode event handling (e.g., 's' sets overlay directly)
                    should_render = true;
                    continue;
                }

                // Check for F12 to enter debug mode
                if let EventKind::Key(key) = &event_kind
                    && key.code == KeyCode::F(12)
                {
                    debug.handle_action(DebugAction::Toggle);
                    should_render = true;
                    continue;
                }

                // Normal event handling
                let actions = handle_event(&event_kind, store.state());
                for action in actions {
                    let _ = action_tx.send(action);
                }
            }

            Some(action) = action_rx.recv() => {
                if matches!(action, Action::Quit) {
                    break;
                }

                let changed = store.dispatch(action);
                should_render = changed;
            }
        }
    }

    cancel_token.cancel();
    Ok(())
}

enum DebugEventResult {
    Action(DebugAction),
    Mouse(u16, u16),
}

fn handle_debug_event(
    event: &EventKind,
    debug: &mut DebugLayer<Action, Context>,
    state: &AppState,
) -> Option<DebugEventResult> {
    match event {
        EventKind::Key(key) => {
            let action = match key.code {
                KeyCode::F(12) | KeyCode::Esc => Some(DebugAction::Toggle),
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    // Show state overlay
                    debug.show_state_overlay(state);
                    return None;
                }
                KeyCode::Char('y') | KeyCode::Char('Y') => Some(DebugAction::CopyFrame),
                KeyCode::Char('i') | KeyCode::Char('I') => Some(DebugAction::ToggleMouseCapture),
                _ => None,
            };
            action.map(DebugEventResult::Action)
        }
        EventKind::Mouse(mouse) => {
            if debug.freeze().mouse_capture_enabled
                && let MouseEventKind::Down(MouseButton::Left) = mouse.kind
            {
                return Some(DebugEventResult::Mouse(mouse.column, mouse.row));
            }
            None
        }
        _ => None,
    }
}

fn handle_side_effect(effect: DebugSideEffect<Action>) {
    match effect {
        DebugSideEffect::CopyToClipboard(text) => {
            copy_to_clipboard(&text);
        }
        DebugSideEffect::ProcessQueuedActions(_actions) => {
            // Actions queued while frozen - we'd dispatch them here
        }
        _ => {}
    }
}

/// Copy text to clipboard via OSC52 escape sequence
fn copy_to_clipboard(text: &str) {
    let encoded = base64::engine::general_purpose::STANDARD.encode(text);
    print!("\x1b]52;c;{}\x07", encoded);
    io::stdout().flush().ok();
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
        _ => vec![],
    }
}

fn render_app(frame: &mut ratatui::Frame, area: Rect, state: &AppState) {
    let chunks = Layout::vertical([
        Constraint::Length(1), // Title bar
        Constraint::Min(1),    // Content
        Constraint::Length(1), // Status bar
    ])
    .split(area);

    // Title bar
    let title = Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled(
            &state.file_path,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" ", Style::default()),
    ]);
    frame.render_widget(
        Paragraph::new(title).style(Style::default().bg(Color::Rgb(30, 30, 40))),
        chunks[0],
    );

    // Content
    render_content(frame, chunks[1], state);

    // Status bar
    render_status_bar(frame, chunks[2], state);
}

fn render_content(frame: &mut ratatui::Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::LEFT | Borders::RIGHT)
        .border_style(Style::default().fg(Color::Rgb(60, 60, 70)));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Get visible lines
    let visible_height = inner.height as usize;
    let start = state.scroll_offset;
    let end = (start + visible_height).min(state.rendered_lines.len());

    let visible_lines: Vec<Line> = state.rendered_lines[start..end]
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let line_idx = start + i;
            // Highlight search matches
            if !state.search.query.is_empty() && state.search.matches.contains(&line_idx) {
                let is_current =
                    state.search.matches.get(state.search.current_match) == Some(&line_idx);
                let bg = if is_current {
                    Color::Rgb(80, 80, 40)
                } else {
                    Color::Rgb(50, 50, 30)
                };
                Line::from(
                    line.spans
                        .iter()
                        .map(|s| Span::styled(s.content.clone(), s.style.bg(bg)))
                        .collect::<Vec<_>>(),
                )
            } else {
                line.clone()
            }
        })
        .collect();

    let paragraph = Paragraph::new(visible_lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

fn render_status_bar(frame: &mut ratatui::Frame, area: Rect, state: &AppState) {
    let style = Style::default().bg(Color::Rgb(30, 30, 40));

    if state.search.active {
        // Search input mode
        let search_line = Line::from(vec![
            Span::styled(
                " /",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(&state.search.query, Style::default().fg(Color::White)),
            Span::styled(
                "_",
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::SLOW_BLINK),
            ),
        ]);
        frame.render_widget(Paragraph::new(search_line).style(style), area);
    } else {
        // Normal status
        let match_info = if !state.search.matches.is_empty() {
            format!(
                " [{}/{}]",
                state.search.current_match + 1,
                state.search.matches.len()
            )
        } else {
            String::new()
        };

        let line_info = format!(
            " {}:{} ",
            state.scroll_offset + 1,
            state.rendered_lines.len()
        );

        let help = " j/k:scroll  /:search  n/N:next/prev  r:reload  F12:debug  q:quit ";

        let status = Line::from(vec![
            Span::styled(line_info, Style::default().fg(Color::DarkGray)),
            Span::styled(match_info, Style::default().fg(Color::Yellow)),
            Span::styled(help, Style::default().fg(Color::Rgb(80, 80, 90))),
        ]);
        frame.render_widget(Paragraph::new(status).style(style), area);
    }
}
