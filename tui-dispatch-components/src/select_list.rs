//! Scrollable selection list component

use crossterm::event::KeyCode;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, List, ListItem, ListState, Scrollbar, ScrollbarOrientation, ScrollbarState,
    },
    Frame,
};
use tui_dispatch_core::{Component, EventKind};

/// Props for SelectList component
pub struct SelectListProps<'a, A> {
    /// Items to display
    pub items: &'a [String],
    /// Currently selected index
    pub selected: usize,
    /// Whether this component has focus
    pub is_focused: bool,
    /// Whether to show border (default: true)
    pub show_border: bool,
    /// Horizontal padding
    pub padding_x: u16,
    /// Vertical padding
    pub padding_y: u16,
    /// Query string to highlight in items (case-insensitive)
    pub highlight_query: Option<&'a str>,
    /// Callback to create action when selection changes
    pub on_select: fn(usize) -> A,
}

/// A scrollable selection list with keyboard navigation
///
/// Handles j/k/up/down for navigation and enter for selection.
/// Renders with highlight on the selected item.
#[derive(Default)]
pub struct SelectList {
    /// Scroll offset for viewport
    scroll_offset: usize,
}

/// Highlight matching characters in text (case-insensitive)
fn highlight_matches(text: &str, query: &str) -> Vec<Span<'static>> {
    if query.is_empty() {
        return vec![Span::raw(text.to_string())];
    }

    let text_lower = text.to_lowercase();
    let query_lower = query.to_lowercase();

    let mut spans = Vec::new();
    let mut last_end = 0;

    // Find all occurrences of query in text
    for (start, _) in text_lower.match_indices(&query_lower) {
        // Add non-matching part before this match
        if start > last_end {
            spans.push(Span::raw(text[last_end..start].to_string()));
        }

        // Add matching part with highlight (yellow bold)
        let end = start + query.len();
        let matched = &text[start..end];
        let style = Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD);
        spans.push(Span::styled(matched.to_string(), style));

        last_end = end;
    }

    // Add remaining part after last match
    if last_end < text.len() {
        spans.push(Span::raw(text[last_end..].to_string()));
    }

    if spans.is_empty() {
        vec![Span::raw(text.to_string())]
    } else {
        spans
    }
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
        if !props.is_focused || props.items.is_empty() {
            return None;
        }

        let len = props.items.len();

        match event {
            EventKind::Key(key) => match key.code {
                // Navigate down
                KeyCode::Char('j') | KeyCode::Down => {
                    let new_idx = (props.selected + 1).min(len.saturating_sub(1));
                    if new_idx != props.selected {
                        Some((props.on_select)(new_idx))
                    } else {
                        None
                    }
                }
                // Navigate up
                KeyCode::Char('k') | KeyCode::Up => {
                    let new_idx = props.selected.saturating_sub(1);
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
        // Apply padding
        let content_area = Rect {
            x: area.x + props.padding_x,
            y: area.y + props.padding_y,
            width: area.width.saturating_sub(props.padding_x * 2),
            height: area.height.saturating_sub(props.padding_y * 2),
        };

        // Calculate viewport height (account for borders if shown)
        let border_offset = if props.show_border { 2 } else { 0 };
        let viewport_height = content_area.height.saturating_sub(border_offset) as usize;

        // Ensure selected item is visible
        self.ensure_visible(props.selected, viewport_height);

        // Build list items with selection marker and highlight
        let items: Vec<ListItem> = props
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let is_selected = i == props.selected;
                let prefix = if is_selected { "> " } else { "  " };

                let line = if let Some(query) = props.highlight_query {
                    // Build line with highlighted matches
                    let mut spans = vec![Span::raw(prefix)];
                    spans.extend(highlight_matches(item, query));
                    Line::from(spans)
                } else {
                    Line::raw(format!("{prefix}{item}"))
                };

                let style = if is_selected {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(line).style(style)
            })
            .collect();

        // Create the list widget
        let mut list = List::new(items).highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

        if props.show_border {
            list = list.block(Block::default().borders(Borders::ALL).border_style(
                if props.is_focused {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ));
        }

        // Use ListState to handle scroll offset
        let mut state = ListState::default().with_selected(Some(props.selected));
        *state.offset_mut() = self.scroll_offset;

        frame.render_stateful_widget(list, content_area, &mut state);

        // Render scrollbar if content exceeds viewport
        if props.items.len() > viewport_height {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .track_symbol(Some("│"))
                .thumb_symbol("█");

            // Use selected index for position - shows where selection is in full list
            let mut scrollbar_state =
                ScrollbarState::new(props.items.len()).position(props.selected);

            // Render scrollbar in the inner area (account for border if shown)
            let scrollbar_area = if props.show_border {
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

    fn make_items() -> Vec<String> {
        vec!["Item 0".into(), "Item 1".into(), "Item 2".into()]
    }

    #[test]
    fn test_navigate_down() {
        let mut list = SelectList::new();
        let items = make_items();
        let props = SelectListProps {
            items: &items,
            selected: 0,
            is_focused: true,
            show_border: true,
            padding_x: 0,
            padding_y: 0,
            highlight_query: None,
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
            selected: 2,
            is_focused: true,
            show_border: true,
            padding_x: 0,
            padding_y: 0,
            highlight_query: None,
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
            selected: 0,
            is_focused: true,
            show_border: true,
            padding_x: 0,
            padding_y: 0,
            highlight_query: None,
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
            selected: 2,
            is_focused: true,
            show_border: true,
            padding_x: 0,
            padding_y: 0,
            highlight_query: None,
            on_select: TestAction::Select,
        };
        let actions: Vec<_> = list
            .handle_event(&EventKind::Key(key("j")), props)
            .into_iter()
            .collect();
        assert!(actions.is_empty());
    }

    #[test]
    fn test_unfocused_ignores_events() {
        let mut list = SelectList::new();
        let items = make_items();
        let props = SelectListProps {
            items: &items,
            selected: 0,
            is_focused: false,
            show_border: true,
            padding_x: 0,
            padding_y: 0,
            highlight_query: None,
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
            selected: 1,
            is_focused: true,
            show_border: true,
            padding_x: 0,
            padding_y: 0,
            highlight_query: None,
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
                selected: 1,
                is_focused: true,
                show_border: true,
                padding_x: 0,
                padding_y: 0,
                highlight_query: None,
                on_select: |_| (),
            };
            list.render(frame, frame.area(), props);
        });

        assert!(output.contains("Item 0"));
        assert!(output.contains("Item 1"));
        assert!(output.contains("Item 2"));
    }
}
