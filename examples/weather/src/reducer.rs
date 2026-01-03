//! Reducer - pure function: (state, action) -> DispatchResult
//!
//! FRAMEWORK PATTERN: Effect Reducer
//! - fn(state: &mut S, action: A) -> DispatchResult<E>
//! - Returns changed flag and any effects to execute
//! - All state mutations happen here
//! - Effects are declarative - executed by main loop

use tui_dispatch::DispatchResult;

use crate::action::Action;
use crate::effect::Effect;
use crate::state::AppState;

/// The reducer handles all state transitions
///
/// # Returns
/// `DispatchResult` with changed flag and any effects to execute
pub fn reducer(state: &mut AppState, action: Action) -> DispatchResult<Effect> {
    match action {
        // ===== Weather actions =====
        Action::WeatherFetch => {
            // Clear previous error, set loading, emit fetch effect
            state.is_loading = true;
            state.error = None;
            let loc = state.current_location();
            DispatchResult::changed_with(Effect::FetchWeather {
                lat: loc.lat,
                lon: loc.lon,
            })
        }

        Action::WeatherDidLoad(data) => {
            state.weather = Some(data);
            state.is_loading = false;
            state.error = None;
            DispatchResult::changed()
        }

        Action::WeatherDidError(msg) => {
            state.is_loading = false;
            state.error = Some(msg);
            DispatchResult::changed()
        }

        // ===== UI actions =====
        Action::UiToggleUnits => {
            state.unit = state.unit.toggle();
            DispatchResult::changed()
        }

        Action::UiTerminalResize(width, height) => {
            if state.terminal_size != (width, height) {
                state.terminal_size = (width, height);
                DispatchResult::changed()
            } else {
                DispatchResult::unchanged()
            }
        }

        // ===== Global actions =====
        Action::Tick => {
            state.tick_count = state.tick_count.wrapping_add(1);
            if state.is_loading {
                DispatchResult::changed()
            } else {
                DispatchResult::unchanged()
            }
        }

        Action::Quit => {
            // Quit is handled in main loop, not here
            DispatchResult::unchanged()
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

        let result = reducer(&mut state, Action::WeatherFetch);

        assert!(result.changed);
        assert!(state.is_loading);
        assert_eq!(result.effects.len(), 1);
        assert!(matches!(result.effects[0], Effect::FetchWeather { .. }));
    }

    #[test]
    fn test_weather_did_load_clears_loading() {
        let mut state = AppState {
            is_loading: true,
            ..Default::default()
        };

        let weather = WeatherData {
            temperature: 22.5,
            weather_code: 0,
            description: "Clear".into(),
        };

        let result = reducer(&mut state, Action::WeatherDidLoad(weather.clone()));

        assert!(result.changed);
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
        let result = reducer(&mut state, Action::UiTerminalResize(100, 40));
        assert!(result.changed);
        assert_eq!(state.terminal_size, (100, 40));

        // Same size should not trigger re-render
        let result = reducer(&mut state, Action::UiTerminalResize(100, 40));
        assert!(!result.changed);
    }

    #[test]
    fn test_tick_only_rerenders_when_loading() {
        let mut state = AppState::default();

        // Not loading - no re-render
        let result = reducer(&mut state, Action::Tick);
        assert!(!result.changed);

        // Loading - should re-render
        state.is_loading = true;
        let result = reducer(&mut state, Action::Tick);
        assert!(result.changed);
    }
}
