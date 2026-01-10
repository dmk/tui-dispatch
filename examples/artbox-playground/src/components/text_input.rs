use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Paragraph};
use tui_dispatch::{Component, EventKind};

use crate::action::Action;

pub struct TextInputProps<'a> {
    pub text: &'a str,
    pub is_focused: bool,
}

#[derive(Default)]
pub struct TextInput {
    cursor_position: usize,
}

impl Component<Action> for TextInput {
    type Props<'a> = TextInputProps<'a>;

    fn handle_event(&mut self, event: &EventKind, props: Self::Props<'_>) -> Vec<Action> {
        if !props.is_focused {
            return vec![];
        }

        match event {
            EventKind::Key(key) => {
                let text = props.text;
                let len = text.chars().count();
                self.cursor_position = self.cursor_position.min(len);

                match key.code {
                    KeyCode::Char(c) => {
                        let mut new_text: String =
                            text.chars().take(self.cursor_position).collect();
                        new_text.push(c);
                        new_text.extend(text.chars().skip(self.cursor_position));
                        self.cursor_position += 1;
                        vec![Action::TextUpdate(new_text)]
                    }
                    KeyCode::Backspace if self.cursor_position > 0 => {
                        let new_text: String = text
                            .chars()
                            .take(self.cursor_position - 1)
                            .chain(text.chars().skip(self.cursor_position))
                            .collect();
                        self.cursor_position -= 1;
                        vec![Action::TextUpdate(new_text)]
                    }
                    KeyCode::Delete if self.cursor_position < len => {
                        let new_text: String = text
                            .chars()
                            .take(self.cursor_position)
                            .chain(text.chars().skip(self.cursor_position + 1))
                            .collect();
                        vec![Action::TextUpdate(new_text)]
                    }
                    KeyCode::Left if self.cursor_position > 0 => {
                        self.cursor_position -= 1;
                        vec![]
                    }
                    KeyCode::Right if self.cursor_position < len => {
                        self.cursor_position += 1;
                        vec![]
                    }
                    KeyCode::Home => {
                        self.cursor_position = 0;
                        vec![]
                    }
                    KeyCode::End => {
                        self.cursor_position = len;
                        vec![]
                    }
                    _ => vec![],
                }
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
            .title(" Text ")
            .border_style(border_style);

        // Show cursor if focused
        let display_text = if props.is_focused {
            let len = props.text.chars().count();
            self.cursor_position = self.cursor_position.min(len);
            let before: String = props.text.chars().take(self.cursor_position).collect();
            let cursor = "│";
            let after: String = props.text.chars().skip(self.cursor_position).collect();
            format!("{}{}{}", before, cursor, after)
        } else {
            props.text.to_string()
        };

        let paragraph = Paragraph::new(display_text).block(block);
        frame.render_widget(paragraph, area);
    }
}
