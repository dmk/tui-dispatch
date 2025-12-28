//! Weather sprite system with auto-sizing based on terminal dimensions
//!
//! Sprites are loaded from text files at compile time using `include_str!`.
//! Each weather condition has Small, Medium, and Large variants.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};

// ============================================================================
// Sprite data - embedded at compile time
// ============================================================================

mod sprite_data {
    pub mod sun {
        pub const SMALL: &str = include_str!("../sprites/sun/small.txt");
        pub const MEDIUM: &str = include_str!("../sprites/sun/medium.txt");
        pub const LARGE: &str = include_str!("../sprites/sun/large.txt");
    }
    pub mod partly_cloudy {
        pub const SMALL: &str = include_str!("../sprites/partly_cloudy/small.txt");
        pub const MEDIUM: &str = include_str!("../sprites/partly_cloudy/medium.txt");
        pub const LARGE: &str = include_str!("../sprites/partly_cloudy/large.txt");
    }
    pub mod cloudy {
        pub const SMALL: &str = include_str!("../sprites/cloudy/small.txt");
        pub const MEDIUM: &str = include_str!("../sprites/cloudy/medium.txt");
        pub const LARGE: &str = include_str!("../sprites/cloudy/large.txt");
    }
    pub mod fog {
        pub const SMALL: &str = include_str!("../sprites/fog/small.txt");
        pub const MEDIUM: &str = include_str!("../sprites/fog/medium.txt");
        pub const LARGE: &str = include_str!("../sprites/fog/large.txt");
    }
    pub mod drizzle {
        pub const SMALL: &str = include_str!("../sprites/drizzle/small.txt");
        pub const MEDIUM: &str = include_str!("../sprites/drizzle/medium.txt");
        pub const LARGE: &str = include_str!("../sprites/drizzle/large.txt");
    }
    pub mod rain {
        pub const SMALL: &str = include_str!("../sprites/rain/small.txt");
        pub const MEDIUM: &str = include_str!("../sprites/rain/medium.txt");
        pub const LARGE: &str = include_str!("../sprites/rain/large.txt");
    }
    pub mod snow {
        pub const SMALL: &str = include_str!("../sprites/snow/small.txt");
        pub const MEDIUM: &str = include_str!("../sprites/snow/medium.txt");
        pub const LARGE: &str = include_str!("../sprites/snow/large.txt");
    }
    pub mod thunderstorm {
        pub const SMALL: &str = include_str!("../sprites/thunderstorm/small.txt");
        pub const MEDIUM: &str = include_str!("../sprites/thunderstorm/medium.txt");
        pub const LARGE: &str = include_str!("../sprites/thunderstorm/large.txt");
    }
}

// ============================================================================
// Types
// ============================================================================

/// Sprite size categories
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpriteSize {
    /// 5 lines - for compact terminals (height < 16)
    Small,
    /// 7 lines - for normal terminals (height 16-28)
    Medium,
    /// 11 lines - for large terminals (height > 28)
    Large,
}

impl SpriteSize {
    /// Determine appropriate sprite size based on terminal dimensions
    pub fn from_terminal_size(_width: u16, height: u16) -> Self {
        // Account for UI chrome: border (2) + header (3) + spacer (1) + help (1) = 7
        let content_height = height.saturating_sub(7);

        match content_height {
            0..=12 => SpriteSize::Small,
            13..=20 => SpriteSize::Medium,
            _ => SpriteSize::Large,
        }
    }
}

/// Weather condition categories
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeatherCondition {
    ClearSky,
    PartlyCloudy,
    Cloudy,
    Fog,
    Drizzle,
    Rain,
    Snow,
    Thunderstorm,
    Unknown,
}

impl WeatherCondition {
    /// Map WMO weather code to condition
    pub fn from_code(code: u8) -> Self {
        match code {
            0 => WeatherCondition::ClearSky,
            1..=2 => WeatherCondition::PartlyCloudy,
            3 => WeatherCondition::Cloudy,
            45 | 48 => WeatherCondition::Fog,
            51..=57 => WeatherCondition::Drizzle,
            61..=67 | 80..=82 => WeatherCondition::Rain,
            71..=77 | 85..=86 => WeatherCondition::Snow,
            95..=99 => WeatherCondition::Thunderstorm,
            _ => WeatherCondition::Unknown,
        }
    }

    /// Get the primary color for this weather condition
    fn color(&self) -> Color {
        match self {
            WeatherCondition::ClearSky => Color::Yellow,
            WeatherCondition::PartlyCloudy => Color::Rgb(200, 200, 100),
            WeatherCondition::Cloudy => Color::Rgb(160, 160, 175),
            WeatherCondition::Fog => Color::Rgb(150, 150, 160),
            WeatherCondition::Drizzle => Color::Rgb(130, 170, 200),
            WeatherCondition::Rain => Color::Rgb(80, 140, 200),
            WeatherCondition::Snow => Color::Rgb(200, 220, 255),
            WeatherCondition::Thunderstorm => Color::Rgb(180, 180, 50),
            WeatherCondition::Unknown => Color::Rgb(150, 150, 165),
        }
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Get sprite from weather code and terminal size
pub fn weather_sprite(code: u8, terminal_size: (u16, u16)) -> (Text<'static>, Color) {
    let condition = WeatherCondition::from_code(code);
    let size = SpriteSize::from_terminal_size(terminal_size.0, terminal_size.1);
    get_sprite(condition, size)
}

/// Get weather art for the given condition and size
pub fn get_sprite(condition: WeatherCondition, size: SpriteSize) -> (Text<'static>, Color) {
    let sprite_str = match condition {
        WeatherCondition::ClearSky => match size {
            SpriteSize::Small => sprite_data::sun::SMALL,
            SpriteSize::Medium => sprite_data::sun::MEDIUM,
            SpriteSize::Large => sprite_data::sun::LARGE,
        },
        WeatherCondition::PartlyCloudy => match size {
            SpriteSize::Small => sprite_data::partly_cloudy::SMALL,
            SpriteSize::Medium => sprite_data::partly_cloudy::MEDIUM,
            SpriteSize::Large => sprite_data::partly_cloudy::LARGE,
        },
        WeatherCondition::Cloudy | WeatherCondition::Unknown => match size {
            SpriteSize::Small => sprite_data::cloudy::SMALL,
            SpriteSize::Medium => sprite_data::cloudy::MEDIUM,
            SpriteSize::Large => sprite_data::cloudy::LARGE,
        },
        WeatherCondition::Fog => match size {
            SpriteSize::Small => sprite_data::fog::SMALL,
            SpriteSize::Medium => sprite_data::fog::MEDIUM,
            SpriteSize::Large => sprite_data::fog::LARGE,
        },
        WeatherCondition::Drizzle => match size {
            SpriteSize::Small => sprite_data::drizzle::SMALL,
            SpriteSize::Medium => sprite_data::drizzle::MEDIUM,
            SpriteSize::Large => sprite_data::drizzle::LARGE,
        },
        WeatherCondition::Rain => match size {
            SpriteSize::Small => sprite_data::rain::SMALL,
            SpriteSize::Medium => sprite_data::rain::MEDIUM,
            SpriteSize::Large => sprite_data::rain::LARGE,
        },
        WeatherCondition::Snow => match size {
            SpriteSize::Small => sprite_data::snow::SMALL,
            SpriteSize::Medium => sprite_data::snow::MEDIUM,
            SpriteSize::Large => sprite_data::snow::LARGE,
        },
        WeatherCondition::Thunderstorm => match size {
            SpriteSize::Small => sprite_data::thunderstorm::SMALL,
            SpriteSize::Medium => sprite_data::thunderstorm::MEDIUM,
            SpriteSize::Large => sprite_data::thunderstorm::LARGE,
        },
    };

    let color = condition.color();
    let text = sprite_to_text(sprite_str, color);
    (text, color)
}

/// Convert sprite string to colored Text
fn sprite_to_text(sprite: &'static str, color: Color) -> Text<'static> {
    let style = Style::default().fg(color);
    let lines: Vec<Line> = sprite
        .lines()
        .map(|line| Line::from(Span::styled(line, style)))
        .collect();
    Text::from(lines)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sprite_size_from_terminal() {
        // Small terminals
        assert_eq!(SpriteSize::from_terminal_size(80, 10), SpriteSize::Small);
        assert_eq!(SpriteSize::from_terminal_size(80, 19), SpriteSize::Small);

        // Medium terminals
        assert_eq!(SpriteSize::from_terminal_size(80, 20), SpriteSize::Medium);
        assert_eq!(SpriteSize::from_terminal_size(80, 27), SpriteSize::Medium);

        // Large terminals
        assert_eq!(SpriteSize::from_terminal_size(80, 28), SpriteSize::Large);
        assert_eq!(SpriteSize::from_terminal_size(80, 50), SpriteSize::Large);
    }

    #[test]
    fn test_weather_condition_from_code() {
        assert_eq!(WeatherCondition::from_code(0), WeatherCondition::ClearSky);
        assert_eq!(
            WeatherCondition::from_code(1),
            WeatherCondition::PartlyCloudy
        );
        assert_eq!(WeatherCondition::from_code(3), WeatherCondition::Cloudy);
        assert_eq!(WeatherCondition::from_code(45), WeatherCondition::Fog);
        assert_eq!(WeatherCondition::from_code(61), WeatherCondition::Rain);
        assert_eq!(WeatherCondition::from_code(71), WeatherCondition::Snow);
        assert_eq!(
            WeatherCondition::from_code(95),
            WeatherCondition::Thunderstorm
        );
        assert_eq!(WeatherCondition::from_code(100), WeatherCondition::Unknown);
    }

    #[test]
    fn test_weather_sprite_returns_text() {
        let (text, _color) = weather_sprite(0, (80, 24));
        assert!(!text.lines.is_empty());
    }

    #[test]
    fn test_all_sprites_load() {
        // Verify all sprites are embedded correctly
        for condition in [
            WeatherCondition::ClearSky,
            WeatherCondition::PartlyCloudy,
            WeatherCondition::Cloudy,
            WeatherCondition::Fog,
            WeatherCondition::Drizzle,
            WeatherCondition::Rain,
            WeatherCondition::Snow,
            WeatherCondition::Thunderstorm,
        ] {
            for size in [SpriteSize::Small, SpriteSize::Medium, SpriteSize::Large] {
                let (text, _) = get_sprite(condition, size);
                assert!(
                    !text.lines.is_empty(),
                    "Sprite {:?}/{:?} should not be empty",
                    condition,
                    size
                );
            }
        }
    }
}
