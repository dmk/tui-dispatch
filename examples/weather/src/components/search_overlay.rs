use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
};
use tui_dispatch::EventKind;
use tui_dispatch_components::{
    InputStyle, Line, ListBehavior, ListStyle, ModalStyle, Padding, ScrollbarStyle, SelectList,
    SelectListProps, SelectionStyle, TextInput, TextInputProps, centered_rect, highlight_substring,
    render_modal,
};

use super::Component;
use crate::action::Action;
use crate::state::Location;

pub struct SearchOverlay {
    input: TextInput,
    list: SelectList,
    was_open: bool,
}

pub struct SearchOverlayProps<'a> {
    pub query: &'a str,
    pub results: &'a [Location],
    pub selected: usize,
    pub is_focused: bool,
    #[allow(unused)]
    pub error: Option<&'a str>,
    // Action constructors
    pub on_query_change: fn(String) -> Action,
    pub on_query_submit: fn(String) -> Action,
    pub on_select: fn(usize) -> Action,
}

impl Default for SearchOverlay {
    fn default() -> Self {
        Self {
            input: TextInput::new(),
            list: SelectList::new(),
            was_open: false,
        }
    }
}

impl SearchOverlay {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_open(&mut self, is_open: bool) {
        if is_open && !self.was_open {
            self.reset();
        }
        self.was_open = is_open;
    }

    fn reset(&mut self) {
        self.input = TextInput::new();
        self.list = SelectList::new();
    }

    /// Build list items with query highlighting
    fn result_items(results: &[Location], query: &str) -> Vec<Line<'static>> {
        let base = Style::default().fg(Color::Reset);
        let highlight = Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD);

        results
            .iter()
            .map(|location| highlight_substring(&location.name, query, base, highlight))
            .collect()
    }
}

impl Component<Action> for SearchOverlay {
    type Props<'a> = SearchOverlayProps<'a>;

    fn handle_event(
        &mut self,
        event: &EventKind,
        props: Self::Props<'_>,
    ) -> impl IntoIterator<Item = Action> {
        if !props.is_focused {
            return Vec::new();
        }

        let EventKind::Key(key) = event else {
            return Vec::new();
        };

        // Handle special keys first
        match key.code {
            KeyCode::Esc => return vec![Action::SearchClose],
            KeyCode::Enter => {
                // If we have results, confirm selection; otherwise submit query
                if !props.results.is_empty() {
                    return vec![Action::SearchConfirm];
                }
                return vec![(props.on_query_submit)(props.query.to_string())];
            }
            // Up/down always navigate the list (if results exist)
            KeyCode::Down | KeyCode::Up => {
                if !props.results.is_empty() {
                    let items = Self::result_items(props.results, props.query);
                    let list_props = SelectListProps {
                        items: &items,
                        count: items.len(),
                        selected: props.selected,
                        is_focused: true,
                        style: ListStyle {
                            border: None,
                            padding: Padding::xy(1, 1),
                            bg: None,
                            fg: None,
                            selection: SelectionStyle::default(),
                            scrollbar: ScrollbarStyle::default(),
                        },
                        behavior: ListBehavior::default(),
                        on_select: props.on_select,
                    };
                    return self
                        .list
                        .handle_event(event, list_props)
                        .into_iter()
                        .collect();
                }
                return Vec::new();
            }
            _ => {}
        }

        // All other keys go to the input
        let input_props = TextInputProps {
            value: props.query,
            placeholder: "Search for a city...",
            is_focused: true,
            style: InputStyle {
                border: None,
                padding: Padding::new(1, 0, 1, 0),
                bg: None,
                fg: None,
                placeholder_style: None,
                cursor_style: None,
            },
            on_change: props.on_query_change,
            on_submit: props.on_query_submit,
            render_action: Some(Action::Render),
        };

        self.input
            .handle_event(event, input_props)
            .into_iter()
            .collect()
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        if area.width < 20 || area.height < 8 {
            return;
        }

        // Render modal with dimmed background - returns content area
        let modal_area = centered_rect(60, 12, area);
        let content_area = render_modal(
            frame,
            modal_area,
            &ModalStyle {
                bg: Some(Color::Rgb(35, 35, 45)),
                ..Default::default()
            },
        );

        let chunks = Layout::vertical([
            Constraint::Length(3), // Input
            Constraint::Min(1),    // Results
        ])
        .split(content_area);

        // Input with lighter background
        let input_props = TextInputProps {
            value: props.query,
            placeholder: "Search for a city...",
            is_focused: props.is_focused,
            style: InputStyle {
                border: None,
                padding: Padding::all(1),
                bg: Some(Color::Rgb(50, 50, 60)),
                fg: None,
                placeholder_style: None,
                cursor_style: None,
            },
            on_change: props.on_query_change,
            on_submit: props.on_query_submit,
            render_action: Some(Action::Render),
        };
        self.input.render(frame, chunks[0], input_props);

        // Build items with highlighting
        let items = Self::result_items(props.results, props.query);
        let list_props = SelectListProps {
            items: &items,
            count: items.len(),
            selected: props.selected,
            is_focused: props.is_focused,
            style: ListStyle {
                border: None,
                padding: Padding::all(1),
                bg: None,
                fg: None,
                selection: SelectionStyle::default(),
                scrollbar: ScrollbarStyle::default(),
            },
            behavior: ListBehavior::default(),
            on_select: props.on_select,
        };
        self.list.render(frame, chunks[1], list_props);
    }
}
