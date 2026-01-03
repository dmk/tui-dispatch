//! Actions demonstrating category inference and async patterns
//!
//! FRAMEWORK PATTERN: Action naming convention
//! - Prefix determines category: WeatherFetch, WeatherDidLoad -> "weather" category
//! - "Did" prefix indicates async result
//! - Verbs at end: Fetch, Load, Clear, Toggle, Quit

use crate::state::WeatherData;

/// Application actions with automatic category inference
///
/// # Categories (auto-inferred from naming):
/// - `weather`: WeatherFetch, WeatherDidLoad, WeatherDidError
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
