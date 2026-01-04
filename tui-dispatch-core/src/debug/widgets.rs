//! Debug rendering utilities and widgets
//!
//! Provides theme-agnostic widgets for rendering debug information.
//! Applications provide their own styles to customize appearance.

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Clear, Paragraph, Row, Table, Widget};
use ratatui::Frame;

use super::cell::{format_color_compact, format_modifier_compact, CellPreview};
use super::table::{ActionLogOverlay, DebugTableOverlay, DebugTableRow};

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

/// Dim a buffer by scaling colors towards black
///
/// `factor` ranges from 0.0 (no change) to 1.0 (fully dimmed/black)
/// Handles RGB, indexed, and named colors.
/// Emoji characters are replaced with spaces (they can't be dimmed).
pub fn dim_buffer(buffer: &mut Buffer, factor: f32) {
    let factor = factor.clamp(0.0, 1.0);
    let scale = 1.0 - factor; // 0.7 factor = 0.3 scale (30% brightness)

    for cell in buffer.content.iter_mut() {
        // Replace emoji with space - they're pre-colored and can't be dimmed
        if contains_emoji(cell.symbol()) {
            cell.set_symbol(" ");
        }
        cell.fg = dim_color(cell.fg, scale);
        cell.bg = dim_color(cell.bg, scale);
    }
}

/// Check if a string contains emoji characters
fn contains_emoji(s: &str) -> bool {
    for c in s.chars() {
        if is_emoji(c) {
            return true;
        }
    }
    false
}

/// Check if a character is a colored emoji (picture emoji that can't be styled)
///
/// Note: This intentionally excludes Dingbats (0x2700-0x27BF) and Miscellaneous
/// Symbols (0x2600-0x26FF) because those include common TUI glyphs like
/// checkmarks, arrows, and stars that can be styled normally.
fn is_emoji(c: char) -> bool {
    let cp = c as u32;
    // Only match true "picture emoji" that are pre-colored
    matches!(cp,
        // Miscellaneous Symbols and Pictographs (🌀-🗿)
        0x1F300..=0x1F5FF |
        // Emoticons (😀-🙏)
        0x1F600..=0x1F64F |
        // Transport and Map Symbols (🚀-🛿)
        0x1F680..=0x1F6FF |
        // Supplemental Symbols and Pictographs (🤀-🧿)
        0x1F900..=0x1F9FF |
        // Symbols and Pictographs Extended-A (🩠-🩿)
        0x1FA00..=0x1FA6F |
        // Symbols and Pictographs Extended-B (🪀-🫿)
        0x1FA70..=0x1FAFF |
        // Regional Indicator Symbols for flags (🇦-🇿)
        0x1F1E0..=0x1F1FF
    )
}

/// Dim a single color by scaling towards black
fn dim_color(color: ratatui::style::Color, scale: f32) -> ratatui::style::Color {
    use ratatui::style::Color;

    match color {
        Color::Rgb(r, g, b) => Color::Rgb(
            ((r as f32) * scale) as u8,
            ((g as f32) * scale) as u8,
            ((b as f32) * scale) as u8,
        ),
        Color::Indexed(idx) => {
            // Convert indexed colors to RGB, dim, then back
            // Standard 16 colors (0-15) and grayscale (232-255) are common
            if let Some((r, g, b)) = indexed_to_rgb(idx) {
                Color::Rgb(
                    ((r as f32) * scale) as u8,
                    ((g as f32) * scale) as u8,
                    ((b as f32) * scale) as u8,
                )
            } else {
                color // Keep as-is if can't convert
            }
        }
        // Named colors - convert to RGB approximations
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

fn dim_named_color(r: u8, g: u8, b: u8, scale: f32) -> ratatui::style::Color {
    ratatui::style::Color::Rgb(
        ((r as f32) * scale) as u8,
        ((g as f32) * scale) as u8,
        ((b as f32) * scale) as u8,
    )
}

/// Convert 256-color index to RGB (approximate)
fn indexed_to_rgb(idx: u8) -> Option<(u8, u8, u8)> {
    match idx {
        // Standard 16 colors
        0 => Some((0, 0, 0)),        // Black
        1 => Some((128, 0, 0)),      // Red
        2 => Some((0, 128, 0)),      // Green
        3 => Some((128, 128, 0)),    // Yellow
        4 => Some((0, 0, 128)),      // Blue
        5 => Some((128, 0, 128)),    // Magenta
        6 => Some((0, 128, 128)),    // Cyan
        7 => Some((192, 192, 192)),  // White/Gray
        8 => Some((128, 128, 128)),  // Bright Black/Dark Gray
        9 => Some((255, 0, 0)),      // Bright Red
        10 => Some((0, 255, 0)),     // Bright Green
        11 => Some((255, 255, 0)),   // Bright Yellow
        12 => Some((0, 0, 255)),     // Bright Blue
        13 => Some((255, 0, 255)),   // Bright Magenta
        14 => Some((0, 255, 255)),   // Bright Cyan
        15 => Some((255, 255, 255)), // Bright White
        // 216 color cube (16-231)
        16..=231 => {
            let idx = idx - 16;
            let r = (idx / 36) % 6;
            let g = (idx / 6) % 6;
            let b = idx % 6;
            let to_rgb = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            Some((to_rgb(r), to_rgb(g), to_rgb(b)))
        }
        // Grayscale (232-255)
        232..=255 => {
            let gray = 8 + (idx - 232) * 10;
            Some((gray, gray, gray))
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

        // Clear the entire line so the banner overrides any previous content.
        for y in area.y..area.y.saturating_add(area.height) {
            for x in area.x..area.x.saturating_add(area.width) {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_symbol(" ");
                    cell.set_style(self.background);
                }
            }
        }

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
    scroll_offset: usize,
}

impl<'a> DebugTableWidget<'a> {
    /// Create a new debug table widget
    pub fn new(table: &'a DebugTableOverlay) -> Self {
        Self {
            table,
            style: DebugTableStyle::default(),
            scroll_offset: 0,
        }
    }

    /// Set the style configuration
    pub fn style(mut self, style: DebugTableStyle) -> Self {
        self.style = style;
        self
    }

    /// Set the scroll offset for the table body
    pub fn scroll_offset(mut self, scroll_offset: usize) -> Self {
        self.scroll_offset = scroll_offset;
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
        let visible_rows = area.height.saturating_sub(1) as usize;
        let max_offset = self.table.rows.len().saturating_sub(visible_rows);
        let scroll_offset = self.scroll_offset.min(max_offset);

        let rows: Vec<Row> = self
            .table
            .rows
            .iter()
            .skip(scroll_offset)
            .enumerate()
            .map(|(idx, row)| match row {
                DebugTableRow::Section(title) => Row::new(vec![
                    Cell::from(format!(" {title} ")).style(self.style.section),
                    Cell::from(""),
                ]),
                DebugTableRow::Entry { key, value } => {
                    let row_index = idx + scroll_offset;
                    let row_style = if row_index % 2 == 0 {
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

// ============================================================================
// Action Log Widget
// ============================================================================

/// Style configuration for action log rendering
#[derive(Clone)]
pub struct ActionLogStyle {
    /// Style for the header row
    pub header: Style,
    /// Style for sequence numbers
    pub sequence: Style,
    /// Style for action names
    pub name: Style,
    /// Style for action parameters
    pub params: Style,
    /// Style for elapsed time
    pub elapsed: Style,
    /// Selected row style
    pub selected: Style,
    /// Alternating row styles (even, odd)
    pub row_styles: (Style, Style),
}

impl Default for ActionLogStyle {
    fn default() -> Self {
        use super::config::DebugStyle;
        Self {
            header: Style::default()
                .fg(DebugStyle::neon_cyan())
                .add_modifier(Modifier::BOLD),
            sequence: Style::default().fg(DebugStyle::text_secondary()),
            name: Style::default()
                .fg(DebugStyle::neon_amber())
                .add_modifier(Modifier::BOLD),
            params: Style::default().fg(DebugStyle::text_primary()),
            elapsed: Style::default().fg(DebugStyle::text_secondary()),
            selected: Style::default()
                .bg(DebugStyle::bg_highlight())
                .add_modifier(Modifier::BOLD),
            row_styles: (
                Style::default().bg(DebugStyle::bg_panel()),
                Style::default().bg(DebugStyle::bg_surface()),
            ),
        }
    }
}

/// A widget for rendering an action log overlay
///
/// Displays recent actions in a scrollable table format with:
/// - Sequence number
/// - Action name
/// - Parameters (truncated if necessary)
/// - Elapsed time since action
pub struct ActionLogWidget<'a> {
    log: &'a ActionLogOverlay,
    style: ActionLogStyle,
    /// Number of visible rows (for scroll calculations)
    visible_rows: usize,
}

impl<'a> ActionLogWidget<'a> {
    /// Create a new action log widget
    pub fn new(log: &'a ActionLogOverlay) -> Self {
        Self {
            log,
            style: ActionLogStyle::default(),
            visible_rows: 10, // Default, will be adjusted based on area
        }
    }

    /// Set the style configuration
    pub fn style(mut self, style: ActionLogStyle) -> Self {
        self.style = style;
        self
    }

    /// Set the number of visible rows
    pub fn visible_rows(mut self, rows: usize) -> Self {
        self.visible_rows = rows;
        self
    }
}

impl Widget for ActionLogWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 2 || area.width < 30 {
            return;
        }

        // Reserve 1 row for header
        let visible_rows = (area.height.saturating_sub(1)) as usize;

        // Column layout: [#] [Action] [Params] [Elapsed]
        let constraints = [
            Constraint::Length(5),  // Sequence #
            Constraint::Length(20), // Action name
            Constraint::Min(30),    // Params (flexible)
            Constraint::Length(8),  // Elapsed
        ];

        // Header row
        let header = Row::new(vec![
            Cell::from("#").style(self.style.header),
            Cell::from("Action").style(self.style.header),
            Cell::from("Params").style(self.style.header),
            Cell::from("Elapsed").style(self.style.header),
        ]);

        // Calculate scroll offset to keep selected row visible
        let scroll_offset = self.log.scroll_offset_for(visible_rows);

        // Build visible rows
        let rows: Vec<Row> = self
            .log
            .entries
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(visible_rows)
            .map(|(idx, entry)| {
                let is_selected = idx == self.log.selected;
                let base_style = if is_selected {
                    self.style.selected
                } else if idx % 2 == 0 {
                    self.style.row_styles.0
                } else {
                    self.style.row_styles.1
                };

                // Truncate params if needed (char-aware to avoid UTF-8 panic)
                let params = if entry.params.chars().count() > 60 {
                    let truncated: String = entry.params.chars().take(57).collect();
                    format!("{}...", truncated)
                } else {
                    entry.params.clone()
                };

                Row::new(vec![
                    Cell::from(format!("{}", entry.sequence)).style(self.style.sequence),
                    Cell::from(entry.name.clone()).style(self.style.name),
                    Cell::from(params).style(self.style.params),
                    Cell::from(entry.elapsed.clone()).style(self.style.elapsed),
                ])
                .style(base_style)
            })
            .collect();

        let table = Table::new(rows, constraints)
            .header(header)
            .column_spacing(1);

        table.render(area, buf);
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
