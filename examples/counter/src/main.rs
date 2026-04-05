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

#[cfg(target_arch = "wasm32")]
extern crate tinycrossterm as crossterm;

#[cfg(target_arch = "wasm32")]
mod ansi;

use std::io;

#[cfg(not(target_arch = "wasm32"))]
use crossterm::event;
use crossterm::event::{Event, KeyCode};
#[cfg(not(target_arch = "wasm32"))]
use crossterm::execute;
#[cfg(not(target_arch = "wasm32"))]
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::layout::{Alignment, Constraint, Flex, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
#[cfg(not(target_arch = "wasm32"))]
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

fn render_ui(frame: &mut Frame, count: i32, help_text: &str) {
    let area = frame.area();

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

    let paragraph = Paragraph::new(format!("{count}"))
        .alignment(Alignment::Center)
        .block(block);

    frame.render_widget(paragraph, center);

    let [_, help_area] = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);
    let help = Paragraph::new(help_text)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, help_area);
}

#[cfg(not(target_arch = "wasm32"))]
fn run_native() -> io::Result<()> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    let mut store = Store::new(AppState::default(), reducer);

    loop {
        terminal.draw(|frame| {
            render_ui(
                frame,
                store.state().count,
                "k/↑ increment  j/↓ decrement  q quit",
            );
        })?;

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

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn run_wasm() -> io::Result<()> {
    use crossterm::event;

    let (mut cols, mut rows) = crossterm::terminal::size().unwrap_or((80, 24));
    let mut store = Store::new(AppState::default(), reducer);

    // Initial render
    ansi::draw_frame(cols, rows, |frame| {
        render_ui(
            frame,
            store.state().count,
            "k/↑ increment  j/↓ decrement  q quit",
        );
    })?;

    loop {
        let raw_event = event::read()?;

        match raw_event {
            Event::Resize(new_cols, new_rows) => {
                cols = new_cols.max(1);
                rows = new_rows.max(1);
            }
            Event::Key(key) => {
                let action = match key.code {
                    KeyCode::Char('k') | KeyCode::Up => Some(Action::Increment),
                    KeyCode::Char('j') | KeyCode::Down => Some(Action::Decrement),
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    _ => None,
                };

                if let Some(action) = action {
                    let _ = store.dispatch(action);
                }
            }
            _ => continue,
        }

        ansi::draw_frame(cols, rows, |frame| {
            render_ui(
                frame,
                store.state().count,
                "k/↑ increment  j/↓ decrement  q quit",
            );
        })?;
    }

    Ok(())
}

fn main() -> io::Result<()> {
    #[cfg(target_arch = "wasm32")]
    {
        return run_wasm();
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        run_native()
    }
}
