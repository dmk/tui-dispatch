//! Status bar component with left/center/right sections

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
    Frame,
};
use tui_dispatch_core::Component;

use crate::style::{BorderStyle, ComponentStyle, Padding};

/// Unified styling for StatusBar
#[derive(Debug, Clone)]
pub struct StatusBarStyle {
    /// Border configuration (None = no border)
    pub border: Option<BorderStyle>,
    /// Padding inside the component
    pub padding: Padding,
    /// Background color
    pub bg: Option<Color>,
    /// Default text style
    pub text: Style,
    /// Style for key hints
    pub hint_key: Style,
    /// Style for hint labels
    pub hint_label: Style,
    /// Style for separators
    pub separator: Style,
}

impl Default for StatusBarStyle {
    fn default() -> Self {
        Self {
            border: None,
            padding: Padding::default(),
            bg: None,
            text: Style::default(),
            hint_key: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            hint_label: Style::default(),
            separator: Style::default().fg(Color::DarkGray),
        }
    }
}

impl StatusBarStyle {
    /// Create a style with no border
    pub fn borderless() -> Self {
        Self {
            border: None,
            ..Default::default()
        }
    }

    /// Create a minimal style (no border, no padding)
    pub fn minimal() -> Self {
        Self {
            border: None,
            padding: Padding::default(),
            bg: None,
            text: Style::default(),
            hint_key: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            hint_label: Style::default(),
            separator: Style::default().fg(Color::DarkGray),
        }
    }
}

impl ComponentStyle for StatusBarStyle {
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

/// A single hint entry (key + label)
#[derive(Debug, Clone, Copy)]
pub struct StatusBarHint<'a> {
    pub key: &'a str,
    pub label: &'a str,
}

impl<'a> StatusBarHint<'a> {
    /// Create a new hint entry
    pub fn new(key: &'a str, label: &'a str) -> Self {
        Self { key, label }
    }
}

/// An item within a status bar section
#[derive(Debug, Clone)]
pub enum StatusBarItem<'a> {
    /// Plain text rendered using the default text style
    Text(&'a str),
    /// Styled span (uses its own style)
    Span(Span<'a>),
}

impl<'a> StatusBarItem<'a> {
    /// Create a new text item
    pub fn text(text: &'a str) -> Self {
        Self::Text(text)
    }

    /// Create a new styled span item
    pub fn span(span: Span<'a>) -> Self {
        Self::Span(span)
    }
}

/// Content for a status bar section
#[derive(Debug, Clone)]
pub enum StatusBarContent<'a> {
    /// No content
    Empty,
    /// Render a list of items
    Items(&'a [StatusBarItem<'a>]),
    /// Render a list of hints
    Hints(&'a [StatusBarHint<'a>]),
}

/// Section configuration for the status bar
#[derive(Debug, Clone)]
pub struct StatusBarSection<'a> {
    /// Section content
    pub content: StatusBarContent<'a>,
    /// Separator inserted between items
    pub separator: &'a str,
}

impl<'a> Default for StatusBarSection<'a> {
    fn default() -> Self {
        Self::empty()
    }
}

impl<'a> StatusBarSection<'a> {
    /// Create an empty section
    pub fn empty() -> Self {
        Self {
            content: StatusBarContent::Empty,
            separator: " ",
        }
    }

    /// Create a section from items
    pub fn items(items: &'a [StatusBarItem<'a>]) -> Self {
        Self {
            content: StatusBarContent::Items(items),
            separator: " ",
        }
    }

    /// Create a section from hint items
    pub fn hints(hints: &'a [StatusBarHint<'a>]) -> Self {
        Self {
            content: StatusBarContent::Hints(hints),
            separator: "",
        }
    }

    /// Override the separator between items
    pub fn with_separator(mut self, separator: &'a str) -> Self {
        self.separator = separator;
        self
    }
}

/// Props for StatusBar component
pub struct StatusBarProps<'a> {
    /// Left-aligned section
    pub left: StatusBarSection<'a>,
    /// Center-aligned section
    pub center: StatusBarSection<'a>,
    /// Right-aligned section
    pub right: StatusBarSection<'a>,
    /// Unified styling
    pub style: StatusBarStyle,
    /// Whether the component is focused (for border styling)
    pub is_focused: bool,
}

impl<'a> StatusBarProps<'a> {
    /// Create props with default style
    pub fn new(
        left: StatusBarSection<'a>,
        center: StatusBarSection<'a>,
        right: StatusBarSection<'a>,
    ) -> Self {
        Self {
            left,
            center,
            right,
            style: StatusBarStyle::default(),
            is_focused: false,
        }
    }
}

/// A status bar with left/center/right sections
#[derive(Default)]
pub struct StatusBar;

impl StatusBar {
    /// Create a new StatusBar
    pub fn new() -> Self {
        Self
    }
}

impl<A> Component<A> for StatusBar {
    type Props<'a> = StatusBarProps<'a>;

    fn handle_event(
        &mut self,
        _event: &tui_dispatch_core::EventKind,
        _props: Self::Props<'_>,
    ) -> impl IntoIterator<Item = A> {
        None
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        let style = &props.style;

        let mut background_style = Style::default();
        if let Some(bg) = style.bg {
            background_style = background_style.bg(bg);
        }

        for y in area.y..area.y.saturating_add(area.height) {
            for x in area.x..area.x.saturating_add(area.width) {
                if let Some(cell) = frame.buffer_mut().cell_mut((x, y)) {
                    cell.set_symbol(" ");
                    cell.set_style(background_style);
                }
            }
        }

        let content_area = Rect {
            x: area.x + style.padding.left,
            y: area.y + style.padding.top,
            width: area.width.saturating_sub(style.padding.horizontal()),
            height: area.height.saturating_sub(style.padding.vertical()),
        };

        let mut inner_area = content_area;
        if let Some(border) = &style.border {
            let block = Block::default()
                .borders(border.borders)
                .border_style(border.style_for_focus(props.is_focused));
            inner_area = block.inner(content_area);
            frame.render_widget(block, content_area);
        }

        if inner_area.width == 0 || inner_area.height == 0 {
            return;
        }

        let row_area = Rect {
            y: inner_area.y,
            height: 1,
            ..inner_area
        };

        let left_line = section_line(&props.left, style);
        let center_line = section_line(&props.center, style);
        let right_line = section_line(&props.right, style);

        let content_width = row_area.width as usize;
        let right_width = right_line.width().min(content_width);
        let left_width = left_line
            .width()
            .min(content_width.saturating_sub(right_width));
        let gap_width = content_width.saturating_sub(left_width + right_width);
        let center_width = center_line.width().min(gap_width);

        if left_width > 0 {
            let left_area = Rect {
                width: left_width as u16,
                ..row_area
            };
            frame.render_widget(Paragraph::new(left_line).style(style.text), left_area);
        }

        if center_width > 0 {
            let center_x = row_area.x
                + left_width as u16
                + ((gap_width.saturating_sub(center_width)) / 2) as u16;
            let center_area = Rect {
                x: center_x,
                width: center_width as u16,
                ..row_area
            };
            frame.render_widget(Paragraph::new(center_line).style(style.text), center_area);
        }

        if right_width > 0 {
            let right_area = Rect {
                x: row_area.x + row_area.width.saturating_sub(right_width as u16),
                width: right_width as u16,
                ..row_area
            };
            frame.render_widget(Paragraph::new(right_line).style(style.text), right_area);
        }
    }
}

fn section_line<'a>(section: &StatusBarSection<'a>, style: &StatusBarStyle) -> Line<'a> {
    match section.content {
        StatusBarContent::Empty => Line::raw(""),
        StatusBarContent::Items(items) => items_line(items, section.separator, style),
        StatusBarContent::Hints(hints) => hints_line(hints, section.separator, style),
    }
}

fn items_line<'a>(
    items: &'a [StatusBarItem<'a>],
    separator: &'a str,
    style: &StatusBarStyle,
) -> Line<'a> {
    let mut spans = Vec::new();

    for (idx, item) in items.iter().enumerate() {
        if idx > 0 && !separator.is_empty() {
            spans.push(Span::styled(separator, style.separator));
        }

        match item {
            StatusBarItem::Text(text) => spans.push(Span::styled(*text, style.text)),
            StatusBarItem::Span(span) => spans.push(span.clone()),
        }
    }

    Line::from(spans)
}

fn hints_line<'a>(
    hints: &'a [StatusBarHint<'a>],
    separator: &'a str,
    style: &StatusBarStyle,
) -> Line<'a> {
    let mut spans = Vec::new();

    for (idx, hint) in hints.iter().enumerate() {
        if idx > 0 && !separator.is_empty() {
            spans.push(Span::styled(separator, style.separator));
        }

        spans.push(Span::styled(format!(" {} ", hint.key), style.hint_key));
        spans.push(Span::styled(format!(" {} ", hint.label), style.hint_label));
    }

    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tui_dispatch_core::testing::RenderHarness;

    #[test]
    fn test_status_bar_renders_sections() {
        let mut harness = RenderHarness::new(60, 1);
        let mut status_bar = StatusBar::new();

        let left_items = [StatusBarItem::text("Left")];
        let center_items = [StatusBarItem::text("Center")];
        let right_hints = [StatusBarHint::new("F1", "Help")];

        let output = harness.render_to_string_plain(|frame| {
            <StatusBar as Component<()>>::render(
                &mut status_bar,
                frame,
                frame.area(),
                StatusBarProps {
                    left: StatusBarSection::items(&left_items),
                    center: StatusBarSection::items(&center_items),
                    right: StatusBarSection::hints(&right_hints),
                    style: StatusBarStyle::default(),
                    is_focused: false,
                },
            );
        });

        assert!(output.contains("Left"));
        assert!(output.contains("Center"));
        assert!(output.contains("Help"));
    }
}
