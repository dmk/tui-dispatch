use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use tui_dispatch::{Component, EventKind};

use crate::action::Action;
use crate::state::FontFamily;

pub struct FontSelectorProps {
    pub family: FontFamily,
    pub is_focused: bool,
}

#[derive(Default)]
pub struct FontSelector;

impl Component<Action> for FontSelector {
    type Props<'a> = FontSelectorProps;

    fn handle_event(&mut self, event: &EventKind, props: Self::Props<'_>) -> Vec<Action> {
        if !props.is_focused {
            return vec![];
        }

        match event {
            EventKind::Key(key) => match key.code {
                KeyCode::Left | KeyCode::Char('h') => vec![Action::FontCyclePrev],
                KeyCode::Right | KeyCode::Char('l') => vec![Action::FontCycleNext],
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
            .title(" Font ")
            .border_style(border_style);

        let families = FontFamily::all();
        let spans: Vec<Span> = families
            .iter()
            .enumerate()
            .flat_map(|(i, &family)| {
                let style = if family == props.family {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                };
                let separator = if i < families.len() - 1 { " | " } else { "" };
                vec![Span::styled(family.name(), style), Span::raw(separator)]
            })
            .collect();

        let line = Line::from(spans);
        let paragraph = Paragraph::new(line).block(block);
        frame.render_widget(paragraph, area);
    }
}
