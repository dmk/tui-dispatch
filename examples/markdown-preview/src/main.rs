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
//! ## Debug Layer (F12)
//! - S: Show state overlay (AST stats)
//! - Y: Copy frame to clipboard (OSC52)
//! - I: Mouse capture for cell inspection

mod action;
mod features;
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
        DebugAction, DebugLayer, DebugSection, DebugSideEffect, DebugState, DebugTableBuilder,
        SimpleDebugContext, inspect_cell,
    },
    features::FeatureFlags,
    process_raw_event, spawn_event_poller,
};

use crate::action::Action;
use crate::features::Features;
use crate::reducer::reducer;
use crate::state::{AppState, CODE_BG};

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

    let result = run_app(&mut terminal, args.file, features).await;

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
) -> io::Result<()> {
    // Action channel
    let (action_tx, mut action_rx) = mpsc::unbounded_channel::<Action>();

    // Store
    let mut store = Store::new(AppState::new(file_path), reducer);

    // Update terminal size in state
    let size = terminal.size()?;
    store.state_mut().terminal_height = size.height;

    // Debug layer - one line setup with sensible defaults
    let mut debug: DebugLayer<Action, _> = DebugLayer::simple();

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
                    render_app(f, area, state, &features);
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
    debug: &mut DebugLayer<Action, SimpleDebugContext>,
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
        EventKind::Scroll { delta, .. } => {
            vec![Action::NavScroll((delta * 3) as i16)]
        }
        EventKind::Mouse(_) => vec![],
        _ => vec![],
    }
}

fn render_app(frame: &mut ratatui::Frame, area: Rect, state: &AppState, features: &Features) {
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
    render_content(frame, chunks[1], state, features);

    // Status bar
    render_status_bar(frame, chunks[2], state, features);
}

fn render_content(frame: &mut ratatui::Frame, area: Rect, state: &AppState, features: &Features) {
    let block = Block::default()
        .borders(Borders::LEFT | Borders::RIGHT)
        .border_style(Style::default().fg(Color::Rgb(60, 60, 70)));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Calculate gutter width for line numbers
    let gutter_width = if features.line_numbers {
        let max_line = state.rendered_lines.len();
        (max_line.to_string().len() + 1) as u16
    } else {
        0
    };

    // Split area for gutter and content
    let content_area = if features.line_numbers {
        Rect {
            x: inner.x + gutter_width,
            width: inner.width.saturating_sub(gutter_width),
            ..inner
        }
    } else {
        inner
    };

    let content_width = content_area.width as usize;

    // Get visible lines
    let visible_height = inner.height as usize;
    let start = state.scroll_offset;
    let end = (start + visible_height).min(state.rendered_lines.len());

    // Render line numbers if enabled
    if features.line_numbers {
        let gutter_area = Rect {
            width: gutter_width,
            ..inner
        };
        let line_nums: Vec<Line> = (start..end)
            .map(|i| {
                Line::from(Span::styled(
                    format!("{:>width$} ", i + 1, width = gutter_width as usize - 1),
                    Style::default().fg(Color::DarkGray),
                ))
            })
            .collect();
        frame.render_widget(Paragraph::new(line_nums), gutter_area);
    }

    // Render each line, handling code blocks specially for full-width background
    for (i, rendered) in state.rendered_lines[start..end].iter().enumerate() {
        let line_idx = start + i;
        let y = content_area.y + i as u16;

        if y >= content_area.y + content_area.height {
            break;
        }

        let line_area = Rect {
            x: content_area.x,
            y,
            width: content_area.width,
            height: 1,
        };

        // For code blocks, fill the entire line with background first
        if rendered.is_code {
            let bg_fill = " ".repeat(content_width);
            frame.render_widget(
                Paragraph::new(Line::from(bg_fill)).style(Style::default().bg(CODE_BG)),
                line_area,
            );

            // Render language label in top-right if present
            if let Some(ref lang) = rendered.lang
                && !lang.is_empty()
            {
                let label = format!(" {} ", lang);
                let label_width = label.len() as u16;
                let label_area = Rect {
                    x: line_area.x + line_area.width.saturating_sub(label_width + 1),
                    y: line_area.y,
                    width: label_width,
                    height: 1,
                };
                frame.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        label,
                        Style::default().fg(Color::Rgb(90, 90, 110)).bg(CODE_BG),
                    ))),
                    label_area,
                );
            }
        }

        // Prepare the line (with search highlighting if needed)
        let line = if !state.search.query.is_empty() && state.search.matches.contains(&line_idx) {
            let is_current =
                state.search.matches.get(state.search.current_match) == Some(&line_idx);
            let bg = if is_current {
                Color::Rgb(80, 80, 40)
            } else {
                Color::Rgb(50, 50, 30)
            };
            Line::from(
                rendered
                    .line
                    .spans
                    .iter()
                    .map(|s| Span::styled(s.content.clone(), s.style.bg(bg)))
                    .collect::<Vec<_>>(),
            )
        } else {
            rendered.line.clone()
        };

        // Render the actual content
        let mut paragraph = Paragraph::new(line);
        if features.wrap_lines && !rendered.is_code {
            paragraph = paragraph.wrap(Wrap { trim: false });
        }
        frame.render_widget(paragraph, line_area);
    }
}

fn render_status_bar(
    frame: &mut ratatui::Frame,
    area: Rect,
    state: &AppState,
    features: &Features,
) {
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

        // Stats info (when enabled)
        let stats_info = if features.show_stats {
            format!(
                " §{} ¶{} ",
                state.stats.heading_count, state.stats.paragraph_count
            )
        } else {
            String::new()
        };

        let help = " j/k:scroll  /:search  n/N:next/prev  F12:debug  q:quit ";

        let status = Line::from(vec![
            Span::styled(line_info, Style::default().fg(Color::DarkGray)),
            Span::styled(match_info, Style::default().fg(Color::Yellow)),
            Span::styled(stats_info, Style::default().fg(Color::Cyan)),
            Span::styled(help, Style::default().fg(Color::Rgb(80, 80, 90))),
        ]);
        frame.render_widget(Paragraph::new(status).style(style), area);
    }
}
