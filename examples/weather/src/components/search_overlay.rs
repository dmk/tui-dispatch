use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Color,
};
use tui_dispatch::EventKind;
use tui_dispatch_components::{
    ModalStyle, SelectList, SelectListProps, TextInput, TextInputProps, centered_rect, render_modal,
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

    fn result_items(results: &[Location]) -> Vec<String> {
        results
            .iter()
            .map(|location| location.name.clone())
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
                    let items = Self::result_items(props.results);
                    let list_props = SelectListProps {
                        items: &items,
                        selected: props.selected,
                        is_focused: true,
                        show_border: false,
                        padding_x: 1,
                        padding_y: 1,
                        highlight_query: None,
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
            show_border: false,
            bg_color: None,
            padding_x: 0,
            padding_y: 1,
            on_change: props.on_query_change,
            on_submit: props.on_query_submit,
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

        // Render modal with dimmed background
        let modal_area = centered_rect(60, 12, area);
        render_modal(
            frame,
            modal_area,
            &ModalStyle::with_bg(Color::Rgb(35, 35, 45)),
        );

        let chunks = Layout::vertical([
            Constraint::Length(3), // Input
            Constraint::Min(1),    // Results
        ])
        .split(modal_area);

        // Input with padding and lighter background
        let input_props = TextInputProps {
            value: props.query,
            placeholder: "Search for a city...",
            is_focused: props.is_focused,
            show_border: false,
            bg_color: Some(Color::Rgb(50, 50, 60)),
            padding_x: 1,
            padding_y: 1,
            on_change: props.on_query_change,
            on_submit: props.on_query_submit,
        };
        self.input.render(frame, chunks[0], input_props);

        let items = Self::result_items(props.results);
        let list_props = SelectListProps {
            items: &items,
            selected: props.selected,
            is_focused: props.is_focused,
            show_border: false,
            padding_x: 1,
            padding_y: 1,
            highlight_query: if props.query.is_empty() {
                None
            } else {
                Some(props.query)
            },
            on_select: props.on_select,
        };
        self.list.render(frame, chunks[1], list_props);
    }
}
