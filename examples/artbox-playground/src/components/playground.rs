use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use tui_dispatch::{Component, EventKind};

use crate::action::{Action, Panel};
use crate::state::AppState;

use super::alignment_grid::{AlignmentGrid, AlignmentGridProps};
use super::color_picker::{ColorPicker, ColorPickerProps};
use super::font_selector::{FontSelector, FontSelectorProps};
use super::gradient_editor::{GradientEditor, GradientEditorProps};
use super::help_bar::{HelpBar, HelpBarProps};
use super::preset_panel::{PresetPanel, PresetPanelProps};
use super::preview::{Preview, PreviewProps};
use super::spacing_control::{SpacingControl, SpacingControlProps};
use super::text_input::{TextInput, TextInputProps};

pub struct PlaygroundProps<'a> {
    pub state: &'a AppState,
}

#[derive(Default)]
pub struct Playground {
    text_input: TextInput,
    font_selector: FontSelector,
    color_picker: ColorPicker,
    gradient_editor: GradientEditor,
    alignment_grid: AlignmentGrid,
    spacing_control: SpacingControl,
    preset_panel: PresetPanel,
    preview: Preview,
    help_bar: HelpBar,
}

impl Component<Action> for Playground {
    type Props<'a> = PlaygroundProps<'a>;

    fn handle_event(&mut self, event: &EventKind, props: Self::Props<'_>) -> Vec<Action> {
        let state = props.state;
        let focused = state.focused_panel;

        // Global keys first
        if let EventKind::Key(key) = event {
            use crossterm::event::KeyCode;
            match key.code {
                KeyCode::Tab => return vec![Action::UiFocusNext],
                KeyCode::BackTab => return vec![Action::UiFocusPrev],
                KeyCode::Char('?') => return vec![Action::UiToggleHelp],
                KeyCode::Char('q') | KeyCode::Esc => return vec![Action::Quit],
                _ => {}
            }
        }

        // Delegate to focused component
        match focused {
            Panel::TextInput => self.text_input.handle_event(
                event,
                TextInputProps {
                    text: &state.text,
                    is_focused: true,
                },
            ),
            Panel::FontSelector => self.font_selector.handle_event(
                event,
                FontSelectorProps {
                    family: state.font_family,
                    is_focused: true,
                },
            ),
            Panel::ColorPicker => self.color_picker.handle_event(
                event,
                ColorPickerProps {
                    fill_mode: &state.fill_mode,
                    is_focused: true,
                },
            ),
            Panel::GradientEditor => self.gradient_editor.handle_event(
                event,
                GradientEditorProps {
                    fill_mode: &state.fill_mode,
                    is_focused: true,
                },
            ),
            Panel::AlignmentGrid => self.alignment_grid.handle_event(
                event,
                AlignmentGridProps {
                    alignment: state.alignment,
                    is_focused: true,
                },
            ),
            Panel::SpacingControl => self.spacing_control.handle_event(
                event,
                SpacingControlProps {
                    spacing: state.letter_spacing,
                    is_focused: true,
                },
            ),
            Panel::PresetPanel => self.preset_panel.handle_event(
                event,
                PresetPanelProps {
                    preset_names: &state.preset_names,
                    current_preset: state.current_preset.as_deref(),
                    is_focused: true,
                },
            ),
            Panel::Preview => self.preview.handle_event(
                event,
                PreviewProps {
                    state,
                    is_focused: true,
                },
            ),
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        let state = props.state;
        let focused = state.focused_panel;

        // Main layout: left sidebar, preview, right sidebar, bottom help
        let main_chunks =
            Layout::vertical([Constraint::Min(10), Constraint::Length(1)]).split(area);

        let content_chunks = Layout::horizontal([
            Constraint::Length(30),
            Constraint::Min(40),
            Constraint::Length(25),
        ])
        .split(main_chunks[0]);

        // Left sidebar: text, font, color, gradient
        let left_chunks = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(5),
            Constraint::Min(5),
        ])
        .split(content_chunks[0]);

        self.text_input.render(
            frame,
            left_chunks[0],
            TextInputProps {
                text: &state.text,
                is_focused: focused == Panel::TextInput,
            },
        );

        self.font_selector.render(
            frame,
            left_chunks[1],
            FontSelectorProps {
                family: state.font_family,
                is_focused: focused == Panel::FontSelector,
            },
        );

        self.color_picker.render(
            frame,
            left_chunks[2],
            ColorPickerProps {
                fill_mode: &state.fill_mode,
                is_focused: focused == Panel::ColorPicker,
            },
        );

        self.gradient_editor.render(
            frame,
            left_chunks[3],
            GradientEditorProps {
                fill_mode: &state.fill_mode,
                is_focused: focused == Panel::GradientEditor,
            },
        );

        // Center: preview
        self.preview.render(
            frame,
            content_chunks[1],
            PreviewProps {
                state,
                is_focused: focused == Panel::Preview,
            },
        );

        // Right sidebar: alignment, spacing, presets
        let right_chunks = Layout::vertical([
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Min(5),
        ])
        .split(content_chunks[2]);

        self.alignment_grid.render(
            frame,
            right_chunks[0],
            AlignmentGridProps {
                alignment: state.alignment,
                is_focused: focused == Panel::AlignmentGrid,
            },
        );

        self.spacing_control.render(
            frame,
            right_chunks[1],
            SpacingControlProps {
                spacing: state.letter_spacing,
                is_focused: focused == Panel::SpacingControl,
            },
        );

        self.preset_panel.render(
            frame,
            right_chunks[2],
            PresetPanelProps {
                preset_names: &state.preset_names,
                current_preset: state.current_preset.as_deref(),
                is_focused: focused == Panel::PresetPanel,
            },
        );

        // Bottom: help bar
        self.help_bar.render(
            frame,
            main_chunks[1],
            HelpBarProps {
                focused_panel: focused,
                show_help: state.show_help,
            },
        );
    }
}
