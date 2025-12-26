//! Reducer - pure function: (state, action) -> state
//!
//! FRAMEWORK PATTERN: Reducer
//! - fn(state: &mut S, action: A) -> bool
//! - Returns true if state changed (triggers re-render)
//! - All state mutations happen here
//! - No side effects - just data transformation

use crate::action::Action;
use crate::state::AppState;

/// The reducer handles all state transitions
///
/// # Returns
/// `true` if state changed and UI should re-render
pub fn reducer(state: &mut AppState, action: Action) -> bool {
    match action {
        // ===== Weather actions =====
        Action::WeatherFetch => {
            // Clear previous error, set loading
            state.is_loading = true;
            state.error = None;
            true // re-render to show loading state
        }

        Action::WeatherDidLoad(data) => {
            state.weather = Some(data);
            state.is_loading = false;
            state.error = None;
            true // re-render with new weather
        }

        Action::WeatherDidError(msg) => {
            state.is_loading = false;
            state.error = Some(msg);
            true // re-render to show error
        }

        // ===== UI actions =====
        Action::UiToggleUnits => {
            state.unit = state.unit.toggle();
            true // re-render with new unit
        }

        Action::UiTerminalResize(width, height) => {
            if state.terminal_size != (width, height) {
                state.terminal_size = (width, height);
                true // re-render with new sprite size
            } else {
                false
            }
        }

        // ===== Global actions =====
        Action::Tick => {
            state.tick_count = state.tick_count.wrapping_add(1);
            state.is_loading // only re-render if loading (for spinner animation)
        }

        Action::Quit => {
            // Quit is handled in main loop, not here
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::WeatherData;

    #[test]
    fn test_weather_fetch_sets_loading() {
        let mut state = AppState::default();
        assert!(!state.is_loading);

        let changed = reducer(&mut state, Action::WeatherFetch);

        assert!(changed);
        assert!(state.is_loading);
    }

    #[test]
    fn test_weather_did_load_clears_loading() {
        let mut state = AppState::default();
        state.is_loading = true;

        let weather = WeatherData {
            temperature: 22.5,
            weather_code: 0,
            description: "Clear".into(),
        };

        let changed = reducer(&mut state, Action::WeatherDidLoad(weather.clone()));

        assert!(changed);
        assert!(!state.is_loading);
        assert_eq!(state.weather, Some(weather));
    }

    #[test]
    fn test_toggle_units() {
        let mut state = AppState::default();
        assert_eq!(state.unit, crate::state::TempUnit::Celsius);

        reducer(&mut state, Action::UiToggleUnits);
        assert_eq!(state.unit, crate::state::TempUnit::Fahrenheit);

        reducer(&mut state, Action::UiToggleUnits);
        assert_eq!(state.unit, crate::state::TempUnit::Celsius);
    }

    #[test]
    fn test_terminal_resize() {
        let mut state = AppState::default();
        assert_eq!(state.terminal_size, (80, 24));

        // Resize should trigger re-render
        let changed = reducer(&mut state, Action::UiTerminalResize(100, 40));
        assert!(changed);
        assert_eq!(state.terminal_size, (100, 40));

        // Same size should not trigger re-render
        let changed = reducer(&mut state, Action::UiTerminalResize(100, 40));
        assert!(!changed);
    }

    #[test]
    fn test_tick_only_rerenders_when_loading() {
        let mut state = AppState::default();

        // Not loading - no re-render
        let changed = reducer(&mut state, Action::Tick);
        assert!(!changed);

        // Loading - should re-render
        state.is_loading = true;
        let changed = reducer(&mut state, Action::Tick);
        assert!(changed);
    }
}
