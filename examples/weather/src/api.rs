//! Open-Meteo API client
//!
//! FRAMEWORK PATTERN: Async Side Effects
//! - Intent action triggers async task spawn
//! - Async task sends Result action back via channel
//! - No async in reducer or component - side effects are isolated

use serde::Deserialize;
use tokio::sync::mpsc;

use crate::action::Action;
use crate::state::{Location, WeatherData};

// ============================================================================
// Geocoding API
// ============================================================================

/// Geocoding API response from Open-Meteo
#[derive(Debug, Deserialize)]
struct GeocodingResponse {
    results: Option<Vec<GeocodingResult>>,
}

#[derive(Debug, Deserialize)]
struct GeocodingResult {
    name: String,
    latitude: f64,
    longitude: f64,
    country: Option<String>,
}

/// Geocoding error type
#[derive(Debug)]
pub enum GeocodingError {
    Request(reqwest::Error),
    NotFound(String),
}

impl std::fmt::Display for GeocodingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GeocodingError::Request(e) => write!(f, "Geocoding request failed: {}", e),
            GeocodingError::NotFound(city) => write!(f, "City not found: {}", city),
        }
    }
}

impl std::error::Error for GeocodingError {}

/// Resolve city name to coordinates using Open-Meteo Geocoding API
pub async fn geocode_city(city: &str) -> Result<Location, GeocodingError> {
    let url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=en",
        urlencoding::encode(city)
    );

    let response = reqwest::get(&url).await.map_err(GeocodingError::Request)?;

    let data: GeocodingResponse = response.json().await.map_err(GeocodingError::Request)?;

    data.results
        .and_then(|results| results.into_iter().next())
        .map(|r| {
            // Build display name with country context
            let display_name = match &r.country {
                Some(country) => format!("{}, {}", r.name, country),
                None => r.name,
            };
            Location {
                name: display_name,
                lat: r.latitude,
                lon: r.longitude,
            }
        })
        .ok_or_else(|| GeocodingError::NotFound(city.to_string()))
}

// ============================================================================
// Weather API
// ============================================================================

/// API response from Open-Meteo
#[derive(Debug, Deserialize)]
struct WeatherResponse {
    current_weather: CurrentWeather,
}

#[derive(Debug, Deserialize)]
struct CurrentWeather {
    temperature: f32,
    weathercode: u8,
}

/// Fetch weather from Open-Meteo API
///
/// # Arguments
/// * `lat`, `lon` - Coordinates
/// * `action_tx` - Channel to send result action
///
/// # Pattern
/// This function is spawned as an async task when `WeatherFetch` is dispatched.
/// It sends `WeatherDidLoad` or `WeatherDidError` back through the action channel.
pub async fn fetch_weather(lat: f64, lon: f64, action_tx: mpsc::UnboundedSender<Action>) {
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current_weather=true",
        lat, lon
    );

    let result = async {
        let response = reqwest::get(&url).await?;
        let data: WeatherResponse = response.json().await?;
        Ok::<_, reqwest::Error>(data)
    }
    .await;

    let action = match result {
        Ok(data) => Action::WeatherDidLoad(WeatherData {
            temperature: data.current_weather.temperature,
            weather_code: data.current_weather.weathercode,
            description: weather_description(data.current_weather.weathercode),
        }),
        Err(e) => Action::WeatherDidError(e.to_string()),
    };

    // Send result action - ignore error if receiver dropped
    let _ = action_tx.send(action);
}

/// Convert WMO weather code to human-readable description
fn weather_description(code: u8) -> String {
    match code {
        0 => "Clear sky",
        1 => "Mainly clear",
        2 => "Partly cloudy",
        3 => "Overcast",
        45 | 48 => "Fog",
        51 | 53 | 55 => "Drizzle",
        56 | 57 => "Freezing drizzle",
        61 | 63 | 65 => "Rain",
        66 | 67 => "Freezing rain",
        71 | 73 | 75 => "Snow",
        77 => "Snow grains",
        80..=82 => "Rain showers",
        85 | 86 => "Snow showers",
        95 => "Thunderstorm",
        96 | 99 => "Thunderstorm with hail",
        _ => "Unknown",
    }
    .to_string()
}
