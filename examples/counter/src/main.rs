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
use std::time::Duration;

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Flex, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tui_dispatch::{process_raw_event, spawn_event_poller, Action, EventKind, RawEvent, Store};

// ============================================================================
// State - What the app knows
// ============================================================================

#[derive(Default)]
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

    let result = run_app(&mut terminal).await;

    // Cleanup
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>) -> io::Result<()> {
    // Action channel
    let (action_tx, mut action_rx) = mpsc::unbounded_channel::<AppAction>();

    // Store = state + reducer
    let mut store = Store::new(AppState::default(), reducer);

    // Event poller
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<RawEvent>();
    let cancel_token = CancellationToken::new();
    let _handle = spawn_event_poller(
        event_tx,
        Duration::from_millis(10),
        Duration::from_millis(16),
        cancel_token.clone(),
    );

    let mut should_render = true;

    loop {
        // Render if state changed
        if should_render {
            terminal.draw(|frame| {
                let area = frame.area();

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

                let text = format!("{}", store.state().count);
                let paragraph = Paragraph::new(text)
                    .alignment(Alignment::Center)
                    .block(block);

                frame.render_widget(paragraph, center);

                // Help text at bottom
                let [_, help_area] =
                    Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);
                let help = Paragraph::new("k/Up: +1  j/Down: -1  q: quit")
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(Color::DarkGray));
                frame.render_widget(help, help_area);
            })?;
            should_render = false;
        }

        // Event loop
        tokio::select! {
            Some(raw_event) = event_rx.recv() => {
                let event = process_raw_event(raw_event);

                // Map events to actions
                if let EventKind::Key(key) = event {
                    use crossterm::event::KeyCode;
                    let action = match key.code {
                        KeyCode::Char('k') | KeyCode::Up => Some(AppAction::CountIncrement),
                        KeyCode::Char('j') | KeyCode::Down => Some(AppAction::CountDecrement),
                        KeyCode::Char('q') | KeyCode::Esc => Some(AppAction::Quit),
                        _ => None,
                    };
                    if let Some(a) = action {
                        let _ = action_tx.send(a);
                    }
                }
            }

            Some(action) = action_rx.recv() => {
                if matches!(action, AppAction::Quit) {
                    break;
                }
                should_render = store.dispatch(action);
            }
        }
    }

    cancel_token.cancel();
    Ok(())
}
