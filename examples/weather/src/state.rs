//! Application state - single source of truth
//!
//! FRAMEWORK PATTERN: State is immutable from component perspective
//! - Components receive `&AppState` as props
//! - Only reducer can mutate state
//! - Reducer returns `bool` indicating if re-render needed

/// Weather data from Open-Meteo API
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WeatherData {
    pub temperature: f32,
    pub weather_code: u8, // WMO weather code
    pub description: String,
}

/// A geographic location
#[derive(Clone, Debug, PartialEq)]
pub struct Location {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
}

/// Temperature unit preference
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum TempUnit {
    #[default]
    Celsius,
    Fahrenheit,
}

impl TempUnit {
    pub fn toggle(&self) -> Self {
        match self {
            TempUnit::Celsius => TempUnit::Fahrenheit,
            TempUnit::Fahrenheit => TempUnit::Celsius,
        }
    }

    pub fn format(&self, celsius: f32) -> String {
        match self {
            TempUnit::Celsius => format!("{:.1}°C", celsius),
            TempUnit::Fahrenheit => format!("{:.1}°F", celsius * 9.0 / 5.0 + 32.0),
        }
    }
}

/// Application state - everything the UI needs to render
#[derive(Clone, Debug)]
pub struct AppState {
    /// Current weather data (None = not yet fetched)
    pub weather: Option<WeatherData>,

    /// Loading state for async operations
    pub is_loading: bool,

    /// Error message (if last fetch failed)
    pub error: Option<String>,

    /// Single location (from geocoding)
    pub location: Location,

    /// Temperature unit preference
    pub unit: TempUnit,

    /// Animation frame counter (for loading spinner)
    pub tick_count: u32,

    /// Terminal dimensions (for sprite sizing)
    pub terminal_size: (u16, u16),
}

impl AppState {
    /// Create state with the given location
    pub fn new(location: Location) -> Self {
        Self {
            weather: None,
            is_loading: false,
            error: None,
            location,
            unit: TempUnit::default(),
            tick_count: 0,
            terminal_size: (80, 24), // Default, updated on resize
        }
    }

    /// Get current location
    pub fn current_location(&self) -> &Location {
        &self.location
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new(Location {
            name: "Kyiv, Ukraine".into(),
            lat: 50.4501,
            lon: 30.5234,
        })
    }
}
