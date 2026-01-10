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

/// Animation timing for the header gradient seam.
pub const LOADING_ANIM_TICK_MS: u64 = 15;
pub const LOADING_ANIM_CYCLE_TICKS: u32 = 60;

/// Application state - everything the UI needs to render
#[derive(Clone, Debug, tui_dispatch::DebugState)]
pub struct AppState {
    /// Current weather data (None = not yet fetched)
    #[debug(skip)]
    pub weather: Option<WeatherData>,

    /// Loading state for async operations
    pub is_loading: bool,

    /// Error message (if last fetch failed)
    #[debug(skip)]
    pub error: Option<String>,

    /// Single location (from geocoding)
    #[debug(skip)]
    pub location: Location,

    /// Temperature unit preference
    #[debug(skip)]
    pub unit: TempUnit,

    /// Animation frame counter (for gradient seam)
    pub tick_count: u32,

    /// Remaining ticks to finish the current animation cycle after loading
    pub loading_anim_ticks_remaining: u32,

    /// Terminal dimensions (for sprite sizing)
    #[debug(skip)]
    pub terminal_size: (u16, u16),

    // --- Search mode ---
    /// Whether search overlay is open
    pub search_mode: bool,

    /// Current search query
    #[debug(skip)]
    pub search_query: String,

    /// Search results from geocoding API
    #[debug(skip)]
    pub search_results: Vec<Location>,

    /// Search error message
    #[debug(skip)]
    pub search_error: Option<String>,

    /// Selected index in search results
    pub search_selected: usize,
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
            loading_anim_ticks_remaining: 0,
            terminal_size: (80, 24), // Default, updated on resize
            search_mode: false,
            search_query: String::new(),
            search_results: Vec::new(),
            search_error: None,
            search_selected: 0,
        }
    }

    /// Get current location
    pub fn current_location(&self) -> &Location {
        &self.location
    }

    pub fn loading_anim_active(&self) -> bool {
        self.is_loading || self.loading_anim_ticks_remaining > 0
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
