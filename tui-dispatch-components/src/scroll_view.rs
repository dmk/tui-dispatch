//! Scrollable viewport component
//!
//! ScrollView is a flexible viewport container that handles scrolling behavior.
//! The user provides a `render_content` callback to render visible content.
//!
//! For simple use cases with pre-rendered lines, use [`LinesScroller`].

use std::rc::Rc;

use crossterm::event::KeyCode;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Paragraph, ScrollbarOrientation, ScrollbarState},
    Frame,
};
use tui_dispatch_core::{Component, EventKind, HandlerResponse};

use crate::commands;
use crate::style::{BaseStyle, ComponentStyle, Padding, ScrollbarStyle};
use crate::{ComponentDebugEntry, ComponentDebugState, ComponentInput, InteractiveComponent};

/// Information about the visible range for content rendering
#[derive(Debug, Clone, Copy)]
pub struct VisibleRange {
    /// First visible line index (0-based)
    pub start: usize,
    /// Last visible line index (exclusive)
    pub end: usize,
    /// Height of the viewport in lines
    pub viewport_height: u16,
    /// Available width for content (excluding scrollbar if shown)
    pub available_width: u16,
}

/// Unified styling for ScrollView
#[derive(Debug, Clone)]
pub struct ScrollViewStyle {
    /// Shared base style
    pub base: BaseStyle,
    /// Scrollbar styling
    pub scrollbar: ScrollbarStyle,
}

impl Default for ScrollViewStyle {
    fn default() -> Self {
        Self {
            base: BaseStyle {
                fg: Some(Color::Reset),
                ..Default::default()
            },
            scrollbar: ScrollbarStyle::default(),
        }
    }
}

impl ScrollViewStyle {
    /// Create a style with no border
    pub fn borderless() -> Self {
        let mut style = Self::default();
        style.base.border = None;
        style
    }

    /// Create a minimal style (no border, no padding)
    pub fn minimal() -> Self {
        let mut style = Self::default();
        style.base.border = None;
        style.base.padding = Padding::default();
        style
    }
}

impl ComponentStyle for ScrollViewStyle {
    fn base(&self) -> &BaseStyle {
        &self.base
    }
}

/// Behavior configuration for ScrollView
#[derive(Debug, Clone)]
pub struct ScrollViewBehavior {
    /// Show scrollbar when content exceeds viewport
    pub show_scrollbar: bool,
    /// Number of lines to scroll per step
    pub scroll_step: usize,
    /// Page size for PageUp/PageDown (0 = viewport height)
    pub page_step: usize,
}

impl Default for ScrollViewBehavior {
    fn default() -> Self {
        Self {
            show_scrollbar: true,
            scroll_step: 1,
            page_step: 0,
        }
    }
}

/// Callback to create an action when the scroll offset changes.
pub type ScrollViewCallback<A> = Rc<dyn Fn(usize) -> A>;

/// Props for ScrollView component
pub struct ScrollViewProps<'a, A> {
    /// Total height of the content in lines
    pub content_height: usize,
    /// Current scroll offset (topmost visible line index)
    pub scroll_offset: usize,
    /// Whether this component has focus
    pub is_focused: bool,
    /// Unified styling
    pub style: ScrollViewStyle,
    /// Behavior configuration
    pub behavior: ScrollViewBehavior,
    /// Callback to create action when scroll offset changes
    pub on_scroll: ScrollViewCallback<A>,
    /// Callback to render visible content
    ///
    /// Called with the content area and visible range. The callback should
    /// render content for lines `range.start..range.end` into the given area.
    pub render_content: &'a mut dyn FnMut(&mut Frame, Rect, VisibleRange),
}

/// Render-only props for ScrollView
pub struct ScrollViewRenderProps<'a> {
    /// Total height of the content in lines
    pub content_height: usize,
    /// Current scroll offset (topmost visible line index)
    pub scroll_offset: usize,
    /// Whether this component has focus
    pub is_focused: bool,
    /// Unified styling
    pub style: ScrollViewStyle,
    /// Behavior configuration
    pub behavior: ScrollViewBehavior,
    /// Callback to render visible content
    pub render_content: &'a mut dyn FnMut(&mut Frame, Rect, VisibleRange),
}

/// A scrollable viewport container
///
/// Handles scroll behavior (keyboard, mouse wheel) and renders scrollbar.
/// Content rendering is delegated to the `render_content` callback.
///
/// For simple cases with pre-rendered lines, use [`LinesScroller`].
#[derive(Default)]
pub struct ScrollView {
    viewport_height: usize,
}

impl ScrollView {
    /// Create a new ScrollView
    pub fn new() -> Self {
        Self::default()
    }

    /// Render the widget without requiring scroll callbacks.
    pub fn render_widget(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        props: ScrollViewRenderProps<'_>,
    ) {
        self.render_with(frame, area, props);
    }

    fn viewport_height_value(&self) -> usize {
        self.viewport_height.max(1)
    }

    fn max_offset(&self, content_height: usize) -> usize {
        content_height.saturating_sub(self.viewport_height_value())
    }

    fn scrollbar_content_length(&self, content_height: usize) -> usize {
        content_height
            .saturating_sub(self.viewport_height_value())
            .saturating_add(1)
    }

    fn page_size(&self, behavior: &ScrollViewBehavior) -> usize {
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

    fn render_with(&mut self, frame: &mut Frame, area: Rect, props: ScrollViewRenderProps<'_>) {
        let style = &props.style;

        // Fill background
        if let Some(bg) = style.base.bg {
            for y in area.y..area.y.saturating_add(area.height) {
                for x in area.x..area.x.saturating_add(area.width) {
                    frame.buffer_mut()[(x, y)].set_bg(bg);
                    frame.buffer_mut()[(x, y)].set_symbol(" ");
                }
            }
        }

        // Apply padding
        let content_area = Rect {
            x: area.x + style.base.padding.left,
            y: area.y + style.base.padding.top,
            width: area.width.saturating_sub(style.base.padding.horizontal()),
            height: area.height.saturating_sub(style.base.padding.vertical()),
        };

        // Apply border
        let mut inner_area = content_area;
        if let Some(border) = &style.base.border {
            let block = Block::default()
                .borders(border.borders)
                .border_style(border.style_for_focus(props.is_focused));
            inner_area = block.inner(content_area);
            frame.render_widget(block, content_area);
        }

        let viewport_height = inner_area.height as usize;
        self.viewport_height = viewport_height;

        if inner_area.width == 0 || inner_area.height == 0 {
            return;
        }

        // Determine scrollbar visibility and adjust content area
        let show_scrollbar = props.behavior.show_scrollbar
            && viewport_height > 0
            && props.content_height > viewport_height
            && inner_area.width > 1;

        let (content_area, scrollbar_area) = if show_scrollbar {
            let scrollbar_area = Rect {
                x: inner_area.x + inner_area.width.saturating_sub(1),
                width: 1,
                ..inner_area
            };
            let content_area = Rect {
                width: inner_area.width.saturating_sub(1),
                ..inner_area
            };
            (content_area, Some(scrollbar_area))
        } else {
            (inner_area, None)
        };

        // Calculate visible range
        let max_offset = self.max_offset(props.content_height);
        let scroll_offset = props.scroll_offset.min(max_offset);
        let visible_end = (scroll_offset + viewport_height).min(props.content_height);

        let visible_range = VisibleRange {
            start: scroll_offset,
            end: visible_end,
            viewport_height: viewport_height as u16,
            available_width: content_area.width,
        };

        // Call user's render callback
        (props.render_content)(frame, content_area, visible_range);

        // Render scrollbar
        if let Some(scrollbar_area) = scrollbar_area {
            let scrollbar = style.scrollbar.build(ScrollbarOrientation::VerticalRight);
            let scrollbar_len = self.scrollbar_content_length(props.content_height);
            let mut scrollbar_state = ScrollbarState::new(scrollbar_len)
                .position(scroll_offset)
                .viewport_content_length(self.viewport_height_value());
            frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
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
        if !props.is_focused || props.content_height == 0 {
            return None;
        }

        let max_offset = self.max_offset(props.content_height);
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
            Some(offset) if offset != props.scroll_offset => {
                Some((props.on_scroll.as_ref())(offset))
            }
            _ => None,
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        self.render_with(
            frame,
            area,
            ScrollViewRenderProps {
                content_height: props.content_height,
                scroll_offset: props.scroll_offset,
                is_focused: props.is_focused,
                style: props.style,
                behavior: props.behavior,
                render_content: props.render_content,
            },
        );
    }
}

impl ComponentDebugState for ScrollView {
    fn debug_state(&self) -> Vec<ComponentDebugEntry> {
        vec![ComponentDebugEntry::new(
            "viewport_height",
            self.viewport_height.to_string(),
        )]
    }
}

impl<A, Ctx> InteractiveComponent<A, Ctx> for ScrollView {
    type Props<'a> = ScrollViewProps<'a, A>;

    fn update(
        &mut self,
        input: ComponentInput<'_, Ctx>,
        props: Self::Props<'_>,
    ) -> HandlerResponse<A> {
        let action = match input {
            ComponentInput::Command { name, .. } => {
                if !props.is_focused || props.content_height == 0 {
                    None
                } else {
                    let max_offset = self.max_offset(props.content_height);
                    let scroll_step = props.behavior.scroll_step.max(1) as isize;
                    let page_size = self.page_size(&props.behavior) as isize;
                    let next_offset = match name {
                        commands::NEXT | commands::DOWN => {
                            Some(self.apply_delta(props.scroll_offset, scroll_step, max_offset))
                        }
                        commands::PREV | commands::UP => {
                            Some(self.apply_delta(props.scroll_offset, -scroll_step, max_offset))
                        }
                        commands::PAGE_DOWN => {
                            Some(self.apply_delta(props.scroll_offset, page_size, max_offset))
                        }
                        commands::PAGE_UP => {
                            Some(self.apply_delta(props.scroll_offset, -page_size, max_offset))
                        }
                        commands::FIRST | commands::HOME => Some(0),
                        commands::LAST | commands::END => Some(max_offset),
                        _ => None,
                    };

                    match next_offset {
                        Some(offset) if offset != props.scroll_offset => {
                            Some((props.on_scroll.as_ref())(offset))
                        }
                        _ => None,
                    }
                }
            }
            ComponentInput::Key(key) => {
                <Self as Component<A>>::handle_event(self, &EventKind::Key(key), props)
                    .into_iter()
                    .next()
            }
            ComponentInput::Scroll {
                column,
                row,
                delta,
                modifiers,
            } => <Self as Component<A>>::handle_event(
                self,
                &EventKind::Scroll {
                    column,
                    row,
                    delta,
                    modifiers,
                },
                props,
            )
            .into_iter()
            .next(),
            _ => None,
        };

        match action {
            Some(action) => HandlerResponse::action(action),
            None => HandlerResponse::ignored(),
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        <Self as Component<A>>::render(self, frame, area, props);
    }
}

// ============================================================================
// Helper: LinesScroller
// ============================================================================

/// Simple wrapper for rendering pre-rendered lines in a ScrollView
///
/// # Example
///
/// ```ignore
/// use std::rc::Rc;
///
/// let lines: Vec<Line> = content.iter().map(|s| Line::raw(s)).collect();
/// let scroller = LinesScroller::new(&lines);
///
/// let mut scroll_view = ScrollView::new();
/// scroll_view.render(frame, area, ScrollViewProps {
///     content_height: scroller.content_height(),
///     scroll_offset,
///     is_focused: true,
///     style: ScrollViewStyle::default(),
///     behavior: ScrollViewBehavior::default(),
///     on_scroll: Rc::new(Action::Scroll),
///     render_content: &mut scroller.renderer(),
/// });
/// ```
pub struct LinesScroller<'a> {
    lines: &'a [Line<'a>],
    style: Style,
}

impl<'a> LinesScroller<'a> {
    /// Create a new LinesScroller with the given lines
    pub fn new(lines: &'a [Line<'a>]) -> Self {
        Self {
            lines,
            style: Style::default(),
        }
    }

    /// Set the base text style
    pub fn with_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Get the total content height
    pub fn content_height(&self) -> usize {
        self.lines.len()
    }

    /// Get a render callback for use with ScrollView
    pub fn renderer(&self) -> impl FnMut(&mut Frame, Rect, VisibleRange) + use<'_, 'a> {
        move |frame: &mut Frame, area: Rect, range: VisibleRange| {
            let visible_lines: Vec<Line<'a>> = self
                .lines
                .iter()
                .skip(range.start)
                .take(range.end.saturating_sub(range.start))
                .cloned()
                .collect();

            let paragraph = Paragraph::new(visible_lines).style(self.style);
            frame.render_widget(paragraph, area);
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

    #[test]
    fn test_scroll_down_action() {
        let mut view = ScrollView::new();
        let lines = make_lines(5);
        let scroller = LinesScroller::new(&lines);
        let mut harness = RenderHarness::new(20, 3);

        // Render once to set viewport height
        harness.render_to_string_plain(|frame| {
            <ScrollView as Component<TestAction>>::render(
                &mut view,
                frame,
                frame.area(),
                ScrollViewProps {
                    content_height: scroller.content_height(),
                    scroll_offset: 0,
                    is_focused: true,
                    style: ScrollViewStyle::borderless(),
                    behavior: ScrollViewBehavior::default(),
                    on_scroll: Rc::new(TestAction::ScrollTo),
                    render_content: &mut scroller.renderer(),
                },
            );
        });

        let mut noop_render = |_: &mut Frame, _: Rect, _: VisibleRange| {};
        let actions: Vec<_> = view
            .handle_event(
                &EventKind::Key(key("j")),
                ScrollViewProps {
                    content_height: lines.len(),
                    scroll_offset: 0,
                    is_focused: true,
                    style: ScrollViewStyle::borderless(),
                    behavior: ScrollViewBehavior::default(),
                    on_scroll: Rc::new(TestAction::ScrollTo),
                    render_content: &mut noop_render,
                },
            )
            .into_iter()
            .collect();

        assert_eq!(actions, vec![TestAction::ScrollTo(1)]);
    }

    #[test]
    fn test_page_down_action() {
        let mut view = ScrollView::new();
        let lines = make_lines(10);
        let scroller = LinesScroller::new(&lines);
        let mut harness = RenderHarness::new(20, 4);

        harness.render_to_string_plain(|frame| {
            <ScrollView as Component<TestAction>>::render(
                &mut view,
                frame,
                frame.area(),
                ScrollViewProps {
                    content_height: scroller.content_height(),
                    scroll_offset: 0,
                    is_focused: true,
                    style: ScrollViewStyle::borderless(),
                    behavior: ScrollViewBehavior::default(),
                    on_scroll: Rc::new(TestAction::ScrollTo),
                    render_content: &mut scroller.renderer(),
                },
            );
        });

        let mut noop_render = |_: &mut Frame, _: Rect, _: VisibleRange| {};
        let page_down = KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE);
        let actions: Vec<_> = view
            .handle_event(
                &EventKind::Key(page_down),
                ScrollViewProps {
                    content_height: lines.len(),
                    scroll_offset: 0,
                    is_focused: true,
                    style: ScrollViewStyle::borderless(),
                    behavior: ScrollViewBehavior::default(),
                    on_scroll: Rc::new(TestAction::ScrollTo),
                    render_content: &mut noop_render,
                },
            )
            .into_iter()
            .collect();

        assert_eq!(actions, vec![TestAction::ScrollTo(4)]);
    }

    #[test]
    fn test_scroll_wheel_action() {
        let mut view = ScrollView::new();
        let lines = make_lines(5);
        let scroller = LinesScroller::new(&lines);
        let mut harness = RenderHarness::new(20, 3);

        harness.render_to_string_plain(|frame| {
            <ScrollView as Component<TestAction>>::render(
                &mut view,
                frame,
                frame.area(),
                ScrollViewProps {
                    content_height: scroller.content_height(),
                    scroll_offset: 1,
                    is_focused: true,
                    style: ScrollViewStyle::borderless(),
                    behavior: ScrollViewBehavior::default(),
                    on_scroll: Rc::new(TestAction::ScrollTo),
                    render_content: &mut scroller.renderer(),
                },
            );
        });

        let mut noop_render = |_: &mut Frame, _: Rect, _: VisibleRange| {};
        let actions: Vec<_> = view
            .handle_event(
                &EventKind::Scroll {
                    column: 0,
                    row: 0,
                    delta: -1,
                    modifiers: KeyModifiers::NONE,
                },
                ScrollViewProps {
                    content_height: lines.len(),
                    scroll_offset: 1,
                    is_focused: true,
                    style: ScrollViewStyle::borderless(),
                    behavior: ScrollViewBehavior::default(),
                    on_scroll: Rc::new(TestAction::ScrollTo),
                    render_content: &mut noop_render,
                },
            )
            .into_iter()
            .collect();

        assert_eq!(actions, vec![TestAction::ScrollTo(0)]);
    }

    #[test]
    fn test_render_respects_offset() {
        let mut view = ScrollView::new();
        let lines = make_lines(6);
        let scroller = LinesScroller::new(&lines);
        let mut harness = RenderHarness::new(20, 3);

        let output = harness.render_to_string_plain(|frame| {
            <ScrollView as Component<TestAction>>::render(
                &mut view,
                frame,
                frame.area(),
                ScrollViewProps {
                    content_height: scroller.content_height(),
                    scroll_offset: 2,
                    is_focused: true,
                    style: ScrollViewStyle::borderless(),
                    behavior: ScrollViewBehavior::default(),
                    on_scroll: Rc::new(TestAction::ScrollTo),
                    render_content: &mut scroller.renderer(),
                },
            );
        });

        assert!(output.contains("Line 2"));
        assert!(!output.contains("Line 0"));
    }

    #[test]
    fn test_lines_scroller_content_height() {
        let lines = make_lines(10);
        let scroller = LinesScroller::new(&lines);
        assert_eq!(scroller.content_height(), 10);
    }
}
