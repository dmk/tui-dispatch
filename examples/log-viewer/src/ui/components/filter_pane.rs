use std::collections::BTreeSet;
use std::rc::Rc;

use crossterm::event::KeyCode;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use tui_dispatch::prelude::HandlerResponse;
use tui_dispatch_components::{
    commands, highlight_substring, BorderStyle, ComponentDebugEntry, ComponentDebugState,
    ComponentInput, InteractiveComponent, SelectList, SelectListBehavior, SelectListProps,
    SelectListRenderProps, SelectListStyle, TextInput, TextInputProps, TextInputRenderProps,
    TextInputStyle,
};

pub type FilterToggleCallback<A> = Rc<dyn Fn(String) -> A>;

pub struct FilterPaneProps<'a, A> {
    pub title: &'a str,
    pub options: &'a [String],
    pub active: &'a BTreeSet<String>,
    pub is_focused: bool,
    pub on_toggle: FilterToggleCallback<A>,
}

#[derive(Default)]
pub struct FilterPane {
    input: TextInput,
    list: SelectList,
    query: String,
    highlighted: usize,
}

impl FilterPane {
    pub fn new() -> Self {
        Self::default()
    }

    fn filtered_options<'a>(&self, options: &'a [String]) -> Vec<&'a String> {
        if self.query.is_empty() {
            return options.iter().collect();
        }

        let query = self.query.to_ascii_lowercase();
        options
            .iter()
            .filter(|option| option.to_ascii_lowercase().contains(&query))
            .collect()
    }

    fn clamp_highlight(&mut self, filtered_len: usize) {
        if filtered_len == 0 {
            self.highlighted = 0;
        } else {
            self.highlighted = self.highlighted.min(filtered_len.saturating_sub(1));
        }
    }

    fn toggle_current<A>(&self, props: &FilterPaneProps<'_, A>) -> Option<A> {
        let filtered = self.filtered_options(props.options);
        let current = filtered.get(self.highlighted)?;
        Some((props.on_toggle.as_ref())((*current).clone()))
    }

    fn render_lines<A>(&self, props: &FilterPaneProps<'_, A>) -> Vec<Line<'static>> {
        self.filtered_options(props.options)
            .into_iter()
            .map(|option| render_filter_option(option, props.active, &self.query))
            .collect()
    }

    fn handle_query_input<A, Ctx>(
        &mut self,
        input: ComponentInput<'_, Ctx>,
        props: &FilterPaneProps<'_, A>,
    ) -> HandlerResponse<A> {
        #[derive(Clone, Debug, PartialEq)]
        enum LocalAction {
            QueryChanged(String),
        }

        let response = <TextInput as InteractiveComponent<LocalAction, Ctx>>::update(
            &mut self.input,
            input,
            TextInputProps {
                value: &self.query,
                placeholder: "type to narrow",
                is_focused: true,
                style: input_style(),
                on_change: Rc::new(LocalAction::QueryChanged),
                on_submit: Rc::new(LocalAction::QueryChanged),
                on_cursor_move: None,
                on_cancel: None,
            },
        );

        let mut changed = response.needs_render;
        for action in response.actions {
            let LocalAction::QueryChanged(next) = action;
            self.query = next;
            self.clamp_highlight(self.filtered_options(props.options).len());
            changed = true;
        }

        if changed {
            HandlerResponse::ignored().with_render().with_consumed(true)
        } else if response.consumed {
            HandlerResponse::ignored().with_consumed(true)
        } else {
            HandlerResponse::ignored()
        }
    }

    fn handle_list_input<A, Ctx>(
        &mut self,
        input: ComponentInput<'_, Ctx>,
        props: &FilterPaneProps<'_, A>,
    ) -> HandlerResponse<A> {
        #[derive(Clone, Debug, PartialEq)]
        enum LocalAction {
            Highlight(usize),
        }

        let lines = self.render_lines(props);
        let response = <SelectList as InteractiveComponent<LocalAction, Ctx>>::update(
            &mut self.list,
            input,
            SelectListProps {
                items: &lines,
                count: lines.len(),
                selected: self.highlighted,
                is_focused: true,
                style: list_style(),
                behavior: SelectListBehavior::default(),
                on_select: Rc::new(LocalAction::Highlight),
                render_item: &clone_line,
            },
        );

        if let Some(action) = response.actions.into_iter().next() {
            let LocalAction::Highlight(index) = action;
            self.highlighted = index;
            return HandlerResponse::ignored().with_render().with_consumed(true);
        }

        if response.consumed || response.needs_render {
            HandlerResponse::ignored().with_render().with_consumed(true)
        } else {
            HandlerResponse::ignored()
        }
    }
}

impl ComponentDebugState for FilterPane {
    fn debug_state(&self) -> Vec<ComponentDebugEntry> {
        vec![
            ComponentDebugEntry::new("query", self.query.clone()),
            ComponentDebugEntry::new("highlighted", self.highlighted.to_string()),
        ]
    }
}

impl<A, Ctx> InteractiveComponent<A, Ctx> for FilterPane {
    type Props<'a> = FilterPaneProps<'a, A>;

    fn update(
        &mut self,
        input: ComponentInput<'_, Ctx>,
        props: Self::Props<'_>,
    ) -> HandlerResponse<A> {
        if !props.is_focused {
            return HandlerResponse::ignored();
        }

        match input {
            command @ ComponentInput::Command { name, .. } => match name {
                commands::TOGGLE | commands::SELECT | commands::CONFIRM => self
                    .toggle_current(&props)
                    .map(HandlerResponse::action)
                    .unwrap_or_else(HandlerResponse::ignored),
                commands::NEXT
                | commands::DOWN
                | commands::PREV
                | commands::UP
                | commands::FIRST
                | commands::HOME
                | commands::LAST
                | commands::END => self.handle_list_input::<A, Ctx>(command, &props),
                _ => self.handle_query_input::<A, Ctx>(command, &props),
            },
            ComponentInput::Key(key) => match key.code {
                KeyCode::Enter | KeyCode::Char(' ') => self
                    .toggle_current(&props)
                    .map(HandlerResponse::action)
                    .unwrap_or_else(HandlerResponse::ignored),
                KeyCode::Up | KeyCode::Down | KeyCode::Home | KeyCode::End => {
                    self.handle_list_input::<A, Ctx>(ComponentInput::Key(key), &props)
                }
                _ => self.handle_query_input::<A, Ctx>(ComponentInput::Key(key), &props),
            },
            _ => HandlerResponse::ignored(),
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        let block = Block::default()
            .title(format!(" {} ", props.title))
            .borders(Borders::ALL)
            .border_style(border_style(props.is_focused))
            .style(Style::default().bg(Color::Rgb(11, 15, 19)));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let [input_area, list_area] =
            Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).areas(inner);

        self.input.render_widget(
            frame,
            input_area,
            TextInputRenderProps {
                value: &self.query,
                placeholder: "type to narrow",
                is_focused: props.is_focused,
                style: input_style(),
            },
        );

        let lines = self.render_lines(&props);
        self.clamp_highlight(lines.len());

        if lines.is_empty() {
            frame.render_widget(
                Paragraph::new("no matches").style(Style::default().fg(Color::Rgb(124, 132, 150))),
                list_area,
            );
            return;
        }

        self.list.render_widget(
            frame,
            list_area,
            SelectListRenderProps {
                items: &lines,
                count: lines.len(),
                selected: self.highlighted,
                is_focused: props.is_focused,
                style: list_style(),
                behavior: SelectListBehavior::default(),
                render_item: &clone_line,
            },
        );
    }
}

fn render_filter_option(option: &str, active: &BTreeSet<String>, query: &str) -> Line<'static> {
    let enabled = active.contains(option);
    let mut spans = vec![Span::styled(
        if enabled { "[x] " } else { "[ ] " },
        Style::default().fg(if enabled {
            Color::Rgb(146, 196, 125)
        } else {
            Color::Rgb(124, 132, 150)
        }),
    )];

    let label = highlight_substring(
        option,
        query,
        Style::default().fg(if enabled {
            Color::Rgb(229, 229, 227)
        } else {
            Color::Rgb(162, 173, 183)
        }),
        Style::default()
            .fg(Color::Rgb(231, 179, 94))
            .add_modifier(Modifier::BOLD),
    );
    spans.extend(label.spans);
    Line::from(spans)
}

fn clone_line(line: &Line<'static>) -> Line<'static> {
    line.clone()
}

fn border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::Rgb(231, 179, 94))
    } else {
        Style::default().fg(Color::Rgb(64, 80, 92))
    }
}

fn input_style() -> TextInputStyle {
    let mut style = TextInputStyle::default();
    style.base.border = Some(BorderStyle {
        style: Style::default().fg(Color::Rgb(57, 70, 82)),
        focused_style: Some(Style::default().fg(Color::Rgb(123, 203, 216))),
        ..BorderStyle::all()
    });
    style.base.bg = Some(Color::Rgb(17, 23, 28));
    style.cursor_style = Some(Style::default().bg(Color::Rgb(231, 179, 94)));
    style.placeholder_style = Some(Style::default().fg(Color::Rgb(97, 108, 120)));
    style
}

fn list_style() -> SelectListStyle {
    let mut style = SelectListStyle::minimal();
    style.base.bg = Some(Color::Rgb(11, 15, 19));
    style.base.fg = Some(Color::Rgb(229, 229, 227));
    style.selection.style = Some(Style::default().bg(Color::Rgb(22, 31, 38)));
    style.selection.marker = Some("› ");
    style
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use tui_dispatch::key;
    use tui_dispatch_components::{ComponentHost, ComponentInput};

    use super::*;
    use crate::{Action, AppBindingContext, AppState, RouteId};

    fn level_props(state: &AppState) -> FilterPaneProps<'_, Action> {
        FilterPaneProps {
            title: "Levels",
            options: &state.level_options,
            active: &state.active_levels,
            is_focused: true,
            on_toggle: Rc::new(Action::ToggleLevel),
        }
    }

    fn source_props(state: &AppState) -> FilterPaneProps<'_, Action> {
        FilterPaneProps {
            title: "Tags",
            options: &state.tag_options,
            active: &state.active_tags,
            is_focused: true,
            on_toggle: Rc::new(Action::ToggleTag),
        }
    }

    #[test]
    fn typing_only_changes_local_query_state() {
        let mut pane = FilterPane::new();
        let active = BTreeSet::from(["error".to_string()]);
        let options = vec!["error".to_string(), "warn".to_string()];

        let response = <FilterPane as InteractiveComponent<Action, AppBindingContext>>::update(
            &mut pane,
            ComponentInput::Key(key("e")),
            FilterPaneProps {
                title: "Levels",
                options: &options,
                active: &active,
                is_focused: true,
                on_toggle: Rc::new(Action::ToggleLevel),
            },
        );

        assert!(response.actions.is_empty());
        assert!(response.needs_render);
        assert_eq!(
            pane.debug_state(),
            vec![
                ComponentDebugEntry::new("query", "e"),
                ComponentDebugEntry::new("highlighted", "0"),
            ]
        );
    }

    #[test]
    fn mounted_instances_keep_independent_query_state() {
        let host = ComponentHost::<AppState, Action, RouteId, AppBindingContext>::new();
        let levels = host.mount(FilterPane::new, level_props);
        let sources = host.mount(FilterPane::new, source_props);

        let mut state = AppState::new("stdin");
        state.tag_options = vec!["service:api".to_string(), "target:worker".to_string()];
        state.active_tags = state.tag_options.iter().cloned().collect();

        host.update(levels, ComponentInput::Key(key("e")), &state);
        host.update(sources, ComponentInput::Key(key("w")), &state);

        let info = host.mounted_components();
        assert_eq!(
            info[0].debug_state[0],
            ComponentDebugEntry::new("query", "e")
        );
        assert_eq!(
            info[1].debug_state[0],
            ComponentDebugEntry::new("query", "w")
        );
    }
}
