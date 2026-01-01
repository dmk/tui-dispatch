//! Actions demonstrating category inference and async patterns
//!
//! FRAMEWORK PATTERN: Action naming convention
//! - Prefix determines category: WeatherFetch, WeatherDidLoad -> "weather" category
//! - "Did" prefix indicates async result
//! - Verbs at end: Fetch, Load, Clear, Toggle, Quit

use crate::state::WeatherData;
use tui_dispatch::ActionSummary;

// Import the derive macro, not the trait
// The trait is `tui_dispatch::Action`, the macro is `tui_dispatch::Action` (from tui_dispatch_macros)
// Since they have the same name, we use the macro directly via the derive attribute

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

/// Custom summary implementation for action logging
/// Shows concise info instead of full Debug output for data-heavy actions
impl ActionSummary for Action {
    fn summary(&self) -> String {
        match self {
            // Show temperature instead of full weather data
            Action::WeatherDidLoad(data) => {
                format!(
                    "WeatherDidLoad {{ temp: {:.1}°C, code: {} }}",
                    data.temperature, data.weather_code
                )
            }
            // Truncate long error messages
            Action::WeatherDidError(e) => {
                let msg = if e.len() > 40 {
                    format!("{}...", &e.chars().take(37).collect::<String>())
                } else {
                    e.clone()
                };
                format!("WeatherDidError({:?})", msg)
            }
            // Use default Debug for simple actions
            _ => format!("{:?}", self),
        }
    }
}

// The derive macro generates:
// - impl Action for Action { fn name(&self) -> &'static str }
// - impl ActionCategory for Action { fn category(&self) -> Option<&'static str> }
// - enum ActionCategory { Weather, Ui, AsyncResult, Uncategorized }
// - is_weather(), is_ui(), is_async_result() predicates
