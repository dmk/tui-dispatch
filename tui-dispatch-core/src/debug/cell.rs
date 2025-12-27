//! Cell inspection utilities
//!
//! Provides types and functions for inspecting individual buffer cells,
//! useful for debugging UI rendering issues.

use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};

/// Visual representation of a buffer cell for debug preview
#[derive(Debug, Clone)]
pub struct CellPreview {
    /// The symbol/character in the cell
    pub symbol: String,
    /// Foreground color
    pub fg: Color,
    /// Background color
    pub bg: Color,
    /// Text modifiers (bold, italic, etc.)
    pub modifier: Modifier,
}

impl CellPreview {
    /// Create a new cell preview
    pub fn new(symbol: impl Into<String>, fg: Color, bg: Color, modifier: Modifier) -> Self {
        Self {
            symbol: symbol.into(),
            fg,
            bg,
            modifier,
        }
    }

    /// Check if the cell has the default/reset style
    pub fn is_default_style(&self) -> bool {
        self.fg == Color::Reset && self.bg == Color::Reset && self.modifier.is_empty()
    }
}

/// Inspect a cell at the given position in a buffer
///
/// Returns `Some(CellPreview)` if the position is within the buffer bounds,
/// `None` otherwise.
///
/// # Example
///
/// ```ignore
/// use tui_dispatch_core::debug::inspect_cell;
///
/// let preview = inspect_cell(&buffer, 10, 5);
/// if let Some(cell) = preview {
///     println!("Symbol: {}, FG: {:?}", cell.symbol, cell.fg);
/// }
/// ```
pub fn inspect_cell(buffer: &Buffer, x: u16, y: u16) -> Option<CellPreview> {
    let area = buffer.area;
    if !point_in_rect(area.x, area.y, area.width, area.height, x, y) {
        return None;
    }

    let cell = &buffer[(x, y)];
    Some(CellPreview {
        symbol: cell.symbol().to_string(),
        fg: cell.fg,
        bg: cell.bg,
        modifier: cell.modifier,
    })
}

/// Check if a point is within a rectangle
#[inline]
pub fn point_in_rect(rect_x: u16, rect_y: u16, width: u16, height: u16, x: u16, y: u16) -> bool {
    let within_x = x >= rect_x && x < rect_x.saturating_add(width);
    let within_y = y >= rect_y && y < rect_y.saturating_add(height);
    within_x && within_y
}

/// Format a color as a compact string for display
///
/// # Example
///
/// ```
/// use ratatui::style::Color;
/// use tui_dispatch_core::debug::format_color_compact;
///
/// assert_eq!(format_color_compact(Color::Rgb(255, 128, 0)), "(255,128,0)");
/// assert_eq!(format_color_compact(Color::Red), "Red");
/// assert_eq!(format_color_compact(Color::Reset), "Reset");
/// ```
pub fn format_color_compact(color: Color) -> String {
    match color {
        Color::Rgb(r, g, b) => format!("({r},{g},{b})"),
        Color::Indexed(i) => format!("#{i}"),
        other => format!("{other:?}"),
    }
}

/// Format modifiers as a compact string
///
/// Returns an empty string if no modifiers are set.
pub fn format_modifier_compact(modifier: Modifier) -> String {
    if modifier.is_empty() {
        return String::new();
    }

    let mut parts = Vec::new();
    if modifier.contains(Modifier::BOLD) {
        parts.push("B");
    }
    if modifier.contains(Modifier::DIM) {
        parts.push("D");
    }
    if modifier.contains(Modifier::ITALIC) {
        parts.push("I");
    }
    if modifier.contains(Modifier::UNDERLINED) {
        parts.push("U");
    }
    if modifier.contains(Modifier::SLOW_BLINK) || modifier.contains(Modifier::RAPID_BLINK) {
        parts.push("*");
    }
    if modifier.contains(Modifier::REVERSED) {
        parts.push("R");
    }
    if modifier.contains(Modifier::CROSSED_OUT) {
        parts.push("X");
    }

    parts.join("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    #[test]
    fn test_point_in_rect() {
        assert!(point_in_rect(0, 0, 10, 10, 5, 5));
        assert!(point_in_rect(0, 0, 10, 10, 0, 0));
        assert!(point_in_rect(0, 0, 10, 10, 9, 9));
        assert!(!point_in_rect(0, 0, 10, 10, 10, 10));
        assert!(!point_in_rect(5, 5, 10, 10, 4, 5));
    }

    #[test]
    fn test_format_color_compact() {
        assert_eq!(format_color_compact(Color::Rgb(255, 0, 128)), "(255,0,128)");
        assert_eq!(format_color_compact(Color::Red), "Red");
        assert_eq!(format_color_compact(Color::Reset), "Reset");
        assert_eq!(format_color_compact(Color::Indexed(42)), "#42");
    }

    #[test]
    fn test_format_modifier_compact() {
        assert_eq!(format_modifier_compact(Modifier::empty()), "");
        assert_eq!(format_modifier_compact(Modifier::BOLD), "B");
        assert_eq!(
            format_modifier_compact(Modifier::BOLD | Modifier::ITALIC),
            "BI"
        );
    }

    #[test]
    fn test_inspect_cell() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 10));
        buffer[(5, 5)].set_char('X');
        buffer[(5, 5)].set_fg(Color::Red);

        let preview = inspect_cell(&buffer, 5, 5).unwrap();
        assert_eq!(preview.symbol, "X");
        assert_eq!(preview.fg, Color::Red);

        // Out of bounds
        assert!(inspect_cell(&buffer, 20, 20).is_none());
    }

    #[test]
    fn test_cell_preview_is_default() {
        let default = CellPreview::new(" ", Color::Reset, Color::Reset, Modifier::empty());
        assert!(default.is_default_style());

        let styled = CellPreview::new("X", Color::Red, Color::Reset, Modifier::empty());
        assert!(!styled.is_default_style());
    }
}
