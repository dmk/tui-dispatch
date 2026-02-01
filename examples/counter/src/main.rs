//! Counter - Minimal tui-dispatch example
//!
//! Demonstrates the core pattern:
//! - State: what the app knows
//! - Action: what can happen
//! - Reducer: how state changes
//! - Store: holds state, applies reducer
//!
//! No async runtime, no extensions - just the essentials.
//!
//! Keys: k/Up = increment, j/Down = decrement, q = quit

use std::io;

use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::layout::{Alignment, Constraint, Flex, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::{backend::CrosstermBackend, Terminal};
use tui_dispatch::prelude::*;

// State - what the app knows
#[derive(Default)]
struct AppState {
    count: i32,
}

// Action - what can happen
#[derive(Clone, Debug, Action)]
enum Action {
    Increment,
    Decrement,
    Quit,
}

// Reducer - how state changes
fn reducer(state: &mut AppState, action: Action) -> bool {
    match action {
        Action::Increment => {
            state.count += 1;
            true
        }
        Action::Decrement => {
            state.count -= 1;
            true
        }
        Action::Quit => false,
    }
}

fn main() -> io::Result<()> {
    // Terminal setup
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    // Create store
    let mut store = Store::new(AppState::default(), reducer);

    // Main loop
    loop {
        // Render
        terminal.draw(|frame| {
            let area = frame.area();

            // Center the counter
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

            let paragraph = Paragraph::new(format!("{}", store.state().count))
                .alignment(Alignment::Center)
                .block(block);

            frame.render_widget(paragraph, center);

            // Help text at bottom
            let [_, help_area] =
                Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);
            let help = Paragraph::new("k/↑ increment  j/↓ decrement  q quit")
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(help, help_area);
        })?;

        // Handle input
        if let Event::Key(key) = event::read()? {
            let action = match key.code {
                KeyCode::Char('k') | KeyCode::Up => Action::Increment,
                KeyCode::Char('j') | KeyCode::Down => Action::Decrement,
                KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
                _ => continue,
            };

            if !store.dispatch(action) {
                break;
            }
        }
    }

    // Cleanup
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
