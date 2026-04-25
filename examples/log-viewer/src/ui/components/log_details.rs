use std::collections::HashSet;
use std::rc::Rc;

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use serde_json::Value;
use tui_dispatch::prelude::HandlerResponse;
use tui_dispatch_components::{
    ComponentDebugEntry, ComponentDebugState, ComponentInput, InteractiveComponent, ScrollView,
    ScrollViewBehavior, ScrollViewProps, ScrollViewRenderProps, ScrollViewStyle, TreeNode,
    TreeNodeRender, TreeView, TreeViewBehavior, TreeViewProps, TreeViewRenderProps, TreeViewStyle,
    VisibleRange,
};

use crate::state::LogEntry;

pub struct LogDetailsProps<'a> {
    pub entry: Option<&'a LogEntry>,
    pub is_focused: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DetailsMode {
    Pretty,
    Tree,
}

pub struct LogDetails {
    pretty_view: ScrollView,
    tree_view: TreeView<String>,
    mode: DetailsMode,
    pretty_scroll: usize,
    expanded_ids: HashSet<String>,
    selected_node: Option<String>,
    current_seq: Option<u64>,
}

impl Default for LogDetails {
    fn default() -> Self {
        Self {
            pretty_view: ScrollView::new(),
            tree_view: TreeView::new(),
            mode: DetailsMode::Pretty,
            pretty_scroll: 0,
            expanded_ids: HashSet::new(),
            selected_node: None,
            current_seq: None,
        }
    }
}

impl LogDetails {
    pub fn new() -> Self {
        Self::default()
    }

    fn sync_entry(&mut self, entry: Option<&LogEntry>) {
        let next_seq = entry.map(|entry| entry.seq);
        if next_seq != self.current_seq {
            self.current_seq = next_seq;
            self.pretty_scroll = 0;
            self.mode = DetailsMode::Pretty;
            self.expanded_ids.clear();
            self.selected_node = None;

            if tree_available(entry) {
                self.expanded_ids.insert("$".to_string());
                self.selected_node = Some("$".to_string());
            }
        } else if !tree_available(entry) && self.mode == DetailsMode::Tree {
            self.mode = DetailsMode::Pretty;
        }
    }

    fn toggle_mode(&mut self, entry: Option<&LogEntry>) -> bool {
        if !tree_available(entry) {
            return false;
        }

        self.mode = match self.mode {
            DetailsMode::Pretty => DetailsMode::Tree,
            DetailsMode::Tree => DetailsMode::Pretty,
        };
        true
    }

    fn handle_scroll_input<A, Ctx>(
        &mut self,
        input: ComponentInput<'_, Ctx>,
        lines: &[Line<'static>],
    ) -> HandlerResponse<A> {
        #[derive(Clone, Debug, PartialEq)]
        enum LocalAction {
            ScrollTo(usize),
        }

        let mut render_content = |_: &mut Frame, _: Rect, _: VisibleRange| {};
        let response = <ScrollView as InteractiveComponent<LocalAction, Ctx>>::update(
            &mut self.pretty_view,
            input,
            ScrollViewProps {
                content_height: lines.len().max(1),
                scroll_offset: self.pretty_scroll,
                is_focused: true,
                style: scroll_style(),
                behavior: ScrollViewBehavior {
                    page_step: 8,
                    ..Default::default()
                },
                on_scroll: Rc::new(LocalAction::ScrollTo),
                render_content: &mut render_content,
            },
        );

        if let Some(action) = response.actions.into_iter().next() {
            let LocalAction::ScrollTo(offset) = action;
            self.pretty_scroll = offset;
            return HandlerResponse::ignored().with_render().with_consumed(true);
        }

        if response.consumed || response.needs_render {
            HandlerResponse::ignored().with_render().with_consumed(true)
        } else {
            HandlerResponse::ignored()
        }
    }

    fn handle_tree_input<A, Ctx>(
        &mut self,
        input: ComponentInput<'_, Ctx>,
        entry: &LogEntry,
    ) -> HandlerResponse<A> {
        #[derive(Clone, Debug, PartialEq)]
        enum LocalAction {
            Select(String),
            Toggle(String, bool),
        }

        #[allow(clippy::ptr_arg)]
        fn on_select(id: &String) -> LocalAction {
            LocalAction::Select(id.clone())
        }

        #[allow(clippy::ptr_arg)]
        fn on_toggle(id: &String, expand: bool) -> LocalAction {
            LocalAction::Toggle(id.clone(), expand)
        }

        let nodes = build_json_tree(entry.payload.as_ref());
        if nodes.is_empty() {
            return HandlerResponse::ignored();
        }

        let response = <TreeView<String> as InteractiveComponent<LocalAction, Ctx>>::update(
            &mut self.tree_view,
            input,
            TreeViewProps {
                nodes: &nodes,
                selected_id: self.selected_node.as_ref(),
                expanded_ids: &self.expanded_ids,
                is_focused: true,
                style: tree_style(),
                behavior: TreeViewBehavior {
                    wrap_navigation: false,
                    ..Default::default()
                },
                measure_node: None,
                column_padding: 1,
                on_select: Rc::new(on_select),
                on_toggle: Rc::new(on_toggle),
                render_node: &render_tree_node,
            },
        );

        if let Some(action) = response.actions.into_iter().next() {
            match action {
                LocalAction::Select(id) => self.selected_node = Some(id),
                LocalAction::Toggle(id, expand) => {
                    if expand {
                        self.expanded_ids.insert(id);
                    } else {
                        self.expanded_ids.remove(&id);
                    }
                }
            }
            return HandlerResponse::ignored().with_render().with_consumed(true);
        }

        if response.consumed || response.needs_render {
            HandlerResponse::ignored().with_render().with_consumed(true)
        } else {
            HandlerResponse::ignored()
        }
    }
}

impl ComponentDebugState for LogDetails {
    fn debug_state(&self) -> Vec<ComponentDebugEntry> {
        vec![
            ComponentDebugEntry::new(
                "mode",
                match self.mode {
                    DetailsMode::Pretty => "pretty",
                    DetailsMode::Tree => "tree",
                },
            ),
            ComponentDebugEntry::new("pretty_scroll", self.pretty_scroll.to_string()),
            ComponentDebugEntry::new(
                "selected_node",
                self.selected_node
                    .clone()
                    .unwrap_or_else(|| "-".to_string()),
            ),
        ]
    }
}

impl<A, Ctx> InteractiveComponent<A, Ctx> for LogDetails {
    type Props<'a> = LogDetailsProps<'a>;

    fn update(
        &mut self,
        input: ComponentInput<'_, Ctx>,
        props: Self::Props<'_>,
    ) -> HandlerResponse<A> {
        self.sync_entry(props.entry);

        if !props.is_focused {
            return HandlerResponse::ignored();
        }

        match input {
            ComponentInput::Command {
                name: "toggle_view",
                ..
            } => {
                if self.toggle_mode(props.entry) {
                    HandlerResponse::ignored().with_render().with_consumed(true)
                } else {
                    HandlerResponse::ignored()
                }
            }
            _ => {
                let lines = pretty_lines(props.entry);
                match (self.mode, props.entry) {
                    (DetailsMode::Pretty, _) => self.handle_scroll_input(input, &lines),
                    (DetailsMode::Tree, Some(entry)) => self.handle_tree_input(input, entry),
                    (DetailsMode::Tree, None) => HandlerResponse::ignored(),
                }
            }
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        self.sync_entry(props.entry);

        let title = if tree_available(props.entry) {
            match self.mode {
                DetailsMode::Pretty => " Details / pretty ",
                DetailsMode::Tree => " Details / tree ",
            }
        } else {
            " Details "
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(if props.is_focused {
                Style::default().fg(Color::Rgb(123, 203, 216))
            } else {
                Style::default().fg(Color::Rgb(64, 80, 92))
            })
            .style(Style::default().bg(Color::Rgb(11, 15, 19)));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        match (self.mode, props.entry) {
            (DetailsMode::Pretty, _) => {
                let lines = pretty_lines(props.entry);
                let mut render_content = |frame: &mut Frame, area: Rect, range: VisibleRange| {
                    let end = range.end.min(lines.len());
                    let start = range.start.min(end);
                    frame.render_widget(Paragraph::new(lines[start..end].to_vec()), area);
                };

                self.pretty_view.render_widget(
                    frame,
                    inner,
                    ScrollViewRenderProps {
                        content_height: lines.len().max(1),
                        scroll_offset: self.pretty_scroll,
                        is_focused: props.is_focused,
                        style: scroll_style(),
                        behavior: ScrollViewBehavior {
                            page_step: 8,
                            ..Default::default()
                        },
                        render_content: &mut render_content,
                    },
                );
            }
            (DetailsMode::Tree, Some(entry)) => {
                let nodes = build_json_tree(entry.payload.as_ref());
                if nodes.is_empty() {
                    frame.render_widget(
                        Paragraph::new("JSON tree is only available for objects and arrays.")
                            .style(Style::default().fg(Color::Rgb(124, 132, 150))),
                        inner,
                    );
                    return;
                }

                self.tree_view.render_widget(
                    frame,
                    inner,
                    TreeViewRenderProps {
                        nodes: &nodes,
                        selected_id: self.selected_node.as_ref(),
                        expanded_ids: &self.expanded_ids,
                        is_focused: props.is_focused,
                        style: tree_style(),
                        behavior: TreeViewBehavior::default(),
                        measure_node: None,
                        column_padding: 1,
                        render_node: &render_tree_node,
                    },
                );
            }
            (DetailsMode::Tree, None) => {
                frame.render_widget(
                    Paragraph::new("Select a log entry to inspect its details.")
                        .style(Style::default().fg(Color::Rgb(124, 132, 150))),
                    inner,
                );
            }
        }
    }
}

fn pretty_lines(entry: Option<&LogEntry>) -> Vec<Line<'static>> {
    let Some(entry) = entry else {
        return vec![
            Line::styled(
                "Waiting for log input.",
                Style::default().fg(Color::Rgb(124, 132, 150)),
            ),
            Line::raw("Pipe output in or pass a file path to start triaging."),
        ];
    };

    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                format!("seq {:>6}", entry.seq),
                Style::default().fg(Color::Rgb(124, 132, 150)),
            ),
            Span::raw("  "),
            Span::styled(
                entry.level.badge().to_string(),
                Style::default()
                    .fg(entry.level.color())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                entry.source.clone(),
                Style::default().fg(Color::Rgb(104, 174, 195)),
            ),
        ]),
        Line::raw(""),
    ];

    if let Some(timestamp) = &entry.timestamp {
        lines.push(Line::from(vec![
            Span::styled("timestamp ", Style::default().fg(Color::Rgb(124, 132, 150))),
            Span::raw(timestamp.clone()),
        ]));
    }

    lines.push(Line::from(vec![
        Span::styled("message   ", Style::default().fg(Color::Rgb(124, 132, 150))),
        Span::styled(
            entry.message.clone(),
            Style::default().fg(Color::Rgb(229, 229, 227)),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("raw       ", Style::default().fg(Color::Rgb(124, 132, 150))),
        Span::styled(
            entry.display_line.clone(),
            Style::default().fg(Color::Rgb(209, 212, 216)),
        ),
    ]));

    if let Some(payload) = &entry.payload {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            "payload",
            Style::default()
                .fg(Color::Rgb(231, 179, 94))
                .add_modifier(Modifier::BOLD),
        ));
        let pretty =
            serde_json::to_string_pretty(payload).unwrap_or_else(|_| entry.display_line.clone());
        lines.extend(pretty.lines().map(|line| Line::raw(line.to_string())));
    }

    lines
}

fn tree_available(entry: Option<&LogEntry>) -> bool {
    entry
        .and_then(|entry| entry.payload.as_ref())
        .is_some_and(|payload| payload.is_object() || payload.is_array())
}

fn build_json_tree(payload: Option<&Value>) -> Vec<TreeNode<String, String>> {
    let Some(payload) = payload else {
        return Vec::new();
    };

    if !payload.is_object() && !payload.is_array() {
        return Vec::new();
    }

    vec![build_node("$".to_string(), "payload".to_string(), payload)]
}

fn build_node(id: String, label: String, value: &Value) -> TreeNode<String, String> {
    match value {
        Value::Object(map) => {
            let children = map
                .iter()
                .map(|(key, value)| {
                    build_node(
                        format!("{id}.{key}"),
                        format!("{key}: {}", value_summary(value)),
                        value,
                    )
                })
                .collect();
            TreeNode::with_children(id, format!("{label} {{}}"), children)
        }
        Value::Array(items) => {
            let children = items
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    build_node(
                        format!("{id}[{index}]"),
                        format!("[{index}]: {}", value_summary(value)),
                        value,
                    )
                })
                .collect();
            TreeNode::with_children(id, format!("{label} []"), children)
        }
        _ => TreeNode::new(id, format!("{label}: {}", value_summary(value))),
    }
}

fn value_summary(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => format!("{value:?}"),
        Value::Array(items) => format!("array({})", items.len()),
        Value::Object(map) => format!("object({})", map.len()),
    }
}

fn render_tree_node(render: TreeNodeRender<'_, String, String>) -> Line<'static> {
    let style = if render.has_children {
        Style::default()
            .fg(Color::Rgb(123, 203, 216))
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Rgb(209, 212, 216))
    };

    Line::styled(render.node.value.clone(), style)
}

fn scroll_style() -> ScrollViewStyle {
    let mut style = ScrollViewStyle::minimal();
    style.base.bg = Some(Color::Rgb(11, 15, 19));
    style.base.fg = Some(Color::Rgb(209, 212, 216));
    style
}

fn tree_style() -> TreeViewStyle {
    let mut style = TreeViewStyle::minimal();
    style.base.bg = Some(Color::Rgb(11, 15, 19));
    style.base.fg = Some(Color::Rgb(209, 212, 216));
    style.selection.style = Some(Style::default().bg(Color::Rgb(22, 31, 38)));
    style.selection.marker = Some("› ");
    style.branches.connector_style = Style::default().fg(Color::Rgb(73, 84, 94));
    style.branches.caret_style = Style::default().fg(Color::Rgb(231, 179, 94));
    style
}

#[cfg(test)]
mod tests {
    use tui_dispatch::key;
    use tui_dispatch_components::ComponentInput;

    use super::*;
    use crate::AppBindingContext;

    #[test]
    fn tree_toggle_only_works_for_object_or_array_payloads() {
        let mut details = LogDetails::new();
        let object_entry = crate::state::parse_log_entry(
            1,
            r#"{"level":"info","target":"api","message":"ok","payload":{"nested":1}}"#.to_string(),
        );
        let scalar_entry = crate::state::parse_log_entry(2, r#""hello""#.to_string());

        let response = <LogDetails as InteractiveComponent<(), AppBindingContext>>::update(
            &mut details,
            ComponentInput::Command {
                name: "toggle_view",
                ctx: AppBindingContext::Details,
            },
            LogDetailsProps {
                entry: Some(&object_entry),
                is_focused: true,
            },
        );
        assert!(response.needs_render);
        assert_eq!(
            details.debug_state()[0],
            ComponentDebugEntry::new("mode", "tree")
        );

        let response = <LogDetails as InteractiveComponent<(), AppBindingContext>>::update(
            &mut details,
            ComponentInput::Command {
                name: "toggle_view",
                ctx: AppBindingContext::Details,
            },
            LogDetailsProps {
                entry: Some(&scalar_entry),
                is_focused: true,
            },
        );
        assert!(!response.needs_render);
        assert_eq!(
            details.debug_state()[0],
            ComponentDebugEntry::new("mode", "pretty")
        );
    }

    #[test]
    fn local_tree_state_survives_same_entry_updates() {
        let mut details = LogDetails::new();
        let entry = crate::state::parse_log_entry(
            1,
            r#"{"level":"info","target":"api","message":"ok","payload":{"nested":{"leaf":1},"other":2}}"#.to_string(),
        );

        <LogDetails as InteractiveComponent<(), AppBindingContext>>::update(
            &mut details,
            ComponentInput::Command {
                name: "toggle_view",
                ctx: AppBindingContext::Details,
            },
            LogDetailsProps {
                entry: Some(&entry),
                is_focused: true,
            },
        );
        <LogDetails as InteractiveComponent<(), AppBindingContext>>::update(
            &mut details,
            ComponentInput::Key(key("down")),
            LogDetailsProps {
                entry: Some(&entry),
                is_focused: true,
            },
        );

        let before = details.debug_state();

        <LogDetails as InteractiveComponent<(), AppBindingContext>>::update(
            &mut details,
            ComponentInput::Key(key("down")),
            LogDetailsProps {
                entry: Some(&entry),
                is_focused: true,
            },
        );

        let after = details.debug_state();
        assert_eq!(before[0], ComponentDebugEntry::new("mode", "tree"));
        assert_eq!(after[0], ComponentDebugEntry::new("mode", "tree"));
        assert_ne!(after[2].value, "-");
    }
}
