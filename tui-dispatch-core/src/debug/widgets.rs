//! Debug rendering utilities and widgets
//!
//! Provides theme-agnostic widgets for rendering debug information.
//! Applications provide their own styles to customize appearance.

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, Clear, Paragraph, Row, Table, Widget};
use ratatui::Frame;

use super::cell::{format_color_compact, format_modifier_compact, CellPreview};
use super::table::{DebugTableOverlay, DebugTableRow};

/// Convert a buffer to plain text (for clipboard export)
///
/// Trims trailing whitespace from each line.
pub fn buffer_to_text(buffer: &Buffer) -> String {
    let area = buffer.area;
    let mut out = String::new();

    for y in area.y..area.y.saturating_add(area.height) {
        let mut line = String::new();
        for x in area.x..area.x.saturating_add(area.width) {
            line.push_str(buffer[(x, y)].symbol());
        }
        out.push_str(line.trim_end_matches(' '));
        if y + 1 < area.y.saturating_add(area.height) {
            out.push('\n');
        }
    }

    out
}

/// Paint a snapshot buffer onto the current frame
///
/// Clears the frame first, then copies cells from the snapshot.
pub fn paint_snapshot(f: &mut Frame, snapshot: &Buffer) {
    let screen = f.area();
    f.render_widget(Clear, screen);

    let snap_area = snapshot.area;
    let x_end = screen
        .x
        .saturating_add(screen.width)
        .min(snap_area.x.saturating_add(snap_area.width));
    let y_end = screen
        .y
        .saturating_add(screen.height)
        .min(snap_area.y.saturating_add(snap_area.height));

    for y in screen.y..y_end {
        for x in screen.x..x_end {
            f.buffer_mut()[(x, y)] = snapshot[(x, y)].clone();
        }
    }
}

/// Dim a buffer by blending with a darker shade
///
/// `factor` ranges from 0.0 (no change) to 1.0 (fully dimmed)
pub fn dim_buffer(buffer: &mut Buffer, factor: f32) {
    let factor = factor.clamp(0.0, 1.0);
    let dim_amount = (255.0 * factor) as u8;

    for cell in buffer.content.iter_mut() {
        if let ratatui::style::Color::Rgb(r, g, b) = cell.bg {
            cell.bg = ratatui::style::Color::Rgb(
                r.saturating_sub(dim_amount),
                g.saturating_sub(dim_amount),
                b.saturating_sub(dim_amount),
            );
        }
    }
}

/// An item in a debug banner (status bar)
#[derive(Clone)]
pub struct BannerItem<'a> {
    /// The key/label shown in a highlighted style
    pub key: &'a str,
    /// The description shown after the key
    pub label: &'a str,
    /// Style for the key
    pub key_style: Style,
}

impl<'a> BannerItem<'a> {
    /// Create a new banner item
    pub fn new(key: &'a str, label: &'a str, key_style: Style) -> Self {
        Self {
            key,
            label,
            key_style,
        }
    }
}

/// A debug banner widget (status bar at bottom)
///
/// # Example
///
/// ```ignore
/// let banner = DebugBanner::new()
///     .title("DEBUG")
///     .item(BannerItem::new("F12", "resume", key_style))
///     .item(BannerItem::new("S", "state", key_style))
///     .background(bg_style);
///
/// f.render_widget(banner, area);
/// ```
pub struct DebugBanner<'a> {
    title: Option<&'a str>,
    title_style: Style,
    items: Vec<BannerItem<'a>>,
    label_style: Style,
    background: Style,
}

impl<'a> Default for DebugBanner<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> DebugBanner<'a> {
    /// Create a new empty debug banner
    pub fn new() -> Self {
        Self {
            title: None,
            title_style: Style::default(),
            items: Vec::new(),
            label_style: Style::default(),
            background: Style::default(),
        }
    }

    /// Set the banner title (e.g., "DEBUG")
    pub fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    /// Set the title style
    pub fn title_style(mut self, style: Style) -> Self {
        self.title_style = style;
        self
    }

    /// Add an item to the banner
    pub fn item(mut self, item: BannerItem<'a>) -> Self {
        self.items.push(item);
        self
    }

    /// Set the style for item labels
    pub fn label_style(mut self, style: Style) -> Self {
        self.label_style = style;
        self
    }

    /// Set the background style
    pub fn background(mut self, style: Style) -> Self {
        self.background = style;
        self
    }
}

impl Widget for DebugBanner<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        // Fill background
        Block::default().style(self.background).render(area, buf);

        let mut spans = Vec::new();

        // Add title if present
        if let Some(title) = self.title {
            spans.push(Span::styled(format!(" {title} "), self.title_style));
            spans.push(Span::raw(" "));
        }

        // Add items
        for item in &self.items {
            spans.push(Span::styled(format!(" {} ", item.key), item.key_style));
            spans.push(Span::styled(format!(" {} ", item.label), self.label_style));
        }

        let line = Paragraph::new(Line::from(spans)).style(self.background);
        line.render(area, buf);
    }
}

/// Style configuration for debug table rendering
#[derive(Clone)]
pub struct DebugTableStyle {
    /// Style for the header row
    pub header: Style,
    /// Style for section titles
    pub section: Style,
    /// Style for entry keys
    pub key: Style,
    /// Style for entry values
    pub value: Style,
    /// Alternating row styles (even, odd)
    pub row_styles: (Style, Style),
}

impl Default for DebugTableStyle {
    fn default() -> Self {
        use super::config::DebugStyle;
        Self {
            header: Style::default()
                .fg(DebugStyle::neon_cyan())
                .add_modifier(Modifier::BOLD),
            section: Style::default()
                .fg(DebugStyle::neon_purple())
                .add_modifier(Modifier::BOLD),
            key: Style::default()
                .fg(DebugStyle::neon_amber())
                .add_modifier(Modifier::BOLD),
            value: Style::default().fg(DebugStyle::text_primary()),
            row_styles: (
                Style::default().bg(DebugStyle::bg_panel()),
                Style::default().bg(DebugStyle::bg_surface()),
            ),
        }
    }
}

/// A debug table widget that renders a DebugTableOverlay
pub struct DebugTableWidget<'a> {
    table: &'a DebugTableOverlay,
    style: DebugTableStyle,
}

impl<'a> DebugTableWidget<'a> {
    /// Create a new debug table widget
    pub fn new(table: &'a DebugTableOverlay) -> Self {
        Self {
            table,
            style: DebugTableStyle::default(),
        }
    }

    /// Set the style configuration
    pub fn style(mut self, style: DebugTableStyle) -> Self {
        self.style = style;
        self
    }
}

impl Widget for DebugTableWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 2 || area.width < 10 {
            return;
        }

        // Calculate column widths
        let max_key_len = self
            .table
            .rows
            .iter()
            .filter_map(|row| match row {
                DebugTableRow::Entry { key, .. } => Some(key.chars().count()),
                DebugTableRow::Section(_) => None,
            })
            .max()
            .unwrap_or(0) as u16;

        let max_label = area.width.saturating_sub(8).max(10);
        let label_width = max_key_len.saturating_add(2).clamp(12, 30).min(max_label);
        let constraints = [Constraint::Length(label_width), Constraint::Min(0)];

        // Build header
        let header = Row::new(vec![
            Cell::from("Field").style(self.style.header),
            Cell::from("Value").style(self.style.header),
        ]);

        // Build rows
        let rows: Vec<Row> = self
            .table
            .rows
            .iter()
            .enumerate()
            .map(|(idx, row)| match row {
                DebugTableRow::Section(title) => Row::new(vec![
                    Cell::from(format!(" {title} ")).style(self.style.section),
                    Cell::from(""),
                ]),
                DebugTableRow::Entry { key, value } => {
                    let row_style = if idx % 2 == 0 {
                        self.style.row_styles.0
                    } else {
                        self.style.row_styles.1
                    };
                    Row::new(vec![
                        Cell::from(key.clone()).style(self.style.key),
                        Cell::from(value.clone()).style(self.style.value),
                    ])
                    .style(row_style)
                }
            })
            .collect();

        let table = Table::new(rows, constraints)
            .header(header)
            .column_spacing(2);

        table.render(area, buf);
    }
}

/// A widget that renders a cell preview
pub struct CellPreviewWidget<'a> {
    preview: &'a CellPreview,
    label_style: Style,
    value_style: Style,
}

impl<'a> CellPreviewWidget<'a> {
    /// Create a new cell preview widget with default neon styling
    pub fn new(preview: &'a CellPreview) -> Self {
        use super::config::DebugStyle;
        Self {
            preview,
            label_style: Style::default().fg(DebugStyle::text_secondary()),
            value_style: Style::default().fg(DebugStyle::text_primary()),
        }
    }

    /// Set the style for labels (fg, bg, etc.)
    pub fn label_style(mut self, style: Style) -> Self {
        self.label_style = style;
        self
    }

    /// Set the style for values
    pub fn value_style(mut self, style: Style) -> Self {
        self.value_style = style;
        self
    }
}

impl Widget for CellPreviewWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        use super::config::DebugStyle;

        if area.width < 20 || area.height < 1 {
            return;
        }

        // Render the character with its actual style
        let char_style = Style::default()
            .fg(self.preview.fg)
            .bg(self.preview.bg)
            .add_modifier(self.preview.modifier);

        // Format RGB values compactly
        let fg_str = format_color_compact(self.preview.fg);
        let bg_str = format_color_compact(self.preview.bg);
        let mod_str = format_modifier_compact(self.preview.modifier);

        // Character background highlight
        let char_bg = Style::default().bg(DebugStyle::bg_surface());
        let mod_style = Style::default().fg(DebugStyle::neon_purple());

        // Single line: [char]  fg █ RGB  bg █ RGB  mod
        let mut spans = vec![
            Span::styled(" ", char_bg),
            Span::styled(self.preview.symbol.clone(), char_style),
            Span::styled(" ", char_bg),
            Span::styled("  fg ", self.label_style),
            Span::styled("█", Style::default().fg(self.preview.fg)),
            Span::styled(format!(" {fg_str}"), self.value_style),
            Span::styled("  bg ", self.label_style),
            Span::styled("█", Style::default().fg(self.preview.bg)),
            Span::styled(format!(" {bg_str}"), self.value_style),
        ];

        if !mod_str.is_empty() {
            spans.push(Span::styled(format!("  {mod_str}"), mod_style));
        }

        let line = Paragraph::new(Line::from(spans));
        line.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    #[test]
    fn test_buffer_to_text() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));

        // Write some text
        buffer[(0, 0)].set_char('H');
        buffer[(1, 0)].set_char('i');
        buffer[(0, 1)].set_char('!');

        let text = buffer_to_text(&buffer);
        let lines: Vec<&str> = text.lines().collect();

        assert_eq!(lines[0], "Hi");
        assert_eq!(lines[1], "!");
    }

    #[test]
    fn test_debug_banner() {
        let banner =
            DebugBanner::new()
                .title("TEST")
                .item(BannerItem::new("F1", "help", Style::default()));

        let mut buffer = Buffer::empty(Rect::new(0, 0, 40, 1));
        banner.render(Rect::new(0, 0, 40, 1), &mut buffer);

        let text = buffer_to_text(&buffer);
        assert!(text.contains("TEST"));
        assert!(text.contains("F1"));
        assert!(text.contains("help"));
    }
}
