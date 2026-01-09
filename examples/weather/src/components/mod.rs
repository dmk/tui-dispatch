pub mod help_bar;
pub mod location_header;
pub mod weather_body;
pub mod weather_display;

use ratatui::{Frame, layout::Rect};
use tui_dispatch::EventKind;

use crate::action::Action;

/// Base trait for UI components in this example.
pub trait Component {
    type Props<'a>;

    fn handle_event(&mut self, _event: &EventKind, _props: Self::Props<'_>) -> Vec<Action> {
        vec![]
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>);
}

pub use help_bar::{HelpBar, HelpBarProps};
pub use location_header::{LocationHeader, LocationHeaderProps};
pub use weather_body::{WeatherBody, WeatherBodyProps};
pub use weather_display::{
    ERROR_ICON, LOCATION_ICON, SPINNERS, WeatherDisplay, WeatherDisplayProps,
};
