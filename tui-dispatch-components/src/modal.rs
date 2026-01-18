//! Modal overlay component with background dimming
//!
//! Dims the background on each frame (keeping animations live) and renders
//! modal content on top.

use crossterm::event::KeyCode;
use ratatui::{buffer::Buffer, layout::Rect, style::Color, widgets::Widget, Frame};
use tui_dispatch_core::{Component, EventKind};

use crate::style::{BaseStyle, BorderStyle, ComponentStyle};

/// Configuration for modal appearance
///
/// Follows the standard component style pattern with `base` plus a dim factor.
pub struct ModalStyle {
    /// Dim factor for background (0.0 = no dim, 1.0 = black)
    pub dim_factor: f32,
    /// Shared base style
    pub base: BaseStyle,
}

impl Default for ModalStyle {
    fn default() -> Self {
        Self {
            dim_factor: 0.5,
            base: BaseStyle {
                border: None,
                fg: None,
                ..Default::default()
            },
        }
    }
}

impl ModalStyle {
    /// Create a style with a background color
    pub fn with_bg(bg: Color) -> Self {
        let mut style = Self::default();
        style.base.bg = Some(bg);
        style
    }

    /// Create a style with a background color and border
    pub fn with_bg_and_border(bg: Color, border: BorderStyle) -> Self {
        let mut style = Self::default();
        style.base.bg = Some(bg);
        style.base.border = Some(border);
        style
    }
}

impl ComponentStyle for ModalStyle {
    fn base(&self) -> &BaseStyle {
        &self.base
    }
}

/// Behavior configuration for Modal
#[derive(Debug, Clone)]
pub struct ModalBehavior {
    /// Close when the escape key is pressed
    pub close_on_esc: bool,
    /// Close when clicking outside the modal area
    pub close_on_backdrop: bool,
}

impl Default for ModalBehavior {
    fn default() -> Self {
        Self {
            close_on_esc: true,
            close_on_backdrop: false,
        }
    }
}

/// Props for Modal component
pub struct ModalProps<'a, A> {
    /// Whether the modal is open
    pub is_open: bool,
    /// Whether this component has focus
    pub is_focused: bool,
    /// Modal area to render in
    pub area: Rect,
    /// Unified styling
    pub style: ModalStyle,
    /// Behavior configuration
    pub behavior: ModalBehavior,
    /// Callback when the modal should close
    pub on_close: fn() -> A,
    /// Render modal content into the inner area
    pub render_content: &'a mut dyn FnMut(&mut Frame, Rect),
}

/// Modal overlay component
#[derive(Default)]
pub struct Modal;

impl Modal {
    /// Create a new Modal
    pub fn new() -> Self {
        Self
    }
}

impl<A> Component<A> for Modal {
    type Props<'a> = ModalProps<'a, A>;

    fn handle_event(
        &mut self,
        event: &EventKind,
        props: Self::Props<'_>,
    ) -> impl IntoIterator<Item = A> {
        if !props.is_open {
            return None;
        }

        match event {
            EventKind::Key(key) if props.behavior.close_on_esc && key.code == KeyCode::Esc => {
                Some((props.on_close)())
            }
            EventKind::Mouse(mouse) if props.behavior.close_on_backdrop => {
                if !point_in_rect(props.area, mouse.column, mouse.row) {
                    Some((props.on_close)())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    #[allow(unused_mut)]
    fn render(&mut self, frame: &mut Frame, _area: Rect, mut props: Self::Props<'_>) {
        if !props.is_open {
            return;
        }

        let style = &props.style;
        let area = props.area;

        // Dim the background (everything rendered so far)
        if style.dim_factor > 0.0 {
            dim_buffer(frame.buffer_mut(), style.dim_factor);
        }

        // Fill modal area with background color
        if let Some(bg) = style.base.bg {
            frame.render_widget(BgFill(bg), area);
        }

        // Calculate content area (inside border and padding)
        let mut content_area = area;

        // Render border if configured
        if let Some(border) = &style.base.border {
            use ratatui::widgets::Block;
            let block = Block::default()
                .borders(border.borders)
                .border_style(border.style_for_focus(props.is_focused));
            frame.render_widget(block, area);

            // Shrink content area for border
            content_area = Rect {
                x: content_area.x + 1,
                y: content_area.y + 1,
                width: content_area.width.saturating_sub(2),
                height: content_area.height.saturating_sub(2),
            };
        }

        // Apply padding
        let inner_area = Rect {
            x: content_area.x + style.base.padding.left,
            y: content_area.y + style.base.padding.top,
            width: content_area
                .width
                .saturating_sub(style.base.padding.horizontal()),
            height: content_area
                .height
                .saturating_sub(style.base.padding.vertical()),
        };

        (props.render_content)(frame, inner_area);
    }
}

fn point_in_rect(area: Rect, x: u16, y: u16) -> bool {
    x >= area.x
        && x < area.x.saturating_add(area.width)
        && y >= area.y
        && y < area.y.saturating_add(area.height)
}

/// Simple widget that fills an area with a background color
struct BgFill(Color);

impl Widget for BgFill {
    fn render(self, area: Rect, buf: &mut Buffer) {
        for y in area.y..area.y.saturating_add(area.height) {
            for x in area.x..area.x.saturating_add(area.width) {
                buf[(x, y)].set_bg(self.0);
                buf[(x, y)].set_symbol(" ");
            }
        }
    }
}

/// Calculate a centered rectangle within an area
pub fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width.saturating_sub(2));
    let height = height.min(area.height.saturating_sub(2));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height)
}

/// Dim a buffer by scaling colors towards black
///
/// `factor` ranges from 0.0 (no change) to 1.0 (fully dimmed/black).
/// Emoji characters are replaced with spaces (they can't be dimmed).
fn dim_buffer(buffer: &mut Buffer, factor: f32) {
    let factor = factor.clamp(0.0, 1.0);
    let scale = 1.0 - factor;

    for cell in buffer.content.iter_mut() {
        if contains_emoji(cell.symbol()) {
            cell.set_symbol(" ");
        }
        cell.fg = dim_color(cell.fg, scale);
        cell.bg = dim_color(cell.bg, scale);
    }
}

fn contains_emoji(s: &str) -> bool {
    s.chars().any(is_emoji)
}

fn is_emoji(c: char) -> bool {
    let cp = c as u32;
    matches!(
        cp,
        0x1F300..=0x1F5FF |
        0x1F600..=0x1F64F |
        0x1F680..=0x1F6FF |
        0x1F900..=0x1F9FF |
        0x1FA00..=0x1FA6F |
        0x1FA70..=0x1FAFF |
        0x1F1E0..=0x1F1FF
    )
}

fn dim_color(color: Color, scale: f32) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(
            ((r as f32) * scale) as u8,
            ((g as f32) * scale) as u8,
            ((b as f32) * scale) as u8,
        ),
        Color::Indexed(idx) => indexed_to_rgb(idx)
            .map(|(r, g, b)| {
                Color::Rgb(
                    ((r as f32) * scale) as u8,
                    ((g as f32) * scale) as u8,
                    ((b as f32) * scale) as u8,
                )
            })
            .unwrap_or(color),
        Color::Black => Color::Black,
        Color::Red => dim_named_color(205, 0, 0, scale),
        Color::Green => dim_named_color(0, 205, 0, scale),
        Color::Yellow => dim_named_color(205, 205, 0, scale),
        Color::Blue => dim_named_color(0, 0, 238, scale),
        Color::Magenta => dim_named_color(205, 0, 205, scale),
        Color::Cyan => dim_named_color(0, 205, 205, scale),
        Color::Gray => dim_named_color(229, 229, 229, scale),
        Color::DarkGray => dim_named_color(127, 127, 127, scale),
        Color::LightRed => dim_named_color(255, 0, 0, scale),
        Color::LightGreen => dim_named_color(0, 255, 0, scale),
        Color::LightYellow => dim_named_color(255, 255, 0, scale),
        Color::LightBlue => dim_named_color(92, 92, 255, scale),
        Color::LightMagenta => dim_named_color(255, 0, 255, scale),
        Color::LightCyan => dim_named_color(0, 255, 255, scale),
        Color::White => dim_named_color(255, 255, 255, scale),
        Color::Reset => Color::Reset,
    }
}

fn dim_named_color(r: u8, g: u8, b: u8, scale: f32) -> Color {
    Color::Rgb(
        ((r as f32) * scale) as u8,
        ((g as f32) * scale) as u8,
        ((b as f32) * scale) as u8,
    )
}

fn indexed_to_rgb(idx: u8) -> Option<(u8, u8, u8)> {
    match idx {
        0 => Some((0, 0, 0)),
        1 => Some((128, 0, 0)),
        2 => Some((0, 128, 0)),
        3 => Some((128, 128, 0)),
        4 => Some((0, 0, 128)),
        5 => Some((128, 0, 128)),
        6 => Some((0, 128, 128)),
        7 => Some((192, 192, 192)),
        8 => Some((128, 128, 128)),
        9 => Some((255, 0, 0)),
        10 => Some((0, 255, 0)),
        11 => Some((255, 255, 0)),
        12 => Some((0, 0, 255)),
        13 => Some((255, 0, 255)),
        14 => Some((0, 255, 255)),
        15 => Some((255, 255, 255)),
        _ => None,
    }
}
