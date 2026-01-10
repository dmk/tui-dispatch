use artbox::{Alignment, Color, ColorStop, Fill, LinearGradient, RadialGradient};
use serde::{Deserialize, Serialize};

use crate::action::Panel;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FontFamily {
    #[default]
    Default,
    Banner,
    Blocky,
    Script,
    Slant,
}

impl FontFamily {
    pub fn name(&self) -> &'static str {
        match self {
            FontFamily::Default => "default",
            FontFamily::Banner => "banner",
            FontFamily::Blocky => "blocky",
            FontFamily::Script => "script",
            FontFamily::Slant => "slant",
        }
    }

    pub fn all() -> &'static [FontFamily] {
        &[
            FontFamily::Default,
            FontFamily::Banner,
            FontFamily::Blocky,
            FontFamily::Script,
            FontFamily::Slant,
        ]
    }

    pub fn next(self) -> Self {
        match self {
            FontFamily::Default => FontFamily::Banner,
            FontFamily::Banner => FontFamily::Blocky,
            FontFamily::Blocky => FontFamily::Script,
            FontFamily::Script => FontFamily::Slant,
            FontFamily::Slant => FontFamily::Default,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            FontFamily::Default => FontFamily::Slant,
            FontFamily::Banner => FontFamily::Default,
            FontFamily::Blocky => FontFamily::Banner,
            FontFamily::Script => FontFamily::Blocky,
            FontFamily::Slant => FontFamily::Script,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FillMode {
    Solid(Color),
    Linear(LinearGradientConfig),
    Radial(RadialGradientConfig),
}

impl Default for FillMode {
    fn default() -> Self {
        FillMode::Linear(LinearGradientConfig::default())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LinearGradientConfig {
    pub angle: f32,
    pub stops: Vec<ColorStop>,
}

impl Default for LinearGradientConfig {
    fn default() -> Self {
        Self {
            angle: 0.0,
            stops: vec![
                ColorStop::new(0.0, Color::rgb(255, 100, 100)),
                ColorStop::new(1.0, Color::rgb(100, 100, 255)),
            ],
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RadialGradientConfig {
    pub center: (f32, f32),
    pub radius: f32,
    pub stops: Vec<ColorStop>,
}

impl Default for RadialGradientConfig {
    fn default() -> Self {
        Self {
            center: (0.5, 0.5),
            radius: 0.7,
            stops: vec![
                ColorStop::new(0.0, Color::rgb(255, 255, 100)),
                ColorStop::new(1.0, Color::rgb(100, 50, 150)),
            ],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub text: String,
    pub font_family: String,
    pub fill_mode: PresetFillMode,
    pub alignment: String,
    pub letter_spacing: i16,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PresetFillMode {
    Solid {
        r: u8,
        g: u8,
        b: u8,
    },
    Linear {
        angle: f32,
        stops: Vec<PresetColorStop>,
    },
    Radial {
        center: (f32, f32),
        radius: f32,
        stops: Vec<PresetColorStop>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PresetColorStop {
    pub position: f32,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl From<&ColorStop> for PresetColorStop {
    fn from(stop: &ColorStop) -> Self {
        let rgb = stop.color.to_rgb();
        Self {
            position: stop.position,
            r: rgb.r,
            g: rgb.g,
            b: rgb.b,
        }
    }
}

impl From<&PresetColorStop> for ColorStop {
    fn from(stop: &PresetColorStop) -> Self {
        ColorStop::new(stop.position, Color::rgb(stop.r, stop.g, stop.b))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StatusMessage {
    pub text: String,
    pub is_error: bool,
    pub tick_shown: u32,
}

#[derive(Clone, Debug, tui_dispatch::DebugState)]
pub struct AppState {
    pub text: String,
    #[debug(skip)]
    pub font_family: FontFamily,
    #[debug(skip)]
    pub fill_mode: FillMode,
    #[debug(skip)]
    pub alignment: Alignment,
    pub letter_spacing: i16,
    #[debug(skip)]
    pub preset_names: Vec<String>,
    #[debug(skip)]
    pub current_preset: Option<String>,
    #[debug(skip)]
    pub focused_panel: Panel,
    #[debug(skip)]
    pub terminal_size: (u16, u16),
    pub show_help: bool,
    pub tick_count: u32,
    pub is_loading: bool,
    #[debug(skip)]
    pub status_message: Option<StatusMessage>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            text: "Hello".to_string(),
            font_family: FontFamily::default(),
            fill_mode: FillMode::default(),
            alignment: Alignment::Center,
            letter_spacing: 0,
            preset_names: Vec::new(),
            current_preset: None,
            focused_panel: Panel::TextInput,
            terminal_size: (80, 24),
            show_help: false,
            tick_count: 0,
            is_loading: false,
            status_message: None,
        }
    }
}

impl AppState {
    pub fn build_fill(&self) -> Fill {
        match &self.fill_mode {
            FillMode::Solid(color) => Fill::Solid(*color),
            FillMode::Linear(config) => Fill::Linear(LinearGradient {
                angle: config.angle,
                stops: config.stops.clone(),
            }),
            FillMode::Radial(config) => Fill::Radial(RadialGradient {
                center: config.center,
                focal: config.center,
                radius: config.radius,
                stops: config.stops.clone(),
            }),
        }
    }

    pub fn to_preset(&self, name: String) -> Preset {
        Preset {
            name,
            text: self.text.clone(),
            font_family: self.font_family.name().to_string(),
            fill_mode: match &self.fill_mode {
                FillMode::Solid(c) => {
                    let rgb = c.to_rgb();
                    PresetFillMode::Solid {
                        r: rgb.r,
                        g: rgb.g,
                        b: rgb.b,
                    }
                }
                FillMode::Linear(config) => PresetFillMode::Linear {
                    angle: config.angle,
                    stops: config.stops.iter().map(PresetColorStop::from).collect(),
                },
                FillMode::Radial(config) => PresetFillMode::Radial {
                    center: config.center,
                    radius: config.radius,
                    stops: config.stops.iter().map(PresetColorStop::from).collect(),
                },
            },
            alignment: format!("{:?}", self.alignment),
            letter_spacing: self.letter_spacing,
        }
    }

    pub fn apply_preset(&mut self, preset: &Preset) {
        self.text = preset.text.clone();
        self.font_family = match preset.font_family.as_str() {
            "banner" => FontFamily::Banner,
            "blocky" => FontFamily::Blocky,
            "script" => FontFamily::Script,
            "slant" => FontFamily::Slant,
            _ => FontFamily::Default,
        };
        self.fill_mode = match &preset.fill_mode {
            PresetFillMode::Solid { r, g, b } => FillMode::Solid(Color::rgb(*r, *g, *b)),
            PresetFillMode::Linear { angle, stops } => FillMode::Linear(LinearGradientConfig {
                angle: *angle,
                stops: stops.iter().map(ColorStop::from).collect(),
            }),
            PresetFillMode::Radial {
                center,
                radius,
                stops,
            } => FillMode::Radial(RadialGradientConfig {
                center: *center,
                radius: *radius,
                stops: stops.iter().map(ColorStop::from).collect(),
            }),
        };
        self.alignment = match preset.alignment.as_str() {
            "TopLeft" => Alignment::TopLeft,
            "Top" => Alignment::Top,
            "TopRight" => Alignment::TopRight,
            "Left" => Alignment::Left,
            "Center" => Alignment::Center,
            "Right" => Alignment::Right,
            "BottomLeft" => Alignment::BottomLeft,
            "Bottom" => Alignment::Bottom,
            "BottomRight" => Alignment::BottomRight,
            _ => Alignment::Center,
        };
        self.letter_spacing = preset.letter_spacing;
    }
}
