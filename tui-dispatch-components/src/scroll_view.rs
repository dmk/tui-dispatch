//! Scrollable text view component

use crossterm::event::KeyCode;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Paragraph, ScrollbarOrientation, ScrollbarState},
    Frame,
};
use tui_dispatch_core::{Component, EventKind};

use crate::style::{BorderStyle, ComponentStyle, Padding, ScrollbarStyle};

/// Unified styling for ScrollView
#[derive(Debug, Clone)]
pub struct ScrollViewStyle {
    /// Border configuration (None = no border)
    pub border: Option<BorderStyle>,
    /// Padding inside the component
    pub padding: Padding,
    /// Background color
    pub bg: Option<Color>,
    /// Foreground (text) color
    pub fg: Option<Color>,
    /// Scrollbar styling
    pub scrollbar: ScrollbarStyle,
}

impl Default for ScrollViewStyle {
    fn default() -> Self {
        Self {
            border: Some(BorderStyle::default()),
            padding: Padding::default(),
            bg: None,
            fg: Some(Color::Reset),
            scrollbar: ScrollbarStyle::default(),
        }
    }
}

impl ScrollViewStyle {
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
            fg: Some(Color::Reset),
            scrollbar: ScrollbarStyle::default(),
        }
    }
}

impl ComponentStyle for ScrollViewStyle {
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

/// Behavior configuration for ScrollView
#[derive(Debug, Clone)]
pub struct ScrollBehavior {
    /// Show scrollbar when content exceeds viewport
    pub show_scrollbar: bool,
    /// Number of lines to scroll per step
    pub scroll_step: usize,
    /// Page size for PageUp/PageDown (0 = viewport height)
    pub page_step: usize,
}

impl Default for ScrollBehavior {
    fn default() -> Self {
        Self {
            show_scrollbar: true,
            scroll_step: 1,
            page_step: 0,
        }
    }
}

/// Props for ScrollView component
pub struct ScrollViewProps<'a, A> {
    /// Lines to render (may be a slice of the full content)
    pub lines: &'a [Line<'a>],
    /// Total number of lines in the full content
    pub content_len: usize,
    /// Index of the first line in `lines` within the full content
    pub line_offset: usize,
    /// Current scroll offset (topmost line index)
    pub scroll_offset: usize,
    /// Whether this component has focus
    pub is_focused: bool,
    /// Unified styling
    pub style: ScrollViewStyle,
    /// Behavior configuration
    pub behavior: ScrollBehavior,
    /// Callback to create action when scroll offset changes
    pub on_scroll: fn(usize) -> A,
}

impl<'a, A> ScrollViewProps<'a, A> {
    /// Create props with sensible defaults
    ///
    /// Sets `content_len` to `lines.len()`, `line_offset` to `0`, `is_focused` to `true`,
    /// and uses default style/behavior.
    pub fn new(lines: &'a [Line<'a>], scroll_offset: usize, on_scroll: fn(usize) -> A) -> Self {
        Self {
            lines,
            content_len: lines.len(),
            line_offset: 0,
            scroll_offset,
            is_focused: true,
            style: ScrollViewStyle::default(),
            behavior: ScrollBehavior::default(),
            on_scroll,
        }
    }
}

/// A scrollable view for pre-wrapped lines
#[derive(Default)]
pub struct ScrollView {
    viewport_height: usize,
}

impl ScrollView {
    /// Create a new ScrollView
    pub fn new() -> Self {
        Self::default()
    }

    fn viewport_height_value(&self) -> usize {
        self.viewport_height.max(1)
    }

    fn max_offset(&self, content_len: usize) -> usize {
        content_len.saturating_sub(self.viewport_height_value())
    }

    fn scrollbar_content_length(&self, content_len: usize) -> usize {
        content_len
            .saturating_sub(self.viewport_height_value())
            .saturating_add(1)
    }

    fn page_size(&self, behavior: &ScrollBehavior) -> usize {
        if behavior.page_step > 0 {
            behavior.page_step
        } else {
            self.viewport_height_value()
        }
    }

    fn apply_delta(&self, current: usize, delta: isize, max_offset: usize) -> usize {
        if delta >= 0 {
            current.saturating_add(delta as usize).min(max_offset)
        } else {
            current.saturating_sub((-delta) as usize)
        }
    }
}

impl<A> Component<A> for ScrollView {
    type Props<'a> = ScrollViewProps<'a, A>;

    fn handle_event(
        &mut self,
        event: &EventKind,
        props: Self::Props<'_>,
    ) -> impl IntoIterator<Item = A> {
        if !props.is_focused || props.content_len == 0 {
            return None;
        }

        let max_offset = self.max_offset(props.content_len);
        let scroll_step = props.behavior.scroll_step.max(1) as isize;
        let page_size = self.page_size(&props.behavior) as isize;

        let next_offset = match event {
            EventKind::Key(key) => match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    Some(self.apply_delta(props.scroll_offset, scroll_step, max_offset))
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    Some(self.apply_delta(props.scroll_offset, -scroll_step, max_offset))
                }
                KeyCode::PageDown => {
                    Some(self.apply_delta(props.scroll_offset, page_size, max_offset))
                }
                KeyCode::PageUp => {
                    Some(self.apply_delta(props.scroll_offset, -page_size, max_offset))
                }
                KeyCode::Char('g') | KeyCode::Home => Some(0),
                KeyCode::Char('G') | KeyCode::End => Some(max_offset),
                _ => None,
            },
            EventKind::Scroll { delta, .. } => {
                if *delta == 0 {
                    None
                } else {
                    let scaled_delta = delta.saturating_mul(scroll_step);
                    Some(self.apply_delta(props.scroll_offset, scaled_delta, max_offset))
                }
            }
            _ => None,
        };

        match next_offset {
            Some(offset) if offset != props.scroll_offset => Some((props.on_scroll)(offset)),
            _ => None,
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        let style = &props.style;

        if let Some(bg) = style.bg {
            for y in area.y..area.y.saturating_add(area.height) {
                for x in area.x..area.x.saturating_add(area.width) {
                    frame.buffer_mut()[(x, y)].set_bg(bg);
                    frame.buffer_mut()[(x, y)].set_symbol(" ");
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

        let mut text_area = inner_area;
        let viewport_height = text_area.height as usize;
        self.viewport_height = viewport_height;

        if text_area.width == 0 || text_area.height == 0 {
            return;
        }

        let show_scrollbar = props.behavior.show_scrollbar
            && viewport_height > 0
            && props.content_len > viewport_height
            && text_area.width > 1;
        let scrollbar_area = if show_scrollbar {
            let scrollbar_area = Rect {
                x: text_area.x + text_area.width.saturating_sub(1),
                width: 1,
                ..text_area
            };
            text_area.width = text_area.width.saturating_sub(1);
            Some(scrollbar_area)
        } else {
            None
        };

        let max_offset = self.max_offset(props.content_len);
        let render_offset = props.scroll_offset.min(max_offset);
        let line_offset = props.line_offset.min(props.content_len);
        let line_end = line_offset
            .saturating_add(props.lines.len())
            .min(props.content_len);
        let visible_end = (render_offset + viewport_height).min(props.content_len);

        let mut lines = Vec::new();
        for idx in render_offset..visible_end {
            if idx >= line_offset && idx < line_end {
                lines.push(props.lines[idx - line_offset].clone());
            } else {
                lines.push(Line::raw(""));
            }
        }

        let mut text_style = Style::default();
        if let Some(fg) = style.fg {
            text_style = text_style.fg(fg);
        }
        if let Some(bg) = style.bg {
            text_style = text_style.bg(bg);
        }

        let paragraph = Paragraph::new(lines).style(text_style);
        frame.render_widget(paragraph, text_area);

        if let Some(scrollbar_area) = scrollbar_area {
            let scrollbar = style.scrollbar.build(ScrollbarOrientation::VerticalRight);
            let scrollbar_len = self.scrollbar_content_length(props.content_len);
            let mut scrollbar_state = ScrollbarState::new(scrollbar_len)
                .position(render_offset)
                .viewport_content_length(self.viewport_height_value());
            frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use tui_dispatch_core::testing::{key, RenderHarness};

    #[derive(Debug, Clone, PartialEq)]
    enum TestAction {
        ScrollTo(usize),
    }

    fn make_lines(count: usize) -> Vec<Line<'static>> {
        (0..count)
            .map(|i| Line::raw(format!("Line {}", i)))
            .collect()
    }

    fn props<'a>(lines: &'a [Line<'a>], scroll_offset: usize) -> ScrollViewProps<'a, TestAction> {
        ScrollViewProps {
            lines,
            content_len: lines.len(),
            line_offset: 0,
            scroll_offset,
            is_focused: true,
            style: ScrollViewStyle::borderless(),
            behavior: ScrollBehavior::default(),
            on_scroll: TestAction::ScrollTo,
        }
    }

    #[test]
    fn test_scroll_down_action() {
        let mut view = ScrollView::new();
        let lines = make_lines(5);
        let mut harness = RenderHarness::new(20, 3);

        harness.render_to_string_plain(|frame| {
            view.render(frame, frame.area(), props(&lines, 0));
        });

        let actions: Vec<_> = view
            .handle_event(&EventKind::Key(key("j")), props(&lines, 0))
            .into_iter()
            .collect();

        assert_eq!(actions, vec![TestAction::ScrollTo(1)]);
    }

    #[test]
    fn test_page_down_action() {
        let mut view = ScrollView::new();
        let lines = make_lines(10);
        let mut harness = RenderHarness::new(20, 4);

        harness.render_to_string_plain(|frame| {
            view.render(frame, frame.area(), props(&lines, 0));
        });

        let page_down = KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE);
        let actions: Vec<_> = view
            .handle_event(&EventKind::Key(page_down), props(&lines, 0))
            .into_iter()
            .collect();

        assert_eq!(actions, vec![TestAction::ScrollTo(4)]);
    }

    #[test]
    fn test_scroll_wheel_action() {
        let mut view = ScrollView::new();
        let lines = make_lines(5);
        let mut harness = RenderHarness::new(20, 3);

        harness.render_to_string_plain(|frame| {
            view.render(frame, frame.area(), props(&lines, 1));
        });

        let actions: Vec<_> = view
            .handle_event(
                &EventKind::Scroll {
                    column: 0,
                    row: 0,
                    delta: -1,
                },
                props(&lines, 1),
            )
            .into_iter()
            .collect();

        assert_eq!(actions, vec![TestAction::ScrollTo(0)]);
    }

    #[test]
    fn test_render_respects_offset() {
        let mut view = ScrollView::new();
        let lines = make_lines(6);
        let mut harness = RenderHarness::new(20, 3);

        let output = harness.render_to_string_plain(|frame| {
            view.render(frame, frame.area(), props(&lines, 2));
        });

        assert!(output.contains("Line 2"));
        assert!(!output.contains("Line 0"));
    }
}
