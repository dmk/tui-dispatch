use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use tui_dispatch::Component;
use tui_dispatch_components::prelude::*;

use crate::state::{AppState, GameState};

/// Characters per cell width.
const CELL_W: u16 = 3;

pub fn render(frame: &mut Frame, state: &AppState) {
    let area = frame.area();

    let [board_area, info_area, help_area] = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(area);

    draw_board(frame, state, board_area);
    draw_info(frame, state, info_area);
    draw_help(frame, state, help_area);
}

fn draw_board(frame: &mut Frame, state: &AppState, area: Rect) {
    let title = match state.game_state {
        GameState::Playing => format!(" Minesweeper — {} ", state.difficulty.label()),
        GameState::Won => " You Win! ".to_string(),
        GameState::Lost => " Game Over ".to_string(),
    };
    let border_color = match state.game_state {
        GameState::Playing => Color::Cyan,
        GameState::Won => Color::Green,
        GameState::Lost => Color::Red,
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Center the grid within the inner area
    let grid_w = state.width as u16 * CELL_W;
    let grid_h = state.height as u16;
    let offset_x = inner.width.saturating_sub(grid_w) / 2;
    let offset_y = inner.height.saturating_sub(grid_h) / 2;

    for y in 0..state.height {
        if y as u16 >= inner.height {
            break;
        }

        let spans: Vec<Span> = (0..state.width).map(|x| cell_span(state, x, y)).collect();

        let line = Line::from(spans);
        let row_area = Rect {
            x: inner.x + offset_x,
            y: inner.y + offset_y + y as u16,
            width: grid_w.min(inner.width),
            height: 1,
        };
        frame.render_widget(Paragraph::new(line), row_area);
    }
}

fn cell_span(state: &AppState, x: usize, y: usize) -> Span<'static> {
    let cell = &state.grid[y][x];
    let is_cursor = x == state.cursor_x && y == state.cursor_y;

    // On game over, reveal all mines
    let show_mine = cell.mine && state.game_state == GameState::Lost;

    let (text, mut style) = if show_mine && !cell.revealed {
        // Unrevealed mine shown on game over
        (" * ".to_string(), Style::default().fg(Color::Red))
    } else if cell.revealed {
        if cell.mine {
            // The mine you hit
            (
                " * ".to_string(),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )
        } else if cell.adjacent == 0 {
            ("   ".to_string(), Style::default())
        } else {
            let color = number_color(cell.adjacent);
            (
                format!(" {} ", cell.adjacent),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )
        }
    } else if cell.flagged {
        (" \u{25a0} ".to_string(), Style::default().fg(Color::Red))
    } else {
        (
            " \u{00b7} ".to_string(),
            Style::default().fg(Color::DarkGray),
        )
    };

    if is_cursor {
        style = style.bg(Color::DarkGray).add_modifier(Modifier::BOLD);
    }

    Span::styled(text, style)
}

fn number_color(n: u8) -> Color {
    match n {
        1 => Color::Blue,
        2 => Color::Green,
        3 => Color::Red,
        4 => Color::Magenta,
        5 => Color::Yellow,
        6 => Color::Cyan,
        7 => Color::Gray,
        8 => Color::DarkGray,
        _ => Color::White,
    }
}

fn draw_info(frame: &mut Frame, state: &AppState, area: Rect) {
    let status = match state.game_state {
        GameState::Playing => "Playing".to_string(),
        GameState::Won => "You win!".to_string(),
        GameState::Lost => "Boom!".to_string(),
    };
    let status_color = match state.game_state {
        GameState::Playing => Color::Cyan,
        GameState::Won => Color::Green,
        GameState::Lost => Color::Red,
    };

    let left_items = [
        StatusBarItem::text("Mines:"),
        StatusBarItem::span(Span::styled(
            format!(" {}", state.mine_count),
            Style::default().fg(Color::Yellow),
        )),
        StatusBarItem::text("  Flags:"),
        StatusBarItem::span(Span::styled(
            format!(" {}/{}", state.flags_placed, state.mine_count),
            Style::default().fg(Color::Red),
        )),
    ];

    let right_items = [StatusBarItem::span(Span::styled(
        status,
        Style::default().fg(status_color),
    ))];

    let mut status_bar = StatusBar::new();
    <StatusBar as Component<()>>::render(
        &mut status_bar,
        frame,
        area,
        StatusBarProps {
            left: StatusBarSection::items(&left_items),
            center: StatusBarSection::empty(),
            right: StatusBarSection::items(&right_items),
            style: StatusBarStyle::minimal(),
            is_focused: false,
        },
    );
}

fn draw_help(frame: &mut Frame, state: &AppState, area: Rect) {
    let hints: &[StatusBarHint] = if state.game_state == GameState::Playing {
        &[
            StatusBarHint::new("hjkl", "move"),
            StatusBarHint::new("space", "reveal"),
            StatusBarHint::new("f", "flag"),
            StatusBarHint::new("1/2/3", "difficulty"),
            StatusBarHint::new("n", "new"),
            StatusBarHint::new("q", "quit"),
        ]
    } else {
        &[
            StatusBarHint::new("1/2/3", "difficulty"),
            StatusBarHint::new("n", "new game"),
            StatusBarHint::new("q", "quit"),
        ]
    };

    let mut status_bar = StatusBar::new();
    <StatusBar as Component<()>>::render(
        &mut status_bar,
        frame,
        area,
        StatusBarProps {
            left: StatusBarSection::hints(hints),
            center: StatusBarSection::empty(),
            right: StatusBarSection::empty(),
            style: StatusBarStyle::minimal(),
            is_focused: false,
        },
    );
}
