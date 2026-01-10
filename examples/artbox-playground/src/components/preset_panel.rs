use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use tui_dispatch::{Component, EventKind};

use crate::action::Action;

pub struct PresetPanelProps<'a> {
    pub preset_names: &'a [String],
    pub current_preset: Option<&'a str>,
    pub is_focused: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Browse,
    Saving,
}

pub struct PresetPanel {
    selected_index: usize,
    mode: Mode,
    save_name: String,
}

impl Default for PresetPanel {
    fn default() -> Self {
        Self {
            selected_index: 0,
            mode: Mode::Browse,
            save_name: String::new(),
        }
    }
}

impl Component<Action> for PresetPanel {
    type Props<'a> = PresetPanelProps<'a>;

    fn handle_event(&mut self, event: &EventKind, props: Self::Props<'_>) -> Vec<Action> {
        if !props.is_focused {
            return vec![];
        }

        match self.mode {
            Mode::Browse => self.handle_browse_event(event, props),
            Mode::Saving => self.handle_saving_event(event),
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
            .title(" Presets ")
            .border_style(border_style);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        match self.mode {
            Mode::Browse => self.render_browse(frame, inner, props),
            Mode::Saving => self.render_saving(frame, inner),
        }
    }
}

impl PresetPanel {
    fn handle_browse_event(
        &mut self,
        event: &EventKind,
        props: PresetPanelProps<'_>,
    ) -> Vec<Action> {
        match event {
            EventKind::Key(key) => match key.code {
                KeyCode::Up | KeyCode::Char('k') if !props.preset_names.is_empty() => {
                    self.selected_index = self.selected_index.saturating_sub(1);
                    vec![]
                }
                KeyCode::Down | KeyCode::Char('j') if !props.preset_names.is_empty() => {
                    self.selected_index =
                        (self.selected_index + 1).min(props.preset_names.len().saturating_sub(1));
                    vec![]
                }
                KeyCode::Enter if !props.preset_names.is_empty() => {
                    let name = props.preset_names[self.selected_index].clone();
                    vec![Action::PresetLoad(name)]
                }
                KeyCode::Char('s') => {
                    self.mode = Mode::Saving;
                    self.save_name = props
                        .current_preset
                        .map(|s| s.to_string())
                        .unwrap_or_default();
                    vec![]
                }
                KeyCode::Char('d') | KeyCode::Delete if !props.preset_names.is_empty() => {
                    let name = props.preset_names[self.selected_index].clone();
                    if self.selected_index > 0 {
                        self.selected_index -= 1;
                    }
                    vec![Action::PresetDelete(name)]
                }
                KeyCode::Char('r') => vec![Action::PresetRefresh],
                _ => vec![],
            },
            _ => vec![],
        }
    }

    fn handle_saving_event(&mut self, event: &EventKind) -> Vec<Action> {
        match event {
            EventKind::Key(key) => match key.code {
                KeyCode::Esc => {
                    self.mode = Mode::Browse;
                    self.save_name.clear();
                    vec![]
                }
                KeyCode::Enter if !self.save_name.is_empty() => {
                    let name = std::mem::take(&mut self.save_name);
                    self.mode = Mode::Browse;
                    vec![Action::PresetSave(name)]
                }
                KeyCode::Char(c) if c.is_alphanumeric() || c == '_' || c == '-' => {
                    if self.save_name.len() < 32 {
                        self.save_name.push(c);
                    }
                    vec![]
                }
                KeyCode::Backspace => {
                    self.save_name.pop();
                    vec![]
                }
                _ => vec![],
            },
            _ => vec![],
        }
    }

    fn render_browse(&mut self, frame: &mut Frame, area: Rect, props: PresetPanelProps<'_>) {
        let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(area);

        // Preset list
        if props.preset_names.is_empty() {
            let empty = Paragraph::new("No presets. Press 's' to save.");
            frame.render_widget(empty, chunks[0]);
        } else {
            self.selected_index = self
                .selected_index
                .min(props.preset_names.len().saturating_sub(1));

            let items: Vec<ListItem> = props
                .preset_names
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    let style = if i == self.selected_index && props.is_focused {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else if Some(name.as_str()) == props.current_preset {
                        Style::default().fg(Color::Green)
                    } else {
                        Style::default().fg(Color::Gray)
                    };
                    ListItem::new(name.as_str()).style(style)
                })
                .collect();

            let list = List::new(items);
            frame.render_widget(list, chunks[0]);
        }

        // Help line
        let help = Line::from(vec![
            Span::styled("s", Style::default().fg(Color::Yellow)),
            Span::raw(":save "),
            Span::styled("d", Style::default().fg(Color::Yellow)),
            Span::raw(":delete "),
            Span::styled("r", Style::default().fg(Color::Yellow)),
            Span::raw(":refresh"),
        ]);
        frame.render_widget(Paragraph::new(help), chunks[1]);
    }

    fn render_saving(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).split(area);

        let prompt = Line::from("Save as:");
        frame.render_widget(Paragraph::new(prompt), chunks[0]);

        let input = Line::from(vec![
            Span::raw(&self.save_name),
            Span::styled("│", Style::default().fg(Color::Yellow)),
        ]);
        frame.render_widget(Paragraph::new(input), chunks[1]);
    }
}
