//! Weather display component with Unicode art
//!
//! FRAMEWORK PATTERN: Component Trait
//! - Props<'a>: Read-only data for rendering (borrowed from state)
//! - handle_event: Receives EventKind, returns `Vec<Action>`
//! - render: Pure function of props - no side effects
//! - Focus handled via props, not event context

use crossterm::event::KeyCode;
use ratatui::prelude::{Frame, Rect};
use ratatui::{
    layout::{Alignment, Constraint, Flex, Layout},
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
};
use tui_dispatch::EventKind;

use crate::action::Action;
use crate::sprites;
use crate::state::AppState;

/// Props for WeatherDisplay - read-only view of state
pub struct WeatherDisplayProps<'a> {
    pub state: &'a AppState,
    pub is_focused: bool,
}

/// The main weather display component
#[derive(Default)]
pub struct WeatherDisplay;

impl WeatherDisplay {
    /// Handle an event and return actions to dispatch
    pub fn handle_event(
        &mut self,
        event: &EventKind,
        props: WeatherDisplayProps<'_>,
    ) -> Vec<Action> {
        if !props.is_focused {
            return vec![];
        }

        match event {
            EventKind::Key(key) => match key.code {
                KeyCode::Char('r') | KeyCode::F(5) => vec![Action::WeatherFetch],
                KeyCode::Char('u') => vec![Action::UiToggleUnits],
                KeyCode::Char('q') | KeyCode::Esc => vec![Action::Quit],
                _ => vec![],
            },
            _ => vec![],
        }
    }

    /// Render the component to the frame
    pub fn render(&mut self, frame: &mut Frame, area: Rect, props: WeatherDisplayProps<'_>) {
        let state = props.state;

        // Main container with nice border
        let outer_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(80, 80, 100)))
            .title(" ☁ Weather ")
            .title_style(Style::default().fg(Color::Cyan).bold())
            .title_alignment(Alignment::Center);

        frame.render_widget(outer_block.clone(), area);
        let inner = outer_block.inner(area);

        // Layout: header, main content, footer
        let chunks = Layout::vertical([
            Constraint::Length(3), // Location header
            Constraint::Min(14),   // Weather art + info
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Help bar
        ])
        .split(inner);

        // Location header with coordinates
        let location = state.current_location();
        let location_text = vec![
            Line::from(vec![
                Span::styled("📍 ", Style::default()),
                Span::styled(&location.name, Style::default().fg(Color::White).bold()),
            ])
            .centered(),
            Line::from(vec![Span::styled(
                format!("{:.2}°N, {:.2}°E", location.lat, location.lon),
                Style::default().fg(Color::DarkGray),
            )])
            .centered(),
        ];
        let header = Paragraph::new(location_text);
        frame.render_widget(header, chunks[0]);

        // Main content area
        let content_area = chunks[1];

        if state.is_loading {
            render_loading(frame, content_area, state.tick_count);
        } else if let Some(ref error) = state.error {
            render_error(frame, content_area, error);
        } else if let Some(ref weather) = state.weather {
            render_weather(frame, content_area, weather, state);
        } else {
            render_empty(frame, content_area);
        }

        // Help bar
        let help = Line::from(vec![
            Span::styled(" r", Style::default().fg(Color::Cyan).bold()),
            Span::styled(" refresh  ", Style::default().fg(Color::DarkGray)),
            Span::styled("u", Style::default().fg(Color::Cyan).bold()),
            Span::styled(" units  ", Style::default().fg(Color::DarkGray)),
            Span::styled("q", Style::default().fg(Color::Cyan).bold()),
            Span::styled(" quit ", Style::default().fg(Color::DarkGray)),
        ])
        .centered();
        frame.render_widget(Paragraph::new(help), chunks[3]);
    }
}

fn render_loading(frame: &mut Frame, area: Rect, tick: u32) {
    let spinners = ["◐", "◓", "◑", "◒"];
    let spinner = spinners[(tick as usize / 2) % spinners.len()];

    let dots = ".".repeat((tick as usize / 3) % 4);

    let loading = Text::from(vec![
        Line::from(""),
        Line::from(""),
        Line::from(""),
        Line::from(vec![
            Span::styled(spinner, Style::default().fg(Color::Cyan)),
            Span::styled(
                format!(" Fetching weather{:<3}", dots),
                Style::default().fg(Color::Gray),
            ),
        ])
        .centered(),
    ]);

    frame.render_widget(Paragraph::new(loading), area);
}

fn render_error(frame: &mut Frame, area: Rect, error: &str) {
    let error_art = vec![
        Line::from(""),
        Line::from(vec![Span::styled("  ⚠️  ", Style::default())]).centered(),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Error",
            Style::default().fg(Color::Red).bold(),
        )])
        .centered(),
        Line::from(""),
        Line::from(vec![Span::styled(
            error,
            Style::default().fg(Color::Rgb(200, 100, 100)),
        )])
        .centered(),
        Line::from(""),
        Line::from(vec![
            Span::styled("Press ", Style::default().fg(Color::DarkGray)),
            Span::styled("r", Style::default().fg(Color::Cyan).bold()),
            Span::styled(" to retry", Style::default().fg(Color::DarkGray)),
        ])
        .centered(),
    ];

    frame.render_widget(Paragraph::new(error_art), area);
}

fn render_empty(frame: &mut Frame, area: Rect) {
    let empty = Text::from(vec![
        Line::from(""),
        Line::from(""),
        Line::from(""),
        Line::from(vec![
            Span::styled("Press ", Style::default().fg(Color::DarkGray)),
            Span::styled("r", Style::default().fg(Color::Cyan).bold()),
            Span::styled(" to fetch weather", Style::default().fg(Color::DarkGray)),
        ])
        .centered(),
    ]);

    frame.render_widget(Paragraph::new(empty), area);
}

fn render_weather(
    frame: &mut Frame,
    area: Rect,
    weather: &crate::state::WeatherData,
    state: &AppState,
) {
    // Split into art area and info area
    let chunks = Layout::horizontal([
        Constraint::Min(30),    // Weather art
        Constraint::Length(20), // Temperature & info
    ])
    .flex(Flex::Center)
    .split(area);

    // Weather art with auto-sizing based on terminal dimensions
    let (art_lines, _) = sprites::weather_sprite(weather.weather_code, state.terminal_size);
    let art = Paragraph::new(art_lines).alignment(Alignment::Center);
    frame.render_widget(art, chunks[0]);

    // Temperature and description
    let temp = state.unit.format(weather.temperature);
    let temp_color = temp_to_color(weather.temperature);

    let info = Text::from(vec![
        Line::from(""),
        Line::from(""),
        Line::from(vec![Span::styled(
            &temp,
            Style::default().fg(temp_color).bold(),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            &weather.description,
            Style::default().fg(Color::Gray),
        )]),
    ]);

    frame.render_widget(Paragraph::new(info).alignment(Alignment::Left), chunks[1]);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::WeatherData;
    use tui_dispatch::testing::*;

    #[test]
    fn test_handle_event_refresh() {
        let mut component = WeatherDisplay;
        let state = AppState::default();
        let props = WeatherDisplayProps {
            state: &state,
            is_focused: true,
        };

        let actions = component.handle_event(&EventKind::Key(key("r")), props);
        actions.assert_count(1);
        actions.assert_first(Action::WeatherFetch);
    }

    #[test]
    fn test_handle_event_quit() {
        let mut component = WeatherDisplay;
        let state = AppState::default();
        let props = WeatherDisplayProps {
            state: &state,
            is_focused: true,
        };

        let actions = component.handle_event(&EventKind::Key(key("q")), props);
        actions.assert_first(Action::Quit);
    }

    #[test]
    fn test_handle_event_unfocused_ignores() {
        let mut component = WeatherDisplay;
        let state = AppState::default();
        let props = WeatherDisplayProps {
            state: &state,
            is_focused: false,
        };

        let actions = component.handle_event(&EventKind::Key(key("r")), props);
        actions.assert_empty();
    }

    #[test]
    fn test_render_loading() {
        let mut render = RenderHarness::new(60, 24);
        let mut component = WeatherDisplay;

        let state = AppState {
            is_loading: true,
            ..Default::default()
        };

        let output = render.render_to_string_plain(|frame| {
            let props = WeatherDisplayProps {
                state: &state,
                is_focused: true,
            };
            component.render(frame, frame.area(), props);
        });

        assert!(output.contains("Fetching weather"));
    }

    #[test]
    fn test_render_weather() {
        let mut render = RenderHarness::new(60, 24);
        let mut component = WeatherDisplay;

        let state = AppState {
            weather: Some(WeatherData {
                temperature: 22.5,
                weather_code: 0,
                description: "Clear sky".into(),
            }),
            ..Default::default()
        };

        let output = render.render_to_string_plain(|frame| {
            let props = WeatherDisplayProps {
                state: &state,
                is_focused: true,
            };
            component.render(frame, frame.area(), props);
        });

        assert!(output.contains("22.5°C"));
        assert!(output.contains("Clear sky"));
    }
}
