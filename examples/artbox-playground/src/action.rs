use crate::state::{FontFamily, Preset};
use artbox::{Alignment, Color, ColorStop};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GradientType {
    Linear,
    Radial,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Panel {
    TextInput,
    FontSelector,
    ColorPicker,
    GradientEditor,
    AlignmentGrid,
    SpacingControl,
    PresetPanel,
    Preview,
}

impl Panel {
    pub fn next(self) -> Self {
        match self {
            Panel::TextInput => Panel::FontSelector,
            Panel::FontSelector => Panel::ColorPicker,
            Panel::ColorPicker => Panel::GradientEditor,
            Panel::GradientEditor => Panel::AlignmentGrid,
            Panel::AlignmentGrid => Panel::SpacingControl,
            Panel::SpacingControl => Panel::PresetPanel,
            Panel::PresetPanel => Panel::Preview,
            Panel::Preview => Panel::TextInput,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Panel::TextInput => Panel::Preview,
            Panel::FontSelector => Panel::TextInput,
            Panel::ColorPicker => Panel::FontSelector,
            Panel::GradientEditor => Panel::ColorPicker,
            Panel::AlignmentGrid => Panel::GradientEditor,
            Panel::SpacingControl => Panel::AlignmentGrid,
            Panel::PresetPanel => Panel::SpacingControl,
            Panel::Preview => Panel::PresetPanel,
        }
    }
}

#[derive(tui_dispatch::Action, Clone, Debug, PartialEq)]
#[action(infer_categories)]
pub enum Action {
    // Text
    TextUpdate(String),
    TextClear,

    // Font
    FontSelect(FontFamily),
    FontCycleNext,
    FontCyclePrev,

    // Color
    ColorSetSolid(Color),
    ColorToggleMode,

    // Gradient
    GradientSetType(GradientType),
    GradientSetAngle(f32),
    GradientSetCenter(f32, f32),
    GradientAddStop(ColorStop),
    GradientRemoveStop(usize),
    GradientUpdateStop { index: usize, stop: ColorStop },

    // Alignment
    AlignmentSet(Alignment),

    // Spacing
    SpacingSet(i16),
    SpacingIncrement,
    SpacingDecrement,

    // Preset (async)
    PresetSave(String),
    PresetDidSave(String),
    PresetDidSaveError(String),
    PresetLoad(String),
    PresetDidLoad(Preset),
    PresetDidLoadError(String),
    PresetDelete(String),
    PresetRefresh,
    PresetDidRefresh(Vec<String>),

    // Export
    ExportClipboard,
    ExportDidClipboard,
    ExportDidError(String),
    ExportFile(String),
    ExportDidFile(String),

    // UI
    UiTerminalResize(u16, u16),
    UiFocusPanel(Panel),
    UiFocusNext,
    UiFocusPrev,
    UiToggleHelp,

    // Global
    Tick,
    Quit,
}
