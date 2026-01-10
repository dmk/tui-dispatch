//! Scrollable selection list component

use crossterm::event::KeyCode;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState},
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
        // Calculate viewport height (area minus borders)
        let viewport_height = area.height.saturating_sub(2) as usize;

        // Ensure selected item is visible
        self.ensure_visible(props.selected, viewport_height);

        // Build list items
        let items: Vec<ListItem> = props
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let style = if i == props.selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(Line::raw(item.as_str())).style(style)
            })
            .collect();

        // Create the list widget
        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(if props.is_focused {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    }),
            )
            .highlight_style(
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD),
            );

        // Use ListState to handle scroll offset
        let mut state = ListState::default().with_selected(Some(props.selected));
        *state.offset_mut() = self.scroll_offset;

        frame.render_stateful_widget(list, area, &mut state);
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
                on_select: |_| (),
            };
            list.render(frame, frame.area(), props);
        });

        assert!(output.contains("Item 0"));
        assert!(output.contains("Item 1"));
        assert!(output.contains("Item 2"));
    }
}
