//! Scrollable selection list component

use crossterm::event::KeyCode;
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};
use tui_dispatch_core::{Component, EventKind};

use crate::style::{BorderStyle, ComponentStyle, Padding, SelectionStyle};

/// Unified styling for SelectList
#[derive(Debug, Clone)]
pub struct ListStyle {
    /// Border configuration (None = no border)
    pub border: Option<BorderStyle>,
    /// Padding inside the component
    pub padding: Padding,
    /// Background color
    pub bg: Option<ratatui::style::Color>,
    /// Foreground (text) color for non-selected items
    pub fg: Option<ratatui::style::Color>,
    /// Selection indication styling
    pub selection: SelectionStyle,
}

impl Default for ListStyle {
    fn default() -> Self {
        Self {
            border: Some(BorderStyle::default()),
            padding: Padding::default(),
            bg: None,
            fg: Some(ratatui::style::Color::Reset),
            selection: SelectionStyle::default(),
        }
    }
}

impl ListStyle {
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
            fg: Some(ratatui::style::Color::Reset),
            selection: SelectionStyle::default(),
        }
    }
}

impl ComponentStyle for ListStyle {
    fn border(&self) -> Option<&BorderStyle> {
        self.border.as_ref()
    }
    fn padding(&self) -> &Padding {
        &self.padding
    }
    fn bg(&self) -> Option<ratatui::style::Color> {
        self.bg
    }
}

/// Behavior configuration for SelectList
#[derive(Debug, Clone)]
pub struct ListBehavior {
    /// Show scrollbar when content exceeds viewport
    pub show_scrollbar: bool,
    /// Wrap navigation from last to first item (and vice versa)
    pub wrap_navigation: bool,
}

impl Default for ListBehavior {
    fn default() -> Self {
        Self {
            show_scrollbar: true,
            wrap_navigation: false,
        }
    }
}

/// Props for SelectList component
pub struct SelectListProps<'a, A> {
    /// Items to render - user provides pre-styled Lines
    pub items: &'a [Line<'a>],
    /// Total count (may differ from items.len() for virtual lists)
    pub count: usize,
    /// Currently selected index
    pub selected: usize,
    /// Whether this component has focus
    pub is_focused: bool,
    /// Unified styling
    pub style: ListStyle,
    /// Behavior configuration
    pub behavior: ListBehavior,
    /// Callback to create action when selection changes
    pub on_select: fn(usize) -> A,
}

impl<'a, A> SelectListProps<'a, A> {
    /// Create props with sensible defaults
    ///
    /// Sets `count` to `items.len()`, `is_focused` to `true`, and uses default style/behavior.
    pub fn new(items: &'a [Line<'a>], selected: usize, on_select: fn(usize) -> A) -> Self {
        Self {
            items,
            count: items.len(),
            selected,
            is_focused: true,
            style: ListStyle::default(),
            behavior: ListBehavior::default(),
            on_select,
        }
    }
}

/// A scrollable selection list with keyboard navigation
///
/// Handles j/k/up/down for navigation and enter for selection.
/// Users provide pre-styled `Line` items for full control over rendering.
#[derive(Default)]
pub struct SelectList {
    /// Scroll offset for viewport
    scroll_offset: usize,
}

impl SelectList {
    /// Create a new SelectList
    pub fn new() -> Self {
        Self::default()
    }

    /// Ensure the selected index is visible within the viewport
    fn ensure_visible(&mut self, selected: usize, viewport_height: usize) {
        if viewport_height == 0 {
            return;
        }

        if selected < self.scroll_offset {
            self.scroll_offset = selected;
        } else if selected >= self.scroll_offset + viewport_height {
            self.scroll_offset = selected.saturating_sub(viewport_height - 1);
        }
    }
}

impl<A> Component<A> for SelectList {
    type Props<'a> = SelectListProps<'a, A>;

    fn handle_event(
        &mut self,
        event: &EventKind,
        props: Self::Props<'_>,
    ) -> impl IntoIterator<Item = A> {
        if !props.is_focused || props.count == 0 {
            return None;
        }

        let len = props.count;

        match event {
            EventKind::Key(key) => match key.code {
                // Navigate down
                KeyCode::Char('j') | KeyCode::Down => {
                    let new_idx = if props.behavior.wrap_navigation && props.selected == len - 1 {
                        0
                    } else {
                        (props.selected + 1).min(len.saturating_sub(1))
                    };
                    if new_idx != props.selected {
                        Some((props.on_select)(new_idx))
                    } else {
                        None
                    }
                }
                // Navigate up
                KeyCode::Char('k') | KeyCode::Up => {
                    let new_idx = if props.behavior.wrap_navigation && props.selected == 0 {
                        len.saturating_sub(1)
                    } else {
                        props.selected.saturating_sub(1)
                    };
                    if new_idx != props.selected {
                        Some((props.on_select)(new_idx))
                    } else {
                        None
                    }
                }
                // Jump to top
                KeyCode::Char('g') | KeyCode::Home => {
                    if props.selected != 0 {
                        Some((props.on_select)(0))
                    } else {
                        None
                    }
                }
                // Jump to bottom
                KeyCode::Char('G') | KeyCode::End => {
                    let last = len.saturating_sub(1);
                    if props.selected != last {
                        Some((props.on_select)(last))
                    } else {
                        None
                    }
                }
                // Select current (re-emit for confirmation actions)
                KeyCode::Enter => Some((props.on_select)(props.selected)),
                _ => None,
            },
            _ => None,
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        let style = &props.style;

        // Fill background if specified
        if let Some(bg) = style.bg {
            for y in area.y..area.y.saturating_add(area.height) {
                for x in area.x..area.x.saturating_add(area.width) {
                    frame.buffer_mut()[(x, y)].set_bg(bg);
                    frame.buffer_mut()[(x, y)].set_symbol(" ");
                }
            }
        }

        // Apply padding
        let content_area = Rect {
            x: area.x + style.padding.left,
            y: area.y + style.padding.top,
            width: area.width.saturating_sub(style.padding.horizontal()),
            height: area.height.saturating_sub(style.padding.vertical()),
        };

        // Calculate viewport height (account for borders if shown)
        let border_offset = if style.border.is_some() { 2 } else { 0 };
        let viewport_height = content_area.height.saturating_sub(border_offset) as usize;

        let render_selected = props.selected.min(props.items.len().saturating_sub(1));

        // Ensure selected item is visible
        if !props.items.is_empty() {
            self.ensure_visible(render_selected, viewport_height);
        }

        // Build list items with optional selection styling
        let items: Vec<ListItem> = props
            .items
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let is_selected = i == render_selected;

                // Apply selection styling unless disabled
                if style.selection.disabled {
                    ListItem::new(line.clone())
                } else {
                    // Build the line with optional marker
                    let display_line = if let Some(marker) = style.selection.marker {
                        let prefix = if is_selected {
                            marker
                        } else {
                            &"  "[..marker.len().min(2)]
                        };
                        let mut spans = vec![Span::raw(prefix)];
                        spans.extend(line.spans.iter().cloned());
                        Line::from(spans)
                    } else {
                        line.clone()
                    };

                    // Apply selection style (or base fg color for non-selected)
                    let item_style = if is_selected {
                        style.selection.style.unwrap_or_default()
                    } else {
                        let mut s = Style::default();
                        if let Some(fg) = style.fg {
                            s = s.fg(fg);
                        }
                        s
                    };

                    ListItem::new(display_line).style(item_style)
                }
            })
            .collect();

        // Create the list widget
        let highlight_style = if style.selection.disabled {
            Style::default()
        } else {
            style.selection.style.unwrap_or_default()
        };
        let mut list = List::new(items).highlight_style(highlight_style);

        if let Some(border) = &style.border {
            list = list.block(
                Block::default()
                    .borders(border.borders)
                    .border_style(border.style_for_focus(props.is_focused)),
            );
        }

        // Use ListState to handle scroll offset
        let selected = if props.items.is_empty() {
            None
        } else {
            Some(render_selected)
        };
        let mut state = ListState::default().with_selected(selected);
        *state.offset_mut() = self.scroll_offset;

        frame.render_stateful_widget(list, content_area, &mut state);

        // Render scrollbar if enabled and content exceeds viewport
        if props.behavior.show_scrollbar && props.count > viewport_height {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .track_symbol(Some("│"))
                .thumb_symbol("█")
                .style(Style::default().fg(tui_dispatch_core::Color::Cyan));

            let mut scrollbar_state = ScrollbarState::new(props.count).position(props.selected);

            // Render scrollbar in the inner area (account for border if shown)
            let scrollbar_area = if style.border.is_some() {
                Rect {
                    x: content_area.x,
                    y: content_area.y + 1,
                    width: content_area.width,
                    height: content_area.height.saturating_sub(2),
                }
            } else {
                content_area
            };

            frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tui_dispatch_core::testing::{key, RenderHarness};

    #[derive(Debug, Clone, PartialEq)]
    enum TestAction {
        Select(usize),
    }

    fn make_items() -> Vec<Line<'static>> {
        vec![
            Line::raw("Item 0"),
            Line::raw("Item 1"),
            Line::raw("Item 2"),
        ]
    }

    #[test]
    fn test_navigate_down() {
        let mut list = SelectList::new();
        let items = make_items();
        let props = SelectListProps {
            items: &items,
            count: items.len(),
            selected: 0,
            is_focused: true,
            style: ListStyle::default(),
            behavior: ListBehavior::default(),
            on_select: TestAction::Select,
        };

        let actions: Vec<_> = list
            .handle_event(&EventKind::Key(key("j")), props)
            .into_iter()
            .collect();

        assert_eq!(actions, vec![TestAction::Select(1)]);
    }

    #[test]
    fn test_navigate_up() {
        let mut list = SelectList::new();
        let items = make_items();
        let props = SelectListProps {
            items: &items,
            count: items.len(),
            selected: 2,
            is_focused: true,
            style: ListStyle::default(),
            behavior: ListBehavior::default(),
            on_select: TestAction::Select,
        };

        let actions: Vec<_> = list
            .handle_event(&EventKind::Key(key("k")), props)
            .into_iter()
            .collect();

        assert_eq!(actions, vec![TestAction::Select(1)]);
    }

    #[test]
    fn test_navigate_at_bounds() {
        let mut list = SelectList::new();
        let items = make_items();

        // At top, going up should not emit
        let props = SelectListProps {
            items: &items,
            count: items.len(),
            selected: 0,
            is_focused: true,
            style: ListStyle::default(),
            behavior: ListBehavior::default(),
            on_select: TestAction::Select,
        };
        let actions: Vec<_> = list
            .handle_event(&EventKind::Key(key("k")), props)
            .into_iter()
            .collect();
        assert!(actions.is_empty());

        // At bottom, going down should not emit
        let props = SelectListProps {
            items: &items,
            count: items.len(),
            selected: 2,
            is_focused: true,
            style: ListStyle::default(),
            behavior: ListBehavior::default(),
            on_select: TestAction::Select,
        };
        let actions: Vec<_> = list
            .handle_event(&EventKind::Key(key("j")), props)
            .into_iter()
            .collect();
        assert!(actions.is_empty());
    }

    #[test]
    fn test_wrap_navigation() {
        let mut list = SelectList::new();
        let items = make_items();

        // At top with wrap, going up should go to bottom
        let props = SelectListProps {
            items: &items,
            count: items.len(),
            selected: 0,
            is_focused: true,
            style: ListStyle::default(),
            behavior: ListBehavior {
                wrap_navigation: true,
                ..Default::default()
            },
            on_select: TestAction::Select,
        };
        let actions: Vec<_> = list
            .handle_event(&EventKind::Key(key("k")), props)
            .into_iter()
            .collect();
        assert_eq!(actions, vec![TestAction::Select(2)]);

        // At bottom with wrap, going down should go to top
        let props = SelectListProps {
            items: &items,
            count: items.len(),
            selected: 2,
            is_focused: true,
            style: ListStyle::default(),
            behavior: ListBehavior {
                wrap_navigation: true,
                ..Default::default()
            },
            on_select: TestAction::Select,
        };
        let actions: Vec<_> = list
            .handle_event(&EventKind::Key(key("j")), props)
            .into_iter()
            .collect();
        assert_eq!(actions, vec![TestAction::Select(0)]);
    }

    #[test]
    fn test_unfocused_ignores_events() {
        let mut list = SelectList::new();
        let items = make_items();
        let props = SelectListProps {
            items: &items,
            count: items.len(),
            selected: 0,
            is_focused: false,
            style: ListStyle::default(),
            behavior: ListBehavior::default(),
            on_select: TestAction::Select,
        };

        let actions: Vec<_> = list
            .handle_event(&EventKind::Key(key("j")), props)
            .into_iter()
            .collect();

        assert!(actions.is_empty());
    }

    #[test]
    fn test_enter_selects_current() {
        let mut list = SelectList::new();
        let items = make_items();
        let props = SelectListProps {
            items: &items,
            count: items.len(),
            selected: 1,
            is_focused: true,
            style: ListStyle::default(),
            behavior: ListBehavior::default(),
            on_select: TestAction::Select,
        };

        let actions: Vec<_> = list
            .handle_event(&EventKind::Key(key("enter")), props)
            .into_iter()
            .collect();

        assert_eq!(actions, vec![TestAction::Select(1)]);
    }

    #[test]
    fn test_render() {
        let mut render = RenderHarness::new(30, 10);
        let mut list = SelectList::new();
        let items = make_items();

        let output = render.render_to_string_plain(|frame| {
            let props = SelectListProps {
                items: &items,
                count: items.len(),
                selected: 1,
                is_focused: true,
                style: ListStyle::default(),
                behavior: ListBehavior::default(),
                on_select: |_| (),
            };
            list.render(frame, frame.area(), props);
        });

        assert!(output.contains("Item 0"));
        assert!(output.contains("Item 1"));
        assert!(output.contains("Item 2"));
    }

    #[test]
    fn test_render_without_selection_styling() {
        let mut render = RenderHarness::new(30, 10);
        let mut list = SelectList::new();
        let items = make_items();

        let output = render.render_to_string_plain(|frame| {
            let props = SelectListProps {
                items: &items,
                count: items.len(),
                selected: 1,
                is_focused: true,
                style: ListStyle {
                    selection: SelectionStyle::disabled(),
                    ..Default::default()
                },
                behavior: ListBehavior::default(),
                on_select: |_| (),
            };
            list.render(frame, frame.area(), props);
        });

        // Should render items without markers
        assert!(output.contains("Item 0"));
        assert!(output.contains("Item 1"));
        assert!(output.contains("Item 2"));
        // Should not contain selection marker
        assert!(!output.contains(">"));
    }
}
