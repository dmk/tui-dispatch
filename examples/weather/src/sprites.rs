//! Weather sprite system with auto-sizing and multi-color layer support
//!
//! Sprites are loaded from text files at compile time using `include_str!`.
//! Each weather condition has Small, Medium, and Large variants.
//! Multi-layer sprites (like partly_cloudy) composite multiple colored layers.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};

// ============================================================================
// Sprite data - embedded at compile time
// File naming: {size}_{color}.txt (e.g., small_yellow.txt, medium_gray.txt)
// ============================================================================

mod sprite_data {
    pub mod sun {
        pub const SMALL_YELLOW: &str = include_str!("../sprites/sun/small_yellow.txt");
        pub const MEDIUM_YELLOW: &str = include_str!("../sprites/sun/medium_yellow.txt");
        pub const LARGE_YELLOW: &str = include_str!("../sprites/sun/large_yellow.txt");
    }
    pub mod partly_cloudy {
        // Sun layer (background)
        pub const SMALL_YELLOW: &str = include_str!("../sprites/partly_cloudy/small_yellow.txt");
        pub const MEDIUM_YELLOW: &str = include_str!("../sprites/partly_cloudy/medium_yellow.txt");
        pub const LARGE_YELLOW: &str = include_str!("../sprites/partly_cloudy/large_yellow.txt");
        // Cloud layer (foreground)
        pub const SMALL_GRAY: &str = include_str!("../sprites/partly_cloudy/small_gray.txt");
        pub const MEDIUM_GRAY: &str = include_str!("../sprites/partly_cloudy/medium_gray.txt");
        pub const LARGE_GRAY: &str = include_str!("../sprites/partly_cloudy/large_gray.txt");
    }
    pub mod cloudy {
        // Back cloud (darker, smaller)
        pub const SMALL_DARKGRAY: &str = include_str!("../sprites/cloudy/small_darkgray.txt");
        pub const MEDIUM_DARKGRAY: &str = include_str!("../sprites/cloudy/medium_darkgray.txt");
        pub const LARGE_DARKGRAY: &str = include_str!("../sprites/cloudy/large_darkgray.txt");
        // Front cloud (lighter, larger)
        pub const SMALL_LIGHTGRAY: &str = include_str!("../sprites/cloudy/small_lightgray.txt");
        pub const MEDIUM_LIGHTGRAY: &str = include_str!("../sprites/cloudy/medium_lightgray.txt");
        pub const LARGE_LIGHTGRAY: &str = include_str!("../sprites/cloudy/large_lightgray.txt");
    }
    pub mod fog {
        // Cloud layer (darker)
        pub const SMALL_DARKGRAY: &str = include_str!("../sprites/fog/small_darkgray.txt");
        pub const MEDIUM_DARKGRAY: &str = include_str!("../sprites/fog/medium_darkgray.txt");
        pub const LARGE_DARKGRAY: &str = include_str!("../sprites/fog/large_darkgray.txt");
        // Fog lines (lighter)
        pub const SMALL_LIGHTGRAY: &str = include_str!("../sprites/fog/small_lightgray.txt");
        pub const MEDIUM_LIGHTGRAY: &str = include_str!("../sprites/fog/medium_lightgray.txt");
        pub const LARGE_LIGHTGRAY: &str = include_str!("../sprites/fog/large_lightgray.txt");
    }
    pub mod drizzle {
        // Cloud layer (background)
        pub const SMALL_GRAY: &str = include_str!("../sprites/drizzle/small_gray.txt");
        pub const MEDIUM_GRAY: &str = include_str!("../sprites/drizzle/medium_gray.txt");
        pub const LARGE_GRAY: &str = include_str!("../sprites/drizzle/large_gray.txt");
        // Drizzle layer (foreground)
        pub const SMALL_BLUE: &str = include_str!("../sprites/drizzle/small_blue.txt");
        pub const MEDIUM_BLUE: &str = include_str!("../sprites/drizzle/medium_blue.txt");
        pub const LARGE_BLUE: &str = include_str!("../sprites/drizzle/large_blue.txt");
    }
    pub mod rain {
        // Cloud layer (background)
        pub const SMALL_GRAY: &str = include_str!("../sprites/rain/small_gray.txt");
        pub const MEDIUM_GRAY: &str = include_str!("../sprites/rain/medium_gray.txt");
        pub const LARGE_GRAY: &str = include_str!("../sprites/rain/large_gray.txt");
        // Rain layer (foreground)
        pub const SMALL_BLUE: &str = include_str!("../sprites/rain/small_blue.txt");
        pub const MEDIUM_BLUE: &str = include_str!("../sprites/rain/medium_blue.txt");
        pub const LARGE_BLUE: &str = include_str!("../sprites/rain/large_blue.txt");
    }
    pub mod snow {
        // Cloud layer (background)
        pub const SMALL_GRAY: &str = include_str!("../sprites/snow/small_gray.txt");
        pub const MEDIUM_GRAY: &str = include_str!("../sprites/snow/medium_gray.txt");
        pub const LARGE_GRAY: &str = include_str!("../sprites/snow/large_gray.txt");
        // Snow layer (foreground)
        pub const SMALL_WHITE: &str = include_str!("../sprites/snow/small_white.txt");
        pub const MEDIUM_WHITE: &str = include_str!("../sprites/snow/medium_white.txt");
        pub const LARGE_WHITE: &str = include_str!("../sprites/snow/large_white.txt");
    }
    pub mod thunderstorm {
        // Cloud layer (background)
        pub const SMALL_GRAY: &str = include_str!("../sprites/thunderstorm/small_gray.txt");
        pub const MEDIUM_GRAY: &str = include_str!("../sprites/thunderstorm/medium_gray.txt");
        pub const LARGE_GRAY: &str = include_str!("../sprites/thunderstorm/large_gray.txt");
        // Lightning layer (foreground)
        pub const SMALL_YELLOW: &str = include_str!("../sprites/thunderstorm/small_yellow.txt");
        pub const MEDIUM_YELLOW: &str = include_str!("../sprites/thunderstorm/medium_yellow.txt");
        pub const LARGE_YELLOW: &str = include_str!("../sprites/thunderstorm/large_yellow.txt");
    }
}

// ============================================================================
// Layer compositing
// ============================================================================

/// A single sprite layer with its content and color
struct SpriteLayer {
    content: &'static str,
    color: Color,
}

/// Composite multiple layers into Text, treating spaces as transparent
fn composite_layers(layers: &[SpriteLayer]) -> Text<'static> {
    if layers.is_empty() {
        return Text::default();
    }

    // Pre-collect lines for each layer to avoid repeated .lines() calls
    let layer_lines: Vec<Vec<&str>> = layers.iter().map(|l| l.content.lines().collect()).collect();

    // Get max dimensions
    let max_lines = layer_lines.iter().map(|l| l.len()).max().unwrap_or(0);
    let max_width = layer_lines
        .iter()
        .flat_map(|lines| lines.iter())
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);

    // Build composited lines
    let mut result_lines = Vec::with_capacity(max_lines);

    for line_idx in 0..max_lines {
        let mut spans = Vec::with_capacity(max_width);

        for col_idx in 0..max_width {
            // Find topmost non-space character at this position
            // Iterate layers back-to-front (last layer = top/foreground)
            let mut found_char = ' ';
            let mut found_color = Color::Reset;

            for (layer_idx, layer) in layers.iter().enumerate().rev() {
                if let Some(line) = layer_lines[layer_idx].get(line_idx)
                    && let Some(ch) = line.chars().nth(col_idx)
                    && ch != ' '
                {
                    found_char = ch;
                    found_color = layer.color;
                    break;
                }
            }

            spans.push(Span::styled(
                found_char.to_string(),
                Style::default().fg(found_color),
            ));
        }

        result_lines.push(Line::from(spans));
    }

    Text::from(result_lines)
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
        // Plus temp (1) + description (1) + blank (1) = 3 more for the new vertical layout
        let content_height = height.saturating_sub(10);

        match content_height {
            0..=10 => SpriteSize::Small,
            11..=16 => SpriteSize::Medium,
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
///
/// Multi-layer sprites (like partly_cloudy) are composited with different colors.
pub fn get_sprite(condition: WeatherCondition, size: SpriteSize) -> (Text<'static>, Color) {
    let primary_color = condition.color();

    let layers: Vec<SpriteLayer> = match condition {
        WeatherCondition::ClearSky => vec![SpriteLayer {
            content: match size {
                SpriteSize::Small => sprite_data::sun::SMALL_YELLOW,
                SpriteSize::Medium => sprite_data::sun::MEDIUM_YELLOW,
                SpriteSize::Large => sprite_data::sun::LARGE_YELLOW,
            },
            color: Color::Yellow,
        }],

        WeatherCondition::PartlyCloudy => vec![
            // Sun layer (background - listed first)
            SpriteLayer {
                content: match size {
                    SpriteSize::Small => sprite_data::partly_cloudy::SMALL_YELLOW,
                    SpriteSize::Medium => sprite_data::partly_cloudy::MEDIUM_YELLOW,
                    SpriteSize::Large => sprite_data::partly_cloudy::LARGE_YELLOW,
                },
                color: Color::Yellow,
            },
            // Cloud layer (foreground - listed last, rendered on top)
            SpriteLayer {
                content: match size {
                    SpriteSize::Small => sprite_data::partly_cloudy::SMALL_GRAY,
                    SpriteSize::Medium => sprite_data::partly_cloudy::MEDIUM_GRAY,
                    SpriteSize::Large => sprite_data::partly_cloudy::LARGE_GRAY,
                },
                color: Color::Rgb(200, 200, 210),
            },
        ],

        WeatherCondition::Cloudy | WeatherCondition::Unknown => vec![
            // Back cloud (darker, in background)
            SpriteLayer {
                content: match size {
                    SpriteSize::Small => sprite_data::cloudy::SMALL_DARKGRAY,
                    SpriteSize::Medium => sprite_data::cloudy::MEDIUM_DARKGRAY,
                    SpriteSize::Large => sprite_data::cloudy::LARGE_DARKGRAY,
                },
                color: Color::Rgb(120, 120, 140),
            },
            // Front cloud (lighter, in foreground)
            SpriteLayer {
                content: match size {
                    SpriteSize::Small => sprite_data::cloudy::SMALL_LIGHTGRAY,
                    SpriteSize::Medium => sprite_data::cloudy::MEDIUM_LIGHTGRAY,
                    SpriteSize::Large => sprite_data::cloudy::LARGE_LIGHTGRAY,
                },
                color: Color::Rgb(170, 170, 185),
            },
        ],

        WeatherCondition::Fog => vec![
            // Cloud layer (darker)
            SpriteLayer {
                content: match size {
                    SpriteSize::Small => sprite_data::fog::SMALL_DARKGRAY,
                    SpriteSize::Medium => sprite_data::fog::MEDIUM_DARKGRAY,
                    SpriteSize::Large => sprite_data::fog::LARGE_DARKGRAY,
                },
                color: Color::Rgb(140, 140, 155),
            },
            // Fog lines (lighter, hazy)
            SpriteLayer {
                content: match size {
                    SpriteSize::Small => sprite_data::fog::SMALL_LIGHTGRAY,
                    SpriteSize::Medium => sprite_data::fog::MEDIUM_LIGHTGRAY,
                    SpriteSize::Large => sprite_data::fog::LARGE_LIGHTGRAY,
                },
                color: Color::Rgb(180, 180, 190),
            },
        ],

        WeatherCondition::Drizzle => vec![
            // Cloud layer (background)
            SpriteLayer {
                content: match size {
                    SpriteSize::Small => sprite_data::drizzle::SMALL_GRAY,
                    SpriteSize::Medium => sprite_data::drizzle::MEDIUM_GRAY,
                    SpriteSize::Large => sprite_data::drizzle::LARGE_GRAY,
                },
                color: Color::Rgb(160, 160, 175),
            },
            // Drizzle layer (foreground)
            SpriteLayer {
                content: match size {
                    SpriteSize::Small => sprite_data::drizzle::SMALL_BLUE,
                    SpriteSize::Medium => sprite_data::drizzle::MEDIUM_BLUE,
                    SpriteSize::Large => sprite_data::drizzle::LARGE_BLUE,
                },
                color: Color::Rgb(130, 170, 200),
            },
        ],

        WeatherCondition::Rain => vec![
            // Cloud layer (background)
            SpriteLayer {
                content: match size {
                    SpriteSize::Small => sprite_data::rain::SMALL_GRAY,
                    SpriteSize::Medium => sprite_data::rain::MEDIUM_GRAY,
                    SpriteSize::Large => sprite_data::rain::LARGE_GRAY,
                },
                color: Color::Rgb(160, 160, 175),
            },
            // Rain layer (foreground)
            SpriteLayer {
                content: match size {
                    SpriteSize::Small => sprite_data::rain::SMALL_BLUE,
                    SpriteSize::Medium => sprite_data::rain::MEDIUM_BLUE,
                    SpriteSize::Large => sprite_data::rain::LARGE_BLUE,
                },
                color: Color::Rgb(80, 140, 200),
            },
        ],

        WeatherCondition::Snow => vec![
            // Cloud layer (background)
            SpriteLayer {
                content: match size {
                    SpriteSize::Small => sprite_data::snow::SMALL_GRAY,
                    SpriteSize::Medium => sprite_data::snow::MEDIUM_GRAY,
                    SpriteSize::Large => sprite_data::snow::LARGE_GRAY,
                },
                color: Color::Rgb(160, 160, 175),
            },
            // Snow layer (foreground)
            SpriteLayer {
                content: match size {
                    SpriteSize::Small => sprite_data::snow::SMALL_WHITE,
                    SpriteSize::Medium => sprite_data::snow::MEDIUM_WHITE,
                    SpriteSize::Large => sprite_data::snow::LARGE_WHITE,
                },
                color: Color::Rgb(200, 220, 255),
            },
        ],

        WeatherCondition::Thunderstorm => vec![
            // Cloud layer (background)
            SpriteLayer {
                content: match size {
                    SpriteSize::Small => sprite_data::thunderstorm::SMALL_GRAY,
                    SpriteSize::Medium => sprite_data::thunderstorm::MEDIUM_GRAY,
                    SpriteSize::Large => sprite_data::thunderstorm::LARGE_GRAY,
                },
                color: Color::Rgb(120, 120, 140),
            },
            // Lightning layer (foreground)
            SpriteLayer {
                content: match size {
                    SpriteSize::Small => sprite_data::thunderstorm::SMALL_YELLOW,
                    SpriteSize::Medium => sprite_data::thunderstorm::MEDIUM_YELLOW,
                    SpriteSize::Large => sprite_data::thunderstorm::LARGE_YELLOW,
                },
                color: Color::Yellow,
            },
        ],
    };

    let text = composite_layers(&layers);
    (text, primary_color)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sprite_size_from_terminal() {
        // Small terminals (content_height = height - 10, then 0-10 = small)
        assert_eq!(SpriteSize::from_terminal_size(80, 10), SpriteSize::Small);
        assert_eq!(SpriteSize::from_terminal_size(80, 20), SpriteSize::Small);

        // Medium terminals (content_height 11-16)
        assert_eq!(SpriteSize::from_terminal_size(80, 21), SpriteSize::Medium);
        assert_eq!(SpriteSize::from_terminal_size(80, 26), SpriteSize::Medium);

        // Large terminals (content_height 17+)
        assert_eq!(SpriteSize::from_terminal_size(80, 27), SpriteSize::Large);
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
