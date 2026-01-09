//! Counter - Minimal tui-dispatch example
//!
//! This example demonstrates the core pattern in ~80 lines:
//! - State: What the app knows
//! - Actions: What can happen
//! - Reducer: How state changes
//! - Store: Where state lives
//! - Main loop: Event -> Action -> Dispatch -> Render
//!
//! Keys: j/Down = decrement, k/Up = increment, q = quit

use std::io;

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};
use tui_dispatch::{debug::DebugLayer, Action, DispatchRuntime, EventKind, RenderContext};

// ============================================================================
// State - What the app knows
// ============================================================================

#[derive(Default, tui_dispatch::DebugState)]
struct AppState {
    count: i32,
}

// ============================================================================
// Actions - What can happen
// ============================================================================

#[derive(Clone, Debug, Action)]
#[action(infer_categories)]
enum AppAction {
    CountIncrement,
    CountDecrement,
    Quit,
}

// ============================================================================
// Reducer - How state changes (pure function, returns true if changed)
// ============================================================================

fn reducer(state: &mut AppState, action: AppAction) -> bool {
    match action {
        AppAction::CountIncrement => {
            state.count += 1;
            true
        }
        AppAction::CountDecrement => {
            state.count -= 1;
            true
        }
        AppAction::Quit => false, // handled in main loop
    }
}

// ============================================================================
// Renderer - ratatui stuff
// ============================================================================

fn render_app(frame: &mut Frame, area: Rect, state: &AppState, _ctx: RenderContext) {
    // Center the counter vertically and horizontally
    let [_, center, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(5),
        Constraint::Fill(1),
    ])
    .areas(area);

    let [_, center, _] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(30),
        Constraint::Fill(1),
    ])
    .flex(Flex::Center)
    .areas(center);

    let block = Block::default()
        .title(" Counter ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let text = format!("{}", state.count);
    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center)
        .block(block);

    frame.render_widget(paragraph, center);

    // Help text at bottom
    let [_, help_area] = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);
    let help = Paragraph::new("k/Up: +1  j/Down: -1  q: quit  F12: debug")
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, help_area);
}

// ============================================================================
// Events handler - send actions when an event happens
// ============================================================================

fn map_event(event: &EventKind, _state: &AppState) -> Option<AppAction> {
    if let EventKind::Key(key) = event {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Char('k') | KeyCode::Up => Some(AppAction::CountIncrement),
            KeyCode::Char('j') | KeyCode::Down => Some(AppAction::CountDecrement),
            KeyCode::Char('q') | KeyCode::Esc => Some(AppAction::Quit),
            _ => None,
        }
    } else {
        None
    }
}

// ============================================================================
// Main - Setup terminal, run event loop, cleanup
// ============================================================================

#[tokio::main]
async fn main() -> io::Result<()> {
    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // start the loop
    let result = run_app(&mut terminal).await;

    // Cleanup
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>) -> io::Result<()> {
    // Debug layer (F12 to toggle)
    let debug: DebugLayer<AppAction> = DebugLayer::simple();

    let mut runtime = DispatchRuntime::new(AppState::default(), reducer).with_debug(debug);

    runtime
        .run(terminal, render_app, map_event, |action| {
            matches!(action, AppAction::Quit)
        })
        .await
}
