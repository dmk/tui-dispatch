use artbox::Color as ArtboxColor;
use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use tui_dispatch::{Component, EventKind};

use crate::action::{Action, GradientType};
use crate::state::{FillMode, LinearGradientConfig, RadialGradientConfig};

pub struct GradientEditorProps<'a> {
    pub fill_mode: &'a FillMode,
    pub is_focused: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Type,
    Angle,
    StopList,
}

impl Field {
    fn next(self, is_linear: bool) -> Self {
        match self {
            Field::Type if is_linear => Field::Angle,
            Field::Type => Field::StopList,
            Field::Angle => Field::StopList,
            Field::StopList => Field::Type,
        }
    }

    fn prev(self, is_linear: bool) -> Self {
        match self {
            Field::Type => Field::StopList,
            Field::Angle => Field::Type,
            Field::StopList if is_linear => Field::Angle,
            Field::StopList => Field::Type,
        }
    }
}

pub struct GradientEditor {
    selected_field: Field,
    selected_stop: usize,
}

impl Default for GradientEditor {
    fn default() -> Self {
        Self {
            selected_field: Field::Type,
            selected_stop: 0,
        }
    }
}

impl Component<Action> for GradientEditor {
    type Props<'a> = GradientEditorProps<'a>;

    fn handle_event(&mut self, event: &EventKind, props: Self::Props<'_>) -> Vec<Action> {
        if !props.is_focused {
            return vec![];
        }

        // Only active in gradient modes
        let (is_linear, stops_len, angle) = match props.fill_mode {
            FillMode::Linear(config) => (true, config.stops.len(), Some(config.angle)),
            FillMode::Radial(config) => (false, config.stops.len(), None),
            FillMode::Solid(_) => return vec![],
        };

        match event {
            EventKind::Key(key) => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.selected_field = self.selected_field.prev(is_linear);
                    vec![]
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.selected_field = self.selected_field.next(is_linear);
                    vec![]
                }
                KeyCode::Left | KeyCode::Char('h') => match self.selected_field {
                    Field::Type => {
                        let new_type = if is_linear {
                            GradientType::Radial
                        } else {
                            GradientType::Linear
                        };
                        vec![Action::GradientSetType(new_type)]
                    }
                    Field::Angle => {
                        if let Some(a) = angle {
                            vec![Action::GradientSetAngle((a - 15.0).rem_euclid(360.0))]
                        } else {
                            vec![]
                        }
                    }
                    Field::StopList if self.selected_stop > 0 => {
                        self.selected_stop -= 1;
                        vec![]
                    }
                    _ => vec![],
                },
                KeyCode::Right | KeyCode::Char('l') => match self.selected_field {
                    Field::Type => {
                        let new_type = if is_linear {
                            GradientType::Radial
                        } else {
                            GradientType::Linear
                        };
                        vec![Action::GradientSetType(new_type)]
                    }
                    Field::Angle => {
                        if let Some(a) = angle {
                            vec![Action::GradientSetAngle((a + 15.0).rem_euclid(360.0))]
                        } else {
                            vec![]
                        }
                    }
                    Field::StopList if self.selected_stop < stops_len.saturating_sub(1) => {
                        self.selected_stop += 1;
                        vec![]
                    }
                    _ => vec![],
                },
                KeyCode::Char('+') | KeyCode::Char('=')
                    if self.selected_field == Field::StopList =>
                {
                    // Add a new stop at midpoint
                    let position = 0.5;
                    let color = ArtboxColor::rgb(200, 200, 200);
                    vec![Action::GradientAddStop(artbox::ColorStop::new(
                        position, color,
                    ))]
                }
                KeyCode::Char('-') if self.selected_field == Field::StopList => {
                    if stops_len > 2 {
                        vec![Action::GradientRemoveStop(self.selected_stop)]
                    } else {
                        vec![]
                    }
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

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Gradient ")
            .border_style(border_style);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        match props.fill_mode {
            FillMode::Solid(_) => {
                let hint = Paragraph::new("Switch to gradient mode in Color panel");
                frame.render_widget(hint, inner);
            }
            FillMode::Linear(config) => {
                self.render_linear(frame, inner, config, props.is_focused);
            }
            FillMode::Radial(config) => {
                self.render_radial(frame, inner, config, props.is_focused);
            }
        }
    }
}

impl GradientEditor {
    fn render_linear(
        &self,
        frame: &mut Frame,
        area: Rect,
        config: &LinearGradientConfig,
        is_focused: bool,
    ) {
        let chunks = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(area);

        // Type row
        let type_style = if is_focused && self.selected_field == Field::Type {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        let type_line = Line::from(vec![
            Span::raw("Type: "),
            Span::styled("Linear", type_style),
            Span::raw(" / Radial"),
        ]);
        frame.render_widget(Paragraph::new(type_line), chunks[0]);

        // Angle row
        let angle_style = if is_focused && self.selected_field == Field::Angle {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        let angle_line = Line::from(vec![
            Span::raw("Angle: "),
            Span::styled(format!("{:.0}°", config.angle), angle_style),
        ]);
        frame.render_widget(Paragraph::new(angle_line), chunks[1]);

        // Stops row
        self.render_stops(frame, chunks[2], &config.stops, is_focused);
    }

    fn render_radial(
        &self,
        frame: &mut Frame,
        area: Rect,
        config: &RadialGradientConfig,
        is_focused: bool,
    ) {
        let chunks = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(area);

        // Type row
        let type_style = if is_focused && self.selected_field == Field::Type {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        let type_line = Line::from(vec![
            Span::raw("Type: Linear / "),
            Span::styled("Radial", type_style),
        ]);
        frame.render_widget(Paragraph::new(type_line), chunks[0]);

        // Center row (read-only, set by mouse in preview)
        let center_line = Line::from(format!(
            "Center: ({:.2}, {:.2}) - click preview to set",
            config.center.0, config.center.1
        ));
        frame.render_widget(
            Paragraph::new(center_line).style(Style::default().fg(Color::DarkGray)),
            chunks[1],
        );

        // Stops row
        self.render_stops(frame, chunks[2], &config.stops, is_focused);
    }

    fn render_stops(
        &self,
        frame: &mut Frame,
        area: Rect,
        stops: &[artbox::ColorStop],
        is_focused: bool,
    ) {
        let is_selected = is_focused && self.selected_field == Field::StopList;
        let style = if is_selected {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        };

        let mut spans = vec![Span::raw("Stops: ")];
        for (i, stop) in stops.iter().enumerate() {
            let rgb = stop.color.to_rgb();
            let stop_style = if is_selected && i == self.selected_stop {
                Style::default()
                    .fg(Color::Rgb(rgb.r, rgb.g, rgb.b))
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else {
                Style::default().fg(Color::Rgb(rgb.r, rgb.g, rgb.b))
            };
            spans.push(Span::styled("■", stop_style));
            if i < stops.len() - 1 {
                spans.push(Span::raw(" "));
            }
        }
        if is_selected {
            spans.push(Span::styled(" (+/-)", style));
        }

        let line = Line::from(spans);
        frame.render_widget(Paragraph::new(line), area);
    }
}
