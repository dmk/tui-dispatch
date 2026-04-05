//! Minesweeper — Middleware showcase for tui-dispatch
//!
//! Demonstrates both middleware capabilities:
//! - **Cancel**: `before()` prevents revealing flagged/revealed cells and
//!   blocks actions after game over
//! - **Inject**: `after()` performs flood-fill by injecting follow-up Reveal
//!   actions through the middleware pipeline
//!
//! Keys: h/j/k/l or arrows = move, space = reveal, f = flag,
//!        1/2/3 = difficulty, n = new game, D = debug, q = quit
//!
//! Run with debug overlay enabled:
//! `cargo run -p minesweeper-example -- --debug`

#[cfg(target_arch = "wasm32")]
extern crate tinycrossterm as crossterm;

mod action;
#[cfg(target_arch = "wasm32")]
mod ansi;
mod middleware;
mod reducer;
mod state;
mod ui;

use std::io;

#[cfg(not(target_arch = "wasm32"))]
use crossterm::event;
use crossterm::event::{Event, KeyCode, KeyEventKind};
#[cfg(not(target_arch = "wasm32"))]
use crossterm::execute;
#[cfg(not(target_arch = "wasm32"))]
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
#[cfg(not(target_arch = "wasm32"))]
use ratatui::{backend::CrosstermBackend, Terminal};
#[cfg(not(target_arch = "wasm32"))]
use tui_dispatch::debug::DebugLayer;
use tui_dispatch::prelude::*;

use action::Action;
use middleware::MinesweeperMiddleware;
use reducer::reducer;
use state::{AppState, Difficulty, GameState};

impl DebugState for AppState {
    fn debug_sections(&self) -> Vec<DebugSection> {
        let game_state = match self.game_state {
            GameState::Playing => "playing",
            GameState::Won => "won",
            GameState::Lost => "lost",
        };

        let cursor = &self.grid[self.cursor_y][self.cursor_x];

        vec![
            DebugSection::new("Game")
                .entry("state", game_state)
                .entry("difficulty", self.difficulty.label())
                .entry("size", format!("{}x{}", self.width, self.height))
                .entry("mines", self.mine_count.to_string())
                .entry("flags", self.flags_placed.to_string())
                .entry("revealed", self.cells_revealed.to_string())
                .entry("safe_total", self.total_safe.to_string()),
            DebugSection::new("Cursor")
                .entry("x", self.cursor_x.to_string())
                .entry("y", self.cursor_y.to_string())
                .entry("revealed", cursor.revealed.to_string())
                .entry("flagged", cursor.flagged.to_string())
                .entry("adjacent", cursor.adjacent.to_string())
                .entry("mine", cursor.mine.to_string()),
        ]
    }
}

fn new_store() -> StoreWithMiddleware<AppState, Action, MinesweeperMiddleware> {
    StoreWithMiddleware::new(
        AppState::new(Difficulty::Beginner),
        reducer,
        MinesweeperMiddleware::default(),
    )
    .with_dispatch_limits(DispatchLimits {
        max_depth: 4096,
        max_actions: 200_000,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn run_native(debug_enabled: bool) -> io::Result<()> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    let mut store = new_store();
    let mut debug: DebugLayer<Action> =
        DebugLayer::simple_with_toggle_key(KeyCode::Char('D')).active(debug_enabled);

    loop {
        terminal.draw(|frame| {
            debug.render_state(frame, store.state(), |frame, area| {
                ui::render_in(frame, area, store.state());
            });
        })?;

        let raw_event = event::read()?;
        let event_kind = match &raw_event {
            Event::Key(key) => Some(process_raw_event(RawEvent::Key(*key))),
            Event::Mouse(mouse) => Some(process_raw_event(RawEvent::Mouse(*mouse))),
            Event::Resize(w, h) => Some(process_raw_event(RawEvent::Resize(*w, *h))),
            _ => None,
        };
        if let Some(event_kind) = event_kind {
            if debug.intercepts_with_state(&event_kind, store.state()) {
                continue;
            }
        }

        let Event::Key(key) = raw_event else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

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

        if let Err(err) = store.try_dispatch(action) {
            eprintln!("dispatch error: {err}");
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
    cols = cols.clamp(1, 300);
    rows = rows.clamp(1, 120);

    let mut store = new_store();

    // Initial render
    ansi::draw_frame(cols, rows, |frame| {
        ui::render_in(frame, frame.area(), store.state());
    })?;

    loop {
        let raw_event = event::read()?;

        match raw_event {
            Event::Resize(new_cols, new_rows) => {
                cols = new_cols.clamp(1, 300);
                rows = new_rows.clamp(1, 120);
            }
            Event::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                let action = match key.code {
                    KeyCode::Char(' ') => {
                        let s = store.state();
                        Some(Action::Reveal(s.cursor_x, s.cursor_y))
                    }
                    KeyCode::Char('f') => {
                        let s = store.state();
                        Some(Action::ToggleFlag(s.cursor_x, s.cursor_y))
                    }
                    KeyCode::Char('h') | KeyCode::Left => Some(Action::CursorLeft),
                    KeyCode::Char('l') | KeyCode::Right => Some(Action::CursorRight),
                    KeyCode::Char('k') | KeyCode::Up => Some(Action::CursorUp),
                    KeyCode::Char('j') | KeyCode::Down => Some(Action::CursorDown),
                    KeyCode::Char('1') => Some(Action::SetDifficulty(Difficulty::Beginner)),
                    KeyCode::Char('2') => Some(Action::SetDifficulty(Difficulty::Intermediate)),
                    KeyCode::Char('3') => Some(Action::SetDifficulty(Difficulty::Expert)),
                    KeyCode::Char('n') => Some(Action::NewGame),
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    _ => None,
                };

                if let Some(action) = action {
                    if let Err(err) = store.try_dispatch(action) {
                        eprintln!("dispatch error: {err}");
                    }
                }
            }
            _ => continue,
        }

        ansi::draw_frame(cols, rows, |frame| {
            ui::render_in(frame, frame.area(), store.state());
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
        let debug_enabled = std::env::args().any(|arg| arg == "--debug");
        run_native(debug_enabled)
    }
}
