use crate::state::{FontFamily, Preset};
use artbox::{Alignment, Fill};

#[derive(Debug, Clone)]
pub enum Effect {
    SavePreset {
        name: String,
        preset: Preset,
    },
    LoadPreset {
        name: String,
    },
    DeletePreset {
        name: String,
    },
    RefreshPresets,
    ExportClipboard {
        text: String,
        font_family: FontFamily,
        fill: Fill,
        alignment: Alignment,
        letter_spacing: i16,
    },
    ExportFile {
        path: String,
        text: String,
        font_family: FontFamily,
        fill: Fill,
        alignment: Alignment,
        letter_spacing: i16,
    },
}

pub fn preset_dir() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("artbox-playground")
        .join("presets")
}
