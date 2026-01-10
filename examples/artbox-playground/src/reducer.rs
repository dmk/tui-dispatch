use artbox::Color;
use tui_dispatch::DispatchResult;

use crate::action::{Action, GradientType};
use crate::effect::Effect;
use crate::state::{AppState, FillMode, LinearGradientConfig, RadialGradientConfig, StatusMessage};

pub fn reducer(state: &mut AppState, action: Action) -> DispatchResult<Effect> {
    // Auto-dismiss status messages after ~3 seconds (30 ticks at 100ms)
    if let Some(ref msg) = state.status_message
        && state.tick_count.saturating_sub(msg.tick_shown) > 30
    {
        state.status_message = None;
    }

    match action {
        // Text
        Action::TextUpdate(text) => {
            state.text = text;
            state.current_preset = None;
            DispatchResult::changed()
        }
        Action::TextClear => {
            state.text.clear();
            state.current_preset = None;
            DispatchResult::changed()
        }

        // Font
        Action::FontSelect(family) => {
            state.font_family = family;
            state.current_preset = None;
            DispatchResult::changed()
        }
        Action::FontCycleNext => {
            state.font_family = state.font_family.next();
            state.current_preset = None;
            DispatchResult::changed()
        }
        Action::FontCyclePrev => {
            state.font_family = state.font_family.prev();
            state.current_preset = None;
            DispatchResult::changed()
        }

        // Color
        Action::ColorSetSolid(color) => {
            state.fill_mode = FillMode::Solid(color);
            state.current_preset = None;
            DispatchResult::changed()
        }
        Action::ColorToggleMode => {
            state.fill_mode = match &state.fill_mode {
                FillMode::Solid(_) => FillMode::Linear(LinearGradientConfig::default()),
                FillMode::Linear(_) => FillMode::Radial(RadialGradientConfig::default()),
                FillMode::Radial(_) => FillMode::Solid(Color::rgb(255, 200, 100)),
            };
            state.current_preset = None;
            DispatchResult::changed()
        }

        // Gradient
        Action::GradientSetType(gradient_type) => {
            match gradient_type {
                GradientType::Linear => {
                    if !matches!(state.fill_mode, FillMode::Linear(_)) {
                        let stops = match &state.fill_mode {
                            FillMode::Radial(config) => config.stops.clone(),
                            _ => LinearGradientConfig::default().stops,
                        };
                        state.fill_mode =
                            FillMode::Linear(LinearGradientConfig { angle: 0.0, stops });
                    }
                }
                GradientType::Radial => {
                    if !matches!(state.fill_mode, FillMode::Radial(_)) {
                        let stops = match &state.fill_mode {
                            FillMode::Linear(config) => config.stops.clone(),
                            _ => RadialGradientConfig::default().stops,
                        };
                        state.fill_mode = FillMode::Radial(RadialGradientConfig {
                            center: (0.5, 0.5),
                            radius: 0.7,
                            stops,
                        });
                    }
                }
            }
            state.current_preset = None;
            DispatchResult::changed()
        }
        Action::GradientSetAngle(angle) => {
            if let FillMode::Linear(ref mut config) = state.fill_mode {
                config.angle = angle;
                state.current_preset = None;
                DispatchResult::changed()
            } else {
                DispatchResult::unchanged()
            }
        }
        Action::GradientSetCenter(x, y) => {
            if let FillMode::Radial(ref mut config) = state.fill_mode {
                config.center = (x, y);
                state.current_preset = None;
                DispatchResult::changed()
            } else {
                DispatchResult::unchanged()
            }
        }
        Action::GradientAddStop(stop) => {
            match &mut state.fill_mode {
                FillMode::Linear(config) => config.stops.push(stop),
                FillMode::Radial(config) => config.stops.push(stop),
                _ => return DispatchResult::unchanged(),
            }
            state.current_preset = None;
            DispatchResult::changed()
        }
        Action::GradientRemoveStop(index) => {
            match &mut state.fill_mode {
                FillMode::Linear(config)
                    if config.stops.len() > 2 && index < config.stops.len() =>
                {
                    config.stops.remove(index);
                }
                FillMode::Radial(config)
                    if config.stops.len() > 2 && index < config.stops.len() =>
                {
                    config.stops.remove(index);
                }
                _ => return DispatchResult::unchanged(),
            }
            state.current_preset = None;
            DispatchResult::changed()
        }
        Action::GradientUpdateStop { index, stop } => {
            match &mut state.fill_mode {
                FillMode::Linear(config) if index < config.stops.len() => {
                    config.stops[index] = stop;
                }
                FillMode::Radial(config) if index < config.stops.len() => {
                    config.stops[index] = stop;
                }
                _ => return DispatchResult::unchanged(),
            }
            state.current_preset = None;
            DispatchResult::changed()
        }

        // Alignment
        Action::AlignmentSet(alignment) => {
            state.alignment = alignment;
            state.current_preset = None;
            DispatchResult::changed()
        }

        // Spacing
        Action::SpacingSet(spacing) => {
            state.letter_spacing = spacing.clamp(-5, 10);
            state.current_preset = None;
            DispatchResult::changed()
        }
        Action::SpacingIncrement => {
            state.letter_spacing = (state.letter_spacing + 1).min(10);
            state.current_preset = None;
            DispatchResult::changed()
        }
        Action::SpacingDecrement => {
            state.letter_spacing = (state.letter_spacing - 1).max(-5);
            state.current_preset = None;
            DispatchResult::changed()
        }

        // Preset (async)
        Action::PresetSave(name) => {
            state.is_loading = true;
            let preset = state.to_preset(name.clone());
            DispatchResult::changed_with(Effect::SavePreset { name, preset })
        }
        Action::PresetDidSave(name) => {
            state.is_loading = false;
            state.current_preset = Some(name.clone());
            state.status_message = Some(StatusMessage {
                text: format!("Saved: {}", name),
                is_error: false,
                tick_shown: state.tick_count,
            });
            if !state.preset_names.contains(&name) {
                state.preset_names.push(name);
                state.preset_names.sort();
            }
            DispatchResult::changed()
        }
        Action::PresetDidSaveError(error) => {
            state.is_loading = false;
            state.status_message = Some(StatusMessage {
                text: format!("Save failed: {}", error),
                is_error: true,
                tick_shown: state.tick_count,
            });
            DispatchResult::changed()
        }
        Action::PresetLoad(name) => {
            state.is_loading = true;
            DispatchResult::changed_with(Effect::LoadPreset { name })
        }
        Action::PresetDidLoad(preset) => {
            state.is_loading = false;
            let name = preset.name.clone();
            state.apply_preset(&preset);
            state.current_preset = Some(name.clone());
            state.status_message = Some(StatusMessage {
                text: format!("Loaded: {}", name),
                is_error: false,
                tick_shown: state.tick_count,
            });
            DispatchResult::changed()
        }
        Action::PresetDidLoadError(error) => {
            state.is_loading = false;
            state.status_message = Some(StatusMessage {
                text: format!("Load failed: {}", error),
                is_error: true,
                tick_shown: state.tick_count,
            });
            DispatchResult::changed()
        }
        Action::PresetDelete(name) => DispatchResult::effect(Effect::DeletePreset { name }),
        Action::PresetRefresh => DispatchResult::effect(Effect::RefreshPresets),
        Action::PresetDidRefresh(names) => {
            state.preset_names = names;
            DispatchResult::changed()
        }

        // Export
        Action::ExportClipboard => {
            state.is_loading = true;
            let fill = state.build_fill();
            DispatchResult::changed_with(Effect::ExportClipboard {
                text: state.text.clone(),
                font_family: state.font_family,
                fill,
                alignment: state.alignment,
                letter_spacing: state.letter_spacing,
            })
        }
        Action::ExportDidClipboard => {
            state.is_loading = false;
            state.status_message = Some(StatusMessage {
                text: "Copied to clipboard!".to_string(),
                is_error: false,
                tick_shown: state.tick_count,
            });
            DispatchResult::changed()
        }
        Action::ExportFile(path) => {
            state.is_loading = true;
            let fill = state.build_fill();
            DispatchResult::changed_with(Effect::ExportFile {
                path,
                text: state.text.clone(),
                font_family: state.font_family,
                fill,
                alignment: state.alignment,
                letter_spacing: state.letter_spacing,
            })
        }
        Action::ExportDidFile(path) => {
            state.is_loading = false;
            state.status_message = Some(StatusMessage {
                text: format!("Saved: {}", path),
                is_error: false,
                tick_shown: state.tick_count,
            });
            DispatchResult::changed()
        }
        Action::ExportDidError(error) => {
            state.is_loading = false;
            state.status_message = Some(StatusMessage {
                text: format!("Export failed: {}", error),
                is_error: true,
                tick_shown: state.tick_count,
            });
            DispatchResult::changed()
        }

        // UI
        Action::UiTerminalResize(width, height) => {
            if state.terminal_size != (width, height) {
                state.terminal_size = (width, height);
                DispatchResult::changed()
            } else {
                DispatchResult::unchanged()
            }
        }
        Action::UiFocusPanel(panel) => {
            state.focused_panel = panel;
            DispatchResult::changed()
        }
        Action::UiFocusNext => {
            state.focused_panel = state.focused_panel.next();
            DispatchResult::changed()
        }
        Action::UiFocusPrev => {
            state.focused_panel = state.focused_panel.prev();
            DispatchResult::changed()
        }
        Action::UiToggleHelp => {
            state.show_help = !state.show_help;
            DispatchResult::changed()
        }

        // Global
        Action::Tick => {
            state.tick_count = state.tick_count.wrapping_add(1);
            if state.status_message.is_some() {
                DispatchResult::changed()
            } else {
                DispatchResult::unchanged()
            }
        }
        Action::Quit => DispatchResult::unchanged(),
    }
}
