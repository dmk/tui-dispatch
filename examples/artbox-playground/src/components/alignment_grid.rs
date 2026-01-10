use artbox::Alignment;
use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use tui_dispatch::{Component, EventKind};

use crate::action::Action;

pub struct AlignmentGridProps {
    pub alignment: Alignment,
    pub is_focused: bool,
}

#[derive(Default)]
pub struct AlignmentGrid;

impl Component<Action> for AlignmentGrid {
    type Props<'a> = AlignmentGridProps;

    fn handle_event(&mut self, event: &EventKind, props: Self::Props<'_>) -> Vec<Action> {
        if !props.is_focused {
            return vec![];
        }

        match event {
            EventKind::Key(key) => {
                let new_alignment = match key.code {
                    KeyCode::Char('1') | KeyCode::Char('q') => Some(Alignment::TopLeft),
                    KeyCode::Char('2') | KeyCode::Char('w') => Some(Alignment::Top),
                    KeyCode::Char('3') | KeyCode::Char('e') => Some(Alignment::TopRight),
                    KeyCode::Char('4') | KeyCode::Char('a') => Some(Alignment::Left),
                    KeyCode::Char('5') | KeyCode::Char('s') => Some(Alignment::Center),
                    KeyCode::Char('6') | KeyCode::Char('d') => Some(Alignment::Right),
                    KeyCode::Char('7') | KeyCode::Char('z') => Some(Alignment::BottomLeft),
                    KeyCode::Char('8') | KeyCode::Char('x') => Some(Alignment::Bottom),
                    KeyCode::Char('9') | KeyCode::Char('c') => Some(Alignment::BottomRight),
                    _ => None,
                };

                new_alignment
                    .map(|a| vec![Action::AlignmentSet(a)])
                    .unwrap_or_default()
            }
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
            .title(" Alignment ")
            .border_style(border_style);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // 3x3 grid display
        let grid = [
            [Alignment::TopLeft, Alignment::Top, Alignment::TopRight],
            [Alignment::Left, Alignment::Center, Alignment::Right],
            [
                Alignment::BottomLeft,
                Alignment::Bottom,
                Alignment::BottomRight,
            ],
        ];

        let symbols = [["◤", "▲", "◥"], ["◀", "●", "▶"], ["◣", "▼", "◢"]];

        let lines: Vec<Line> = grid
            .iter()
            .zip(symbols.iter())
            .map(|(row, sym_row)| {
                let spans: Vec<Span> = row
                    .iter()
                    .zip(sym_row.iter())
                    .flat_map(|(&align, &sym)| {
                        let style = if align == props.alignment {
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::Gray)
                        };
                        vec![Span::styled(sym, style), Span::raw(" ")]
                    })
                    .collect();
                Line::from(spans)
            })
            .collect();

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }
}
