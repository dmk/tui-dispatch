//! Render snapshot tests using RenderHarness
//!
//! FRAMEWORK PATTERN: RenderHarness
//! - Create harness with terminal dimensions
//! - Render component to test buffer
//! - Convert to string for snapshot testing

use tui_dispatch::testing::*;
use weather_example::{
    components::{WeatherDisplay, WeatherDisplayProps},
    state::{AppState, Location, TempUnit, WeatherData},
};

#[test]
fn test_render_loading_state() {
    // PATTERN: RenderHarness for visual testing
    let mut render = RenderHarness::new(60, 24);
    let mut component = WeatherDisplay;

    let state = AppState {
        is_loading: true,
        tick_count: 0,
        ..Default::default()
    };

    let output = render.render_to_string_plain(|frame| {
        let props = WeatherDisplayProps {
            state: &state,
            is_focused: true,
        };
        component.render(frame, frame.area(), props);
    });

    // Should show loading indicator
    assert!(
        output.contains("Fetching weather"),
        "Should show loading text"
    );
}

#[test]
fn test_render_clear_weather() {
    let mut render = RenderHarness::new(50, 20);
    let mut component = WeatherDisplay;

    let state = AppState {
        weather: Some(WeatherData {
            temperature: 22.5,
            weather_code: 0, // Clear sky
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

    assert!(output.contains("Kyiv"), "Should show location");
    assert!(output.contains("22.5°C"), "Should show temperature");
    assert!(output.contains("Clear sky"), "Should show description");
}

#[test]
fn test_render_error_state() {
    let mut render = RenderHarness::new(50, 20);
    let mut component = WeatherDisplay;

    let state = AppState {
        error: Some("Network error".into()),
        ..Default::default()
    };

    let output = render.render_to_string_plain(|frame| {
        let props = WeatherDisplayProps {
            state: &state,
            is_focused: true,
        };
        component.render(frame, frame.area(), props);
    });

    assert!(output.contains("Error"), "Should show error label");
    assert!(
        output.contains("Network error"),
        "Should show error message"
    );
    assert!(output.contains("retry"), "Should show retry hint");
}

#[test]
fn test_render_fahrenheit() {
    let mut render = RenderHarness::new(50, 20);
    let mut component = WeatherDisplay;

    let state = AppState {
        weather: Some(WeatherData {
            temperature: 0.0, // 0°C = 32°F
            weather_code: 0,
            description: "Clear".into(),
        }),
        unit: TempUnit::Fahrenheit,
        ..Default::default()
    };

    let output = render.render_to_string_plain(|frame| {
        let props = WeatherDisplayProps {
            state: &state,
            is_focused: true,
        };
        component.render(frame, frame.area(), props);
    });

    assert!(output.contains("32.0°F"), "Should show Fahrenheit");
}

#[test]
fn test_render_custom_location() {
    let mut render = RenderHarness::new(50, 20);
    let mut component = WeatherDisplay;

    let custom = Location {
        name: "My Beach House".into(),
        lat: 0.0,
        lon: 0.0,
    };
    let state = AppState::new(custom);

    let output = render.render_to_string_plain(|frame| {
        let props = WeatherDisplayProps {
            state: &state,
            is_focused: true,
        };
        component.render(frame, frame.area(), props);
    });

    assert!(
        output.contains("My Beach House"),
        "Should show custom location name"
    );
}

#[test]
fn test_render_help_bar() {
    let mut render = RenderHarness::new(80, 24);
    let mut component = WeatherDisplay;

    let state = AppState::default();

    let output = render.render_to_string_plain(|frame| {
        let props = WeatherDisplayProps {
            state: &state,
            is_focused: true,
        };
        component.render(frame, frame.area(), props);
    });

    // Should show keybinding hints (new format: "r refresh" style)
    assert!(output.contains("refresh"), "Should show refresh hint");
    assert!(output.contains("units"), "Should show units hint");
    assert!(output.contains("quit"), "Should show quit hint");
}

#[test]
fn test_render_initial_state() {
    let mut render = RenderHarness::new(50, 20);
    let mut component = WeatherDisplay;

    let state = AppState::default();

    let output = render.render_to_string_plain(|frame| {
        let props = WeatherDisplayProps {
            state: &state,
            is_focused: true,
        };
        component.render(frame, frame.area(), props);
    });

    // Initial state should prompt user to fetch
    assert!(
        output.contains("to fetch weather"),
        "Should show fetch prompt"
    );
}

#[test]
fn test_render_rain_weather() {
    let mut render = RenderHarness::new(50, 20);
    let mut component = WeatherDisplay;

    let state = AppState {
        weather: Some(WeatherData {
            temperature: 15.0,
            weather_code: 61, // Rain
            description: "Rain".into(),
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

    assert!(output.contains("15.0°C"), "Should show temperature");
    assert!(output.contains("Rain"), "Should show rain description");
}
