use artbox::Color as ArtboxColor;
use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph};
use tui_dispatch::{Component, EventKind};

use crate::action::Action;
use crate::state::FillMode;

pub struct ColorPickerProps<'a> {
    pub fill_mode: &'a FillMode,
    pub is_focused: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Channel {
    Red,
    Green,
    Blue,
}

impl Channel {
    fn next(self) -> Self {
        match self {
            Channel::Red => Channel::Green,
            Channel::Green => Channel::Blue,
            Channel::Blue => Channel::Red,
        }
    }

    fn prev(self) -> Self {
        match self {
            Channel::Red => Channel::Blue,
            Channel::Green => Channel::Red,
            Channel::Blue => Channel::Green,
        }
    }
}

pub struct ColorPicker {
    selected_channel: Channel,
}

impl Default for ColorPicker {
    fn default() -> Self {
        Self {
            selected_channel: Channel::Red,
        }
    }
}

impl Component<Action> for ColorPicker {
    type Props<'a> = ColorPickerProps<'a>;

    fn handle_event(&mut self, event: &EventKind, props: Self::Props<'_>) -> Vec<Action> {
        if !props.is_focused {
            return vec![];
        }

        let FillMode::Solid(color) = props.fill_mode else {
            // In gradient mode, just handle mode toggle
            if let EventKind::Key(key) = event
                && (key.code == KeyCode::Tab || key.code == KeyCode::Char('m'))
            {
                return vec![Action::ColorToggleMode];
            }
            return vec![];
        };

        match event {
            EventKind::Key(key) => match key.code {
                KeyCode::Tab | KeyCode::Char('m') => vec![Action::ColorToggleMode],
                KeyCode::Up | KeyCode::Char('k') => {
                    self.selected_channel = self.selected_channel.prev();
                    vec![]
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.selected_channel = self.selected_channel.next();
                    vec![]
                }
                KeyCode::Left | KeyCode::Char('h') => {
                    let new_color = self.adjust_color(color, -10);
                    vec![Action::ColorSetSolid(new_color)]
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    let new_color = self.adjust_color(color, 10);
                    vec![Action::ColorSetSolid(new_color)]
                }
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

        let mode_name = match props.fill_mode {
            FillMode::Solid(_) => "Solid",
            FillMode::Linear(_) => "Linear",
            FillMode::Radial(_) => "Radial",
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Color ({}) ", mode_name))
            .border_style(border_style);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        match props.fill_mode {
            FillMode::Solid(color) => {
                self.render_rgb_sliders(frame, inner, color, props.is_focused);
            }
            FillMode::Linear(_) | FillMode::Radial(_) => {
                let hint = Paragraph::new("Tab to switch mode");
                frame.render_widget(hint, inner);
            }
        }
    }
}

impl ColorPicker {
    fn adjust_color(&self, color: &ArtboxColor, delta: i16) -> ArtboxColor {
        let rgb = color.to_rgb();
        let (r, g, b) = (rgb.r as i16, rgb.g as i16, rgb.b as i16);
        match self.selected_channel {
            Channel::Red => ArtboxColor::rgb((r + delta).clamp(0, 255) as u8, rgb.g, rgb.b),
            Channel::Green => ArtboxColor::rgb(rgb.r, (g + delta).clamp(0, 255) as u8, rgb.b),
            Channel::Blue => ArtboxColor::rgb(rgb.r, rgb.g, (b + delta).clamp(0, 255) as u8),
        }
    }

    fn render_rgb_sliders(
        &self,
        frame: &mut Frame,
        area: Rect,
        color: &ArtboxColor,
        is_focused: bool,
    ) {
        let rgb = color.to_rgb();
        let chunks = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

        let channels = [
            (Channel::Red, "R", rgb.r, Color::Red),
            (Channel::Green, "G", rgb.g, Color::Green),
            (Channel::Blue, "B", rgb.b, Color::Blue),
        ];

        for ((channel, label, value, gauge_color), chunk) in channels.iter().zip(chunks.iter()) {
            let is_selected = is_focused && self.selected_channel == *channel;
            let style = if is_selected {
                Style::default().fg(*gauge_color)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let gauge = Gauge::default()
                .label(format!("{}: {:3}", label, value))
                .ratio(*value as f64 / 255.0)
                .gauge_style(style);

            frame.render_widget(gauge, *chunk);
        }
    }
}
