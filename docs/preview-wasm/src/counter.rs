mod ansi;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};
use tinycrossterm::event::{Event, KeyCode, KeyEventKind};
use tui_dispatch_core::prelude::*;
use tui_dispatch_macros::Action;

#[derive(Default)]
struct AppState {
    count: i64,
}

#[derive(Clone, Debug, Action)]
enum CounterAction {
    Increment,
    Decrement,
    Reset,
}

fn reducer(state: &mut AppState, action: CounterAction) -> bool {
    match action {
        CounterAction::Increment => state.count = state.count.saturating_add(1),
        CounterAction::Decrement => state.count = state.count.saturating_sub(1),
        CounterAction::Reset => state.count = 0,
    }
    true
}

fn main() -> std::io::Result<()> {
    let (mut cols, mut rows) = ansi::viewport_size_from_args();
    let mut store = Store::new(AppState::default(), reducer);

    for event in ansi::drain_events()? {
        match event {
            Event::Resize(new_cols, new_rows) => {
                cols = new_cols.max(1);
                rows = new_rows.max(1);
            }
            Event::Key(key) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
                match key.code {
                    KeyCode::Char('+') | KeyCode::Char('=') => {
                        store.dispatch(CounterAction::Increment);
                    }
                    KeyCode::Char('-') | KeyCode::Char('_') => {
                        store.dispatch(CounterAction::Decrement);
                    }
                    KeyCode::Char('r') => {
                        store.dispatch(CounterAction::Reset);
                    }
                    _ => {}
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
                Constraint::Min(5),
                Constraint::Length(3),
            ])
            .split(area);

        frame.render_widget(
            Paragraph::new("Counter Example")
                .alignment(Alignment::Center)
                .style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            chunks[0],
        );

        frame.render_widget(
            Paragraph::new(format!(
                "Count: {}\n\n+ / = increment\n- / _ decrement\nr reset",
                store.state().count
            ))
            .block(Block::default().title("counter").borders(Borders::ALL))
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::White)),
            chunks[1],
        );

        frame.render_widget(
            Paragraph::new("Interactive WASM preview (tui-dispatch store)")
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::DarkGray)),
            chunks[2],
        );
    })
}
