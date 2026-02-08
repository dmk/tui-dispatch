//! Minesweeper — Middleware showcase for tui-dispatch
//!
//! Demonstrates both middleware capabilities:
//! - **Cancel**: `before()` prevents revealing flagged/revealed cells and
//!   blocks actions after game over
//! - **Inject**: `after()` drives flood-fill — revealing an empty cell
//!   injects Reveal for all neighbors, each going through the full
//!   middleware pipeline recursively
//!
//! Keys: h/j/k/l or arrows = move, space = reveal, f = flag,
//!        1/2/3 = difficulty, n = new game, q = quit

mod action;
mod middleware;
mod reducer;
mod state;
mod ui;

use std::io;

use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tui_dispatch::prelude::*;

use action::Action;
use middleware::MinesweeperMiddleware;
use reducer::reducer;
use state::{AppState, Difficulty};

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    let mut store = StoreWithMiddleware::new(
        AppState::new(Difficulty::Beginner),
        reducer,
        MinesweeperMiddleware,
    );

    loop {
        terminal.draw(|frame| {
            ui::render(frame, store.state());
        })?;

        if let Event::Key(key) = event::read()? {
            let action = match key.code {
                KeyCode::Char(' ') => {
                    let s = store.state();
                    Action::Reveal(s.cursor_x, s.cursor_y)
                }
                KeyCode::Char('f') => {
                    let s = store.state();
                    Action::ToggleFlag(s.cursor_x, s.cursor_y)
                }
                KeyCode::Char('h') | KeyCode::Left => Action::CursorLeft,
                KeyCode::Char('l') | KeyCode::Right => Action::CursorRight,
                KeyCode::Char('k') | KeyCode::Up => Action::CursorUp,
                KeyCode::Char('j') | KeyCode::Down => Action::CursorDown,
                KeyCode::Char('1') => Action::SetDifficulty(Difficulty::Beginner),
                KeyCode::Char('2') => Action::SetDifficulty(Difficulty::Intermediate),
                KeyCode::Char('3') => Action::SetDifficulty(Difficulty::Expert),
                KeyCode::Char('n') => Action::NewGame,
                KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
                _ => continue,
            };

            if matches!(action, Action::Quit) {
                break;
            }

            store.dispatch(action);
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
