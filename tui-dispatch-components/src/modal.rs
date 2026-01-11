//! Modal overlay component with background dimming
//!
//! Dims the background on each frame (keeping animations live) and renders
//! modal content on top.

use ratatui::{buffer::Buffer, layout::Rect, style::Color, widgets::Widget, Frame};
use tui_dispatch_core::debug::dim_buffer;

use crate::style::{BorderStyle, ComponentStyle, Padding};

/// Configuration for modal appearance
///
/// Follows the standard component style pattern with `border`, `padding`, and `bg`.
pub struct ModalStyle {
    /// Dim factor for background (0.0 = no dim, 1.0 = black)
    pub dim_factor: f32,
    /// Border configuration (None = no border)
    pub border: Option<BorderStyle>,
    /// Padding inside the modal (between border and content)
    pub padding: Padding,
    /// Background color for the modal area
    pub bg: Option<Color>,
}

impl Default for ModalStyle {
    fn default() -> Self {
        Self {
            dim_factor: 0.5,
            border: None,
            padding: Padding::default(),
            bg: None,
        }
    }
}

impl ModalStyle {
    /// Create a style with a background color
    pub fn with_bg(bg: Color) -> Self {
        Self {
            bg: Some(bg),
            ..Default::default()
        }
    }

    /// Create a style with a background color and border
    pub fn with_bg_and_border(bg: Color, border: BorderStyle) -> Self {
        Self {
            bg: Some(bg),
            border: Some(border),
            ..Default::default()
        }
    }
}

impl ComponentStyle for ModalStyle {
    fn border(&self) -> Option<&BorderStyle> {
        self.border.as_ref()
    }
    fn padding(&self) -> &Padding {
        &self.padding
    }
    fn bg(&self) -> Option<Color> {
        self.bg
    }
}

/// Render a modal overlay with dimmed background
///
/// Call this AFTER rendering background content. It dims the current buffer,
/// fills the modal area with background color, and renders the border.
///
/// Returns the inner content area (after border and padding) where you should
/// render your modal content.
///
/// # Example
///
/// ```ignore
/// // Render background first
/// weather_display.render(frame, area, props);
///
/// // Then render modal on top (if open)
/// if state.show_dialog {
///     let modal_area = centered_rect(60, 12, frame.area());
///     let content_area = render_modal(frame, modal_area, &ModalStyle::with_bg(Color::Rgb(30, 30, 40)));
///     // Render modal content in content_area
///     my_content.render(frame, content_area, props);
/// }
/// ```
pub fn render_modal(frame: &mut Frame, area: Rect, style: &ModalStyle) -> Rect {
    // Dim the background (everything rendered so far)
    dim_buffer(frame.buffer_mut(), style.dim_factor);

    // Fill modal area with background color
    if let Some(bg) = style.bg {
        frame.render_widget(BgFill(bg), area);
    }

    // Calculate content area (inside border and padding)
    let mut content_area = area;

    // Render border if configured
    if let Some(border) = &style.border {
        use ratatui::widgets::Block;
        let block = Block::default()
            .borders(border.borders)
            .border_style(border.style);
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
    Rect {
        x: content_area.x + style.padding.left,
        y: content_area.y + style.padding.top,
        width: content_area
            .width
            .saturating_sub(style.padding.horizontal()),
        height: content_area.height.saturating_sub(style.padding.vertical()),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::widgets::Paragraph;
    use tui_dispatch_core::testing::RenderHarness;

    #[test]
    fn test_modal_renders_content() {
        let mut harness = RenderHarness::new(80, 24);

        let output = harness.render_to_string_plain(|frame| {
            // Render some background
            frame.render_widget(Paragraph::new("Background content"), frame.area());

            // Render modal
            let area = centered_rect(40, 10, frame.area());
            let content_area =
                render_modal(frame, area, &ModalStyle::with_bg(Color::Rgb(30, 30, 40)));
            frame.render_widget(Paragraph::new("Modal content"), content_area);
        });

        assert!(output.contains("Modal content"));
    }

    #[test]
    fn test_modal_returns_content_area_with_padding() {
        let mut harness = RenderHarness::new(80, 24);

        harness.render_to_string_plain(|frame| {
            let area = Rect::new(10, 10, 40, 20);
            let style = ModalStyle {
                padding: Padding::all(2),
                ..Default::default()
            };
            let content_area = render_modal(frame, area, &style);

            // Content area should be shrunk by padding
            assert_eq!(content_area.x, 12);
            assert_eq!(content_area.y, 12);
            assert_eq!(content_area.width, 36);
            assert_eq!(content_area.height, 16);
        });
    }

    #[test]
    fn test_centered_rect() {
        let area = Rect::new(0, 0, 80, 24);
        let centered = centered_rect(40, 10, area);

        assert_eq!(centered.width, 40);
        assert_eq!(centered.height, 10);
        assert_eq!(centered.x, 20); // (80 - 40) / 2
        assert_eq!(centered.y, 7); // (24 - 10) / 2
    }

    #[test]
    fn test_centered_rect_clamps_to_area() {
        let area = Rect::new(0, 0, 30, 10);
        let centered = centered_rect(100, 50, area);

        // Should clamp to area minus 2 (for some margin)
        assert!(centered.width <= 28);
        assert!(centered.height <= 8);
    }
}
