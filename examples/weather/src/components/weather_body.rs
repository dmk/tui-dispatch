use artbox::{
    Alignment as ArtAlignment, Color as ArtColor, Fill, LinearGradient, Renderer, fonts,
    integrations::ratatui::ArtBox,
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::Paragraph,
};

use super::{Component, ERROR_ICON, LocationHeader, LocationHeaderProps};
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
                temperature: props.state.weather.as_ref().map(|w| w.temperature),
                is_animating: props.state.loading_anim_active(),
                tick_count: props.state.tick_count,
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
    Temperature { text: String, celsius: f32 },
}

impl BodyBlock {
    fn height(&self) -> u16 {
        match self {
            BodyBlock::Line(_) => 1,
            BodyBlock::Sprite { height, .. } => *height,
            BodyBlock::Temperature { .. } => 4, // blocky font height
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
            BodyBlock::Temperature { text, celsius } => {
                let renderer = Renderer::new(fonts::family("blocky").unwrap())
                    .with_alignment(ArtAlignment::Center)
                    .with_fill(temperature_gradient(celsius));
                let widget = ArtBox::new(&renderer, &text);
                frame.render_widget(widget, area);
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

            vec![
                blank_line(),
                BodyBlock::Sprite {
                    art: art_lines,
                    height: sprite_height,
                },
                blank_line(),
                BodyBlock::Temperature {
                    text: temp,
                    celsius: weather.temperature,
                },
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
            // Loading animation is shown via the header gradient
            vec![blank_line()]
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

fn temperature_gradient(celsius: f32) -> Fill {
    let (start, end) = match celsius {
        t if t < 0.0 => (
            ArtColor::rgb(150, 200, 255), // Ice blue
            ArtColor::rgb(200, 230, 255), // Light ice
        ),
        t if t < 15.0 => (
            ArtColor::rgb(100, 180, 255), // Cool blue
            ArtColor::rgb(150, 220, 200), // Teal
        ),
        t if t < 25.0 => (
            ArtColor::rgb(100, 200, 150), // Green
            ArtColor::rgb(255, 220, 100), // Yellow
        ),
        t if t < 35.0 => (
            ArtColor::rgb(255, 180, 80), // Orange
            ArtColor::rgb(255, 120, 80), // Deep orange
        ),
        _ => (
            ArtColor::rgb(255, 100, 80), // Red-orange
            ArtColor::rgb(255, 60, 60),  // Hot red
        ),
    };
    Fill::Linear(LinearGradient::horizontal(start, end))
}
