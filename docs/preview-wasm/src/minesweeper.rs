mod ansi;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use tinycrossterm::event::{Event, KeyCode, KeyEventKind};
use tui_dispatch_core::prelude::*;
use tui_dispatch_macros::Action;

const WIDTH: usize = 8;
const HEIGHT: usize = 6;
const MINES: &[(usize, usize)] = &[(1, 0), (4, 1), (6, 2), (0, 4), (3, 5), (7, 3)];
const SAFE_CELLS: usize = WIDTH * HEIGHT - MINES.len();

#[derive(Clone, Copy, Default)]
struct Cell {
    mine: bool,
    revealed: bool,
    flagged: bool,
}

struct AppState {
    cells: [[Cell; WIDTH]; HEIGHT],
    cursor_x: usize,
    cursor_y: usize,
    game_over: bool,
    won: bool,
    revealed_safe: usize,
}

impl AppState {
    fn new() -> Self {
        let mut cells = [[Cell::default(); WIDTH]; HEIGHT];
        for &(x, y) in MINES {
            cells[y][x].mine = true;
        }

        Self {
            cells,
            cursor_x: 0,
            cursor_y: 0,
            game_over: false,
            won: false,
            revealed_safe: 0,
        }
    }

    fn move_cursor(&mut self, dx: isize, dy: isize) {
        self.cursor_x = (self.cursor_x as isize + dx).clamp(0, (WIDTH - 1) as isize) as usize;
        self.cursor_y = (self.cursor_y as isize + dy).clamp(0, (HEIGHT - 1) as isize) as usize;
    }

    fn adjacent_mines(&self, x: usize, y: usize) -> u8 {
        neighbors(x, y)
            .into_iter()
            .filter(|&(nx, ny)| self.cells[ny][nx].mine)
            .count() as u8
    }

    fn flags_used(&self) -> usize {
        self.cells
            .iter()
            .flat_map(|r| r.iter())
            .filter(|c| c.flagged)
            .count()
    }
}

fn neighbors(x: usize, y: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::with_capacity(8);
    for dy in -1isize..=1 {
        for dx in -1isize..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = x as isize + dx;
            let ny = y as isize + dy;
            if nx >= 0 && ny >= 0 && nx < WIDTH as isize && ny < HEIGHT as isize {
                out.push((nx as usize, ny as usize));
            }
        }
    }
    out
}

#[derive(Clone, Debug, Action)]
enum MinesAction {
    Reveal(usize, usize),
    ToggleFlag(usize, usize),
    CursorUp,
    CursorDown,
    CursorLeft,
    CursorRight,
    Reset,
}

fn reducer(state: &mut AppState, action: MinesAction) -> bool {
    match action {
        MinesAction::Reveal(x, y) => {
            let cell = &mut state.cells[y][x];
            if cell.revealed {
                return false;
            }

            cell.revealed = true;
            if cell.mine {
                state.game_over = true;
            } else {
                state.revealed_safe += 1;
                if state.revealed_safe >= SAFE_CELLS {
                    state.won = true;
                }
            }
            true
        }
        MinesAction::ToggleFlag(x, y) => {
            let cell = &mut state.cells[y][x];
            cell.flagged = !cell.flagged;
            true
        }
        MinesAction::CursorUp => {
            state.move_cursor(0, -1);
            true
        }
        MinesAction::CursorDown => {
            state.move_cursor(0, 1);
            true
        }
        MinesAction::CursorLeft => {
            state.move_cursor(-1, 0);
            true
        }
        MinesAction::CursorRight => {
            state.move_cursor(1, 0);
            true
        }
        MinesAction::Reset => {
            *state = AppState::new();
            true
        }
    }
}

struct MinesMiddleware;

impl Middleware<AppState, MinesAction> for MinesMiddleware {
    fn before(&mut self, action: &MinesAction, state: &AppState) -> bool {
        match action {
            MinesAction::Reveal(x, y) => {
                !state.game_over
                    && !state.won
                    && !state.cells[*y][*x].revealed
                    && !state.cells[*y][*x].flagged
            }
            MinesAction::ToggleFlag(x, y) => {
                !state.game_over && !state.won && !state.cells[*y][*x].revealed
            }
            _ => true,
        }
    }

    fn after(
        &mut self,
        action: &MinesAction,
        state_changed: bool,
        state: &AppState,
    ) -> Vec<MinesAction> {
        if !state_changed {
            return vec![];
        }

        match action {
            MinesAction::Reveal(x, y)
                if !state.game_over
                    && !state.won
                    && state.cells[*y][*x].revealed
                    && !state.cells[*y][*x].mine
                    && state.adjacent_mines(*x, *y) == 0 =>
            {
                neighbors(*x, *y)
                    .into_iter()
                    .map(|(nx, ny)| MinesAction::Reveal(nx, ny))
                    .collect()
            }
            _ => vec![],
        }
    }
}

fn main() -> std::io::Result<()> {
    let (mut cols, mut rows) = ansi::viewport_size_from_args();
    let mut store = StoreWithMiddleware::new(AppState::new(), reducer, MinesMiddleware);

    for event in ansi::drain_events()? {
        match event {
            Event::Resize(new_cols, new_rows) => {
                cols = new_cols.max(1);
                rows = new_rows.max(1);
            }
            Event::Key(key) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
                let action = match key.code {
                    KeyCode::Left | KeyCode::Char('h') => Some(MinesAction::CursorLeft),
                    KeyCode::Right | KeyCode::Char('l') => Some(MinesAction::CursorRight),
                    KeyCode::Up | KeyCode::Char('k') => Some(MinesAction::CursorUp),
                    KeyCode::Down | KeyCode::Char('j') => Some(MinesAction::CursorDown),
                    KeyCode::Char('f') => {
                        let s = store.state();
                        Some(MinesAction::ToggleFlag(s.cursor_x, s.cursor_y))
                    }
                    KeyCode::Char('r') => Some(MinesAction::Reset),
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        let s = store.state();
                        Some(MinesAction::Reveal(s.cursor_x, s.cursor_y))
                    }
                    _ => None,
                };

                if let Some(action) = action {
                    store.dispatch(action);
                }
            }
            _ => {}
        }
    }

    ansi::draw_frame(cols, rows, |frame| {
        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(8),
                Constraint::Length(3),
            ])
            .split(area);

        let state = store.state();
        let status = if state.won {
            "You won"
        } else if state.game_over {
            "Game over"
        } else {
            "Playing"
        };

        frame.render_widget(
            Paragraph::new("Minesweeper Example")
                .style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
                .alignment(Alignment::Center),
            chunks[0],
        );

        let mut lines: Vec<Line> = Vec::with_capacity(HEIGHT + 2);
        for y in 0..HEIGHT {
            let mut spans: Vec<Span> = Vec::with_capacity(WIDTH);
            for x in 0..WIDTH {
                let cell = state.cells[y][x];
                let (glyph, mut style) = if state.game_over && cell.mine {
                    ('*', Style::default().fg(Color::Red))
                } else if cell.revealed {
                    if cell.mine {
                        ('*', Style::default().fg(Color::Red))
                    } else {
                        let adj = state.adjacent_mines(x, y);
                        if adj == 0 {
                            (' ', Style::default().fg(Color::Gray))
                        } else {
                            ((b'0' + adj) as char, Style::default().fg(Color::Cyan))
                        }
                    }
                } else if cell.flagged {
                    ('F', Style::default().fg(Color::Yellow))
                } else {
                    ('#', Style::default().fg(Color::DarkGray))
                };

                if x == state.cursor_x && y == state.cursor_y {
                    style = style
                        .bg(Color::Blue)
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD);
                }

                spans.push(Span::styled(format!(" {glyph} "), style));
            }
            lines.push(Line::from(spans));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(format!(
            "Status: {status} | Flags: {}/{}",
            state.flags_used(),
            MINES.len()
        )));

        frame.render_widget(
            Paragraph::new(lines)
                .block(Block::default().title("minesweeper").borders(Borders::ALL)),
            chunks[1],
        );

        frame.render_widget(
            Paragraph::new("h/j/k/l or arrows move | space reveal | f flag | r reset")
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Center),
            chunks[2],
        );
    })
}
