use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::Paragraph,
};

use super::{Component, ERROR_ICON, LocationHeader, LocationHeaderProps, SPINNERS};
use crate::action::Action;
use crate::sprites;
use crate::state::{AppState, WeatherData};

pub struct WeatherBody;

pub struct WeatherBodyProps<'a> {
    pub state: &'a AppState,
}

impl Component<Action> for WeatherBody {
    type Props<'a> = WeatherBodyProps<'a>;

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        let blocks = blocks_for_state(props.state);
        if blocks.is_empty() {
            return;
        }

        let mut constraints = Vec::with_capacity(blocks.len() + 1);
        constraints.push(Constraint::Length(LocationHeader::HEIGHT));
        constraints.extend(
            blocks
                .iter()
                .map(|block| Constraint::Length(block.height())),
        );

        let chunks = Layout::vertical(constraints).flex(Flex::Center).split(area);
        let (header_area, body_areas) = chunks.split_first().unwrap();

        let mut header = LocationHeader;
        header.render(
            frame,
            *header_area,
            LocationHeaderProps {
                location: props.state.current_location(),
            },
        );

        for (block, area) in blocks.into_iter().zip(body_areas.iter().copied()) {
            block.render(frame, area);
        }
    }
}

enum WeatherView<'a> {
    Error(&'a str),
    Ready(&'a WeatherData),
    Loading,
    Empty,
}

impl<'a> WeatherView<'a> {
    fn from_state(state: &'a AppState) -> Self {
        if let Some(error) = state.error.as_deref() {
            WeatherView::Error(error)
        } else if let Some(weather) = state.weather.as_ref() {
            WeatherView::Ready(weather)
        } else if state.is_loading {
            WeatherView::Loading
        } else {
            WeatherView::Empty
        }
    }
}

enum BodyBlock {
    Line(Line<'static>),
    Sprite { art: Text<'static>, height: u16 },
}

impl BodyBlock {
    fn height(&self) -> u16 {
        match self {
            BodyBlock::Line(_) => 1,
            BodyBlock::Sprite { height, .. } => *height,
        }
    }

    fn render(self, frame: &mut Frame, area: Rect) {
        match self {
            BodyBlock::Line(line) => {
                frame.render_widget(Paragraph::new(line), area);
            }
            BodyBlock::Sprite { art, .. } => {
                frame.render_widget(Paragraph::new(art).alignment(Alignment::Center), area);
            }
        }
    }
}

fn blocks_for_state(state: &AppState) -> Vec<BodyBlock> {
    let view = WeatherView::from_state(state);

    match view {
        WeatherView::Error(error) => vec![
            blank_line(),
            BodyBlock::Line(Line::from(ERROR_ICON).centered()),
            BodyBlock::Line(
                Line::from(vec![Span::styled(
                    "Error",
                    Style::default().fg(Color::Red).bold(),
                )])
                .centered(),
            ),
            BodyBlock::Line(
                Line::from(vec![Span::styled(
                    error.to_string(),
                    Style::default().fg(Color::Rgb(200, 100, 100)),
                )])
                .centered(),
            ),
            blank_line(),
            BodyBlock::Line(
                Line::from(vec![
                    Span::styled("Press ", Style::default().fg(Color::DarkGray)),
                    Span::styled("r", Style::default().fg(Color::Cyan).bold()),
                    Span::styled(" to retry", Style::default().fg(Color::DarkGray)),
                ])
                .centered(),
            ),
        ],
        WeatherView::Ready(weather) => {
            let (art_lines, _) = sprites::weather_sprite(weather.weather_code, state.terminal_size);
            let sprite_height = art_lines.lines.len() as u16;

            let temp = state.unit.format(weather.temperature);
            let temp_color = temp_to_color(weather.temperature);

            vec![
                blank_line(),
                BodyBlock::Sprite {
                    art: art_lines,
                    height: sprite_height,
                },
                blank_line(),
                BodyBlock::Line(
                    Line::from(vec![Span::styled(
                        temp,
                        Style::default().fg(temp_color).bold(),
                    )])
                    .centered(),
                ),
                BodyBlock::Line(
                    Line::from(vec![Span::styled(
                        weather.description.to_string(),
                        Style::default().fg(Color::Gray),
                    )])
                    .centered(),
                ),
            ]
        }
        WeatherView::Loading => {
            let spinner = SPINNERS[(state.tick_count as usize / 2) % SPINNERS.len()];
            let dots = ".".repeat((state.tick_count as usize / 3) % 4);

            vec![
                blank_line(),
                BodyBlock::Line(
                    Line::from(vec![
                        Span::styled(spinner, Style::default().fg(Color::Cyan)),
                        Span::styled(
                            format!(" Fetching weather{:<3}", dots),
                            Style::default().fg(Color::Gray),
                        ),
                    ])
                    .centered(),
                ),
            ]
        }
        WeatherView::Empty => vec![
            blank_line(),
            BodyBlock::Line(
                Line::from(vec![
                    Span::styled("Press ", Style::default().fg(Color::DarkGray)),
                    Span::styled("r", Style::default().fg(Color::Cyan).bold()),
                    Span::styled(" to fetch weather", Style::default().fg(Color::DarkGray)),
                ])
                .centered(),
            ),
        ],
    }
}

fn blank_line() -> BodyBlock {
    BodyBlock::Line(Line::from("").centered())
}

/// Get temperature-based color
fn temp_to_color(celsius: f32) -> Color {
    match celsius as i32 {
        ..=-10 => Color::Rgb(150, 200, 255),  // Very cold - light blue
        -9..=0 => Color::Rgb(100, 180, 255),  // Cold - blue
        1..=10 => Color::Rgb(100, 220, 200),  // Cool - cyan
        11..=20 => Color::Rgb(150, 230, 150), // Mild - green
        21..=30 => Color::Rgb(255, 220, 100), // Warm - yellow
        31..=40 => Color::Rgb(255, 150, 80),  // Hot - orange
        _ => Color::Rgb(255, 100, 100),       // Very hot - red
    }
}
