use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Gauge};
use tui_dispatch::{Component, EventKind};

use crate::action::Action;

pub struct SpacingControlProps {
    pub spacing: i16,
    pub is_focused: bool,
}

#[derive(Default)]
pub struct SpacingControl;

impl Component<Action> for SpacingControl {
    type Props<'a> = SpacingControlProps;

    fn handle_event(&mut self, event: &EventKind, props: Self::Props<'_>) -> Vec<Action> {
        if !props.is_focused {
            return vec![];
        }

        match event {
            EventKind::Key(key) => match key.code {
                KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('-') => {
                    vec![Action::SpacingDecrement]
                }
                KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('+') | KeyCode::Char('=') => {
                    vec![Action::SpacingIncrement]
                }
                KeyCode::Char('0') => vec![Action::SpacingSet(0)],
                _ => vec![],
            },
            _ => vec![],
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        let border_style = if props.is_focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Spacing ")
            .border_style(border_style);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Map spacing (-5 to 10) to ratio (0.0 to 1.0)
        let min = -5i16;
        let max = 10i16;
        let ratio = (props.spacing - min) as f64 / (max - min) as f64;

        let gauge_style = if props.is_focused {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        };

        let gauge = Gauge::default()
            .label(format!("{:+}", props.spacing))
            .ratio(ratio)
            .gauge_style(gauge_style);

        frame.render_widget(gauge, inner);
    }
}
