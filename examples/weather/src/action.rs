//! Actions demonstrating category inference and async patterns
//!
//! FRAMEWORK PATTERN: Action naming convention
//! - Prefix determines category: WeatherFetch, WeatherDidLoad -> "weather" category
//! - "Did" prefix indicates async result
//! - Verbs at end: Fetch, Load, Clear, Toggle, Quit

use crate::state::{Location, WeatherData};

/// Application actions with automatic category inference
///
/// # Categories (auto-inferred from naming):
/// - `weather`: WeatherFetch, WeatherDidLoad, WeatherDidError
/// - `search`: SearchOpen, SearchClose, SearchQuery*, SearchDidLoad, etc.
/// - `ui`: UiToggleUnits, UiTerminalResize
/// - `uncategorized`: Tick, Quit
#[derive(tui_dispatch::Action, Clone, Debug, PartialEq)]
#[action(infer_categories)]
pub enum Action {
    // ===== Weather category =====
    /// Intent: Request weather data fetch (triggers async task)
    WeatherFetch,

    /// Result: Weather data loaded successfully
    WeatherDidLoad(WeatherData),

    /// Result: Weather fetch failed
    WeatherDidError(String),

    // ===== Search category =====
    /// Open city search overlay
    SearchOpen,

    /// Close search overlay (cancel)
    SearchClose,

    /// Search query text changed
    SearchQueryChange(String),

    /// Submit search query (explicit trigger)
    SearchQuerySubmit(String),

    /// Result: Cities found from geocoding API
    SearchDidLoad(Vec<Location>),

    /// Result: Search failed
    SearchDidError(String),

    /// Select a result in the list (by index)
    SearchSelect(usize),

    /// Confirm selection - switch to selected city
    SearchConfirm,

    // ===== UI category =====
    /// Toggle between Celsius and Fahrenheit
    UiToggleUnits,

    /// Terminal was resized - update sprite sizing
    UiTerminalResize(u16, u16),

    // ===== Uncategorized (global) =====
    /// Periodic tick for loading animation
    Tick,

    /// Exit the application
    Quit,
}

// The derive macro generates:
// - impl Action for Action { fn name(&self) -> &'static str }
// - impl ActionParams for Action { fn params(&self) -> String }
// - impl ActionCategory for Action { fn category(&self) -> Option<&'static str> }
// - enum ActionCategory { Weather, Ui, AsyncResult, Uncategorized }
// - is_weather(), is_ui(), is_async_result() predicates
