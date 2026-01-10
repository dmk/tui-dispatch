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
    layout::{Alignment, Constraint, Layout},
    style::{Color, Style, Stylize},
    widgets::{Block, Borders},
};
use tui_dispatch::EventKind;

use super::{Component, HelpBar, HelpBarProps, WeatherBody, WeatherBodyProps};
use crate::action::Action;
use crate::state::AppState;

pub const LOCATION_ICON: &str = "📍 ";
pub const ERROR_ICON: &str = "⚠️";
pub const SPINNERS: [&str; 4] = ["◐", "◓", "◑", "◒"];
/// Props for WeatherDisplay - read-only view of state
pub struct WeatherDisplayProps<'a> {
    pub state: &'a AppState,
    pub is_focused: bool,
}

/// The main weather display component
#[derive(Default)]
pub struct WeatherDisplay;

impl Component<Action> for WeatherDisplay {
    type Props<'a> = WeatherDisplayProps<'a>;

    /// Handle an event and return actions to dispatch
    fn handle_event(&mut self, event: &EventKind, props: WeatherDisplayProps<'_>) -> Vec<Action> {
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
    fn render(&mut self, frame: &mut Frame, area: Rect, props: WeatherDisplayProps<'_>) {
        let state = props.state;

        // Loading indicator for title
        let loading_indicator = if state.is_loading {
            let spinner = SPINNERS[(state.tick_count as usize / 2) % SPINNERS.len()];
            format!(" {} ", spinner)
        } else {
            String::new()
        };

        // Main container with nice border
        let outer_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(80, 80, 100)))
            .title(format!(" ☁ Weather{}", loading_indicator))
            .title_style(Style::default().fg(Color::Cyan).bold())
            .title_alignment(Alignment::Center);

        frame.render_widget(outer_block.clone(), area);
        let inner = outer_block.inner(area);

        // Layout: main content area + help bar at bottom
        let chunks = Layout::vertical([
            Constraint::Min(1),    // Main content (centered by WeatherBody)
            Constraint::Length(1), // Help bar
        ])
        .split(inner);

        let mut body = WeatherBody;
        body.render(frame, chunks[0], WeatherBodyProps { state: props.state });

        let mut help = HelpBar;
        help.render(frame, chunks[1], HelpBarProps);
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
