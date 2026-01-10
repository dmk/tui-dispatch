pub mod help_bar;
pub mod location_header;
pub mod weather_body;
pub mod weather_display;

// Re-export core Component trait
pub use tui_dispatch::Component;

pub use help_bar::{HelpBar, HelpBarProps};
pub use location_header::{LocationHeader, LocationHeaderProps};
pub use weather_body::{WeatherBody, WeatherBodyProps};
pub use weather_display::{
    ERROR_ICON, LOCATION_ICON, SPINNERS, WeatherDisplay, WeatherDisplayProps,
};
