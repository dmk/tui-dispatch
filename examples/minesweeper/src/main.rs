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

use std::{io, path::PathBuf};

use clap::Parser;
use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tui_dispatch::prelude::*;
#[cfg(unix)]
use tui_dispatch::{observe::DebugCollector, transport::DebugServer};

use action::Action;
use middleware::MinesweeperMiddleware;
use reducer::reducer;
use state::{AppState, Difficulty};

#[derive(Parser)]
#[command(about = "Minesweeper — tui-dispatch middleware showcase")]
struct Args {
    /// Expose a read-only debugger through this Unix socket path
    #[arg(long)]
    debug_socket: Option<PathBuf>,
}

struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, crossterm::cursor::Show);
    }
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    let mut store = StoreWithMiddleware::new(
        AppState::new(Difficulty::Beginner),
        reducer,
        MinesweeperMiddleware,
    );

    // The game keeps its synchronous loop. Only attachment starts a background
    // Tokio worker, and binding errors happen before changing terminal modes.
    #[cfg(unix)]
    let debugger = if let Some(path) = args.debug_socket {
        let collector = DebugCollector::new(serde_json::to_value(store.state())?, 64);
        store.set_observer(collector.observer());
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()?;
        let server = runtime.block_on(DebugServer::bind(path, collector))?;
        Some((runtime, server))
    } else {
        None
    };
    #[cfg(not(unix))]
    if args.debug_socket.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "debugger attachment requires Unix",
        ));
    }

    let result = run_game(&mut store);
    #[cfg(unix)]
    if let Some((runtime, server)) = debugger {
        runtime.block_on(server.shutdown())?;
    }
    result
}

fn run_game(
    store: &mut StoreWithMiddleware<AppState, Action, NoEffect, MinesweeperMiddleware>,
) -> io::Result<()> {
    enable_raw_mode()?;
    let _guard = TerminalGuard;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

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

    Ok(())
}
