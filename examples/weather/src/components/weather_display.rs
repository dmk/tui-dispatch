//! Weather display component with Unicode art
//!
//! FRAMEWORK PATTERN: Component Trait
//! - Props<'a>: Read-only data for rendering (borrowed from state)
//! - handle_event: Receives EventKind, returns `impl IntoIterator<Item = Action>`
//! - render: Pure function of props - no side effects
//! - Focus handled via props, not event context

use crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Layout};
use ratatui::prelude::{Frame, Rect};
use tui_dispatch::EventKind;

use super::{Component, HelpBar, HelpBarProps, WeatherBody, WeatherBodyProps};
use crate::action::Action;
use crate::state::AppState;

pub const ERROR_ICON: &str = "⚠️";
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

    fn handle_event(
        &mut self,
        event: &EventKind,
        props: Self::Props<'_>,
    ) -> impl IntoIterator<Item = Action> {
        if !props.is_focused {
            return None;
        }

        match event {
            EventKind::Key(key) => match key.code {
                KeyCode::Char('r') | KeyCode::F(5) => Some(Action::WeatherFetch),
                KeyCode::Char('/') => Some(Action::SearchOpen),
                KeyCode::Char('u') => Some(Action::UiToggleUnits),
                KeyCode::Char('q') | KeyCode::Esc => Some(Action::Quit),
                _ => None,
            },
            _ => None,
        }
    }

    /// Render the component to the frame
    fn render(&mut self, frame: &mut Frame, area: Rect, props: WeatherDisplayProps<'_>) {
        // Layout: main content area + help bar at bottom
        let chunks = Layout::vertical([
            Constraint::Min(1),    // Main content (centered by WeatherBody)
            Constraint::Length(1), // Help bar
        ])
        .split(area);

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

        let actions: Vec<_> = component
            .handle_event(&EventKind::Key(key("r")), props)
            .into_iter()
            .collect();
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

        let actions: Vec<_> = component
            .handle_event(&EventKind::Key(key("q")), props)
            .into_iter()
            .collect();
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

        let actions: Vec<_> = component
            .handle_event(&EventKind::Key(key("r")), props)
            .into_iter()
            .collect();
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

        // Loading is now indicated by animated gradient on city name
        // Just verify the component renders without panicking
        assert!(!output.is_empty());
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

        // Temperature is now rendered as FIGlet ASCII art, so check for description instead
        assert!(output.contains("Clear sky"));
    }
}
