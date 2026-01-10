//! ASCII art preview with mouse-interactive gradient center.
//!
//! # Educational Pattern: Mouse Bypass
//!
//! High-frequency mouse move events update component's internal state directly
//! for zero-latency preview. Only on click do we dispatch an action to persist
//! the gradient center to app state.
//!
//! This demonstrates when to "go around" tui-dispatch:
//! - Mouse move: Update `&mut self` for instant visual feedback
//! - Mouse click: Dispatch `GradientSetCenter` to persist to state
//!
//! This pattern is useful when:
//! 1. Events are too frequent for full dispatch cycle (60+ times/sec)
//! 2. Visual feedback needs to be immediate
//! 3. Only final value matters for state persistence

use artbox::integrations::ratatui::ArtBox;
use artbox::{Fill, LinearGradient, RadialGradient, Renderer};
use crossterm::event::{MouseButton, MouseEventKind};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders};
use tui_dispatch::{Component, EventKind};

use crate::action::Action;
use crate::state::{AppState, FillMode};

pub struct PreviewProps<'a> {
    pub state: &'a AppState,
    pub is_focused: bool,
}

/// ASCII art preview with mouse-interactive gradient center.
#[derive(Default)]
pub struct Preview {
    /// Internal preview center - updated on mouse move.
    /// NOT part of app state, exists only for visual preview.
    preview_center: Option<(f32, f32)>,

    /// The preview area (updated each render for hit testing).
    render_area: Option<Rect>,
}

impl Component<Action> for Preview {
    type Props<'a> = PreviewProps<'a>;

    fn handle_event(&mut self, event: &EventKind, props: Self::Props<'_>) -> Vec<Action> {
        if !props.is_focused {
            self.preview_center = None;
            return vec![];
        }

        // Only handle mouse events for radial gradient mode
        let is_radial = matches!(props.state.fill_mode, FillMode::Radial(_));
        if !is_radial {
            self.preview_center = None;
            return vec![];
        }

        match event {
            EventKind::Mouse(mouse) => {
                let Some(area) = self.render_area else {
                    return vec![];
                };

                // Check if mouse is within our area
                if mouse.column < area.x
                    || mouse.column >= area.x + area.width
                    || mouse.row < area.y
                    || mouse.row >= area.y + area.height
                {
                    self.preview_center = None;
                    return vec![];
                }

                // Convert to normalized coordinates (0.0-1.0)
                let x = (mouse.column - area.x) as f32 / area.width.max(1) as f32;
                let y = (mouse.row - area.y) as f32 / area.height.max(1) as f32;

                match mouse.kind {
                    MouseEventKind::Moved | MouseEventKind::Drag(_) => {
                        // BYPASS PATTERN: Update internal state directly
                        // No action dispatched - just visual preview
                        self.preview_center = Some((x, y));
                        vec![] // No action - internal state only
                    }
                    MouseEventKind::Down(MouseButton::Left) => {
                        // PERSIST PATTERN: Click dispatches action to save to state
                        self.preview_center = Some((x, y));
                        vec![Action::GradientSetCenter(x, y)]
                    }
                    _ => vec![],
                }
            }
            EventKind::Key(key) => {
                use crossterm::event::KeyCode;
                match key.code {
                    KeyCode::Char('c') => vec![Action::ExportClipboard],
                    _ => vec![],
                }
            }
            _ => vec![],
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        // Store area for mouse hit testing
        self.render_area = Some(area);

        let state = props.state;

        // Border style based on focus
        let border_style = if props.is_focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let title = if matches!(state.fill_mode, FillMode::Radial(_)) && props.is_focused {
            " Preview (click to set gradient center) "
        } else {
            " Preview "
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(border_style);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if state.text.is_empty() {
            return;
        }

        // Build the fill, potentially using preview_center for radial gradient
        let fill = self.build_preview_fill(state);

        // Build renderer with current settings
        let fonts = artbox::fonts::family(state.font_family.name()).unwrap_or_default();
        let renderer = Renderer::new(fonts)
            .with_plain_fallback()
            .with_alignment(state.alignment)
            .with_letter_spacing(state.letter_spacing)
            .with_fill(fill);

        // Render the ASCII art
        let widget = ArtBox::new(&renderer, &state.text);
        frame.render_widget(widget, inner);

        // Show radial center indicator if in radial mode and focused
        if props.is_focused && matches!(state.fill_mode, FillMode::Radial(_)) {
            self.render_center_indicator(frame, inner);
        }
    }
}

impl Preview {
    /// Build fill using preview_center if available, otherwise state center.
    fn build_preview_fill(&self, state: &AppState) -> Fill {
        match &state.fill_mode {
            FillMode::Solid(color) => Fill::Solid(*color),
            FillMode::Linear(config) => Fill::Linear(LinearGradient {
                angle: config.angle,
                stops: config.stops.clone(),
            }),
            FillMode::Radial(config) => {
                // Use preview_center if available (mouse is hovering)
                // Otherwise use persisted center from state
                let center = self.preview_center.unwrap_or(config.center);
                Fill::Radial(RadialGradient {
                    center,
                    focal: center,
                    radius: config.radius,
                    stops: config.stops.clone(),
                })
            }
        }
    }

    /// Render a small crosshair at the gradient center.
    fn render_center_indicator(&self, frame: &mut Frame, area: Rect) {
        let center = self.preview_center;
        if let Some((x, y)) = center {
            let px = area.x + (x * area.width as f32) as u16;
            let py = area.y + (y * area.height as f32) as u16;

            if px >= area.x && px < area.x + area.width && py >= area.y && py < area.y + area.height
            {
                let buf = frame.buffer_mut();
                buf[(px, py)].set_char('+').set_fg(Color::Yellow);
            }
        }
    }
}
