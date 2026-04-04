use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use tui_dispatch::prelude::Component;
use tui_dispatch_components::{
    ComponentHost, StatusBar, StatusBarHint, StatusBarItem, StatusBarProps, StatusBarSection,
    StatusBarStyle,
};

use crate::state::AppState;
use crate::ui::MountedViews;
use crate::{Action, AppBindingContext, RouteId};

pub fn render_app(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    host: &ComponentHost<AppState, Action, RouteId, AppBindingContext>,
    mounted: MountedViews,
    status_bar: &mut StatusBar,
) {
    let [header_area, body_area, status_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(12),
        Constraint::Length(1),
    ])
    .areas(area);

    render_header(frame, header_area, state);

    let [filters_area, content_area] =
        Layout::horizontal([Constraint::Length(28), Constraint::Min(44)]).areas(body_area);

    let [levels_area, tags_area] =
        Layout::vertical([Constraint::Percentage(48), Constraint::Percentage(52)])
            .spacing(1)
            .areas(filters_area);

    let [content_label_area, content_body_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(content_area);

    render_section_label(
        frame,
        content_label_area,
        if state.details_open() {
            "Preview"
        } else {
            "Logs"
        },
        matches!(state.focus, crate::Focus::Logs | crate::Focus::Details),
    );
    host.render(mounted.levels, frame, levels_area, state);
    host.render(mounted.tags, frame, tags_area, state);
    if state.details_open() {
        host.render(mounted.details, frame, content_body_area, state);
    } else {
        host.render(mounted.logs, frame, content_body_area, state);
    }

    render_status_bar(frame, status_area, state, status_bar);
}

fn render_header(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Rgb(9, 13, 17)))
        .border_style(Style::default().fg(Color::Rgb(64, 80, 92)));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let selected = state
        .selected_entry()
        .map(|entry| format!("{} {}", entry.level.badge(), entry.message))
        .unwrap_or_else(|| "No matching log entry".to_string());

    let lines = vec![
        Line::from(vec![
            Span::styled(
                "log-viewer",
                Style::default()
                    .fg(Color::Rgb(123, 203, 216))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  source {}", state.source_label),
                Style::default().fg(Color::Rgb(162, 173, 183)),
            ),
            Span::styled(
                format!("  buffer {}/{}", state.total_count(), state.buffer_cap),
                Style::default().fg(Color::Rgb(162, 173, 183)),
            ),
        ]),
        Line::from(vec![Span::styled(
            "Enter opens details for the highlighted log. The drawer stays closed until you ask for it.",
            Style::default().fg(Color::Rgb(219, 222, 226)),
        )]),
        Line::from(vec![
            Span::styled("Selected ", Style::default().fg(Color::Rgb(145, 155, 165))),
            Span::styled(selected, Style::default().fg(Color::Rgb(229, 229, 227))),
        ]),
    ];

    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_section_label(frame: &mut Frame, area: Rect, label: &str, focused: bool) {
    let style = if focused {
        Style::default()
            .fg(Color::Rgb(231, 179, 94))
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Rgb(130, 139, 149))
    };
    frame.render_widget(Paragraph::new(label).style(style), area);
}

fn render_status_bar(frame: &mut Frame, area: Rect, state: &AppState, status_bar: &mut StatusBar) {
    let left_items = [
        StatusBarItem::span(Span::styled(
            format!(" {} ", state.focus.label()),
            Style::default()
                .fg(Color::Rgb(9, 13, 17))
                .bg(Color::Rgb(231, 179, 94))
                .add_modifier(Modifier::BOLD),
        )),
        StatusBarItem::span(Span::styled(
            format!(
                " visible {}/{} ",
                state.visible_count(),
                state.total_count()
            ),
            Style::default().fg(Color::Rgb(209, 212, 216)),
        )),
        StatusBarItem::span(Span::styled(
            format!(" dropped {} ", state.dropped_lines),
            Style::default().fg(Color::Rgb(162, 173, 183)),
        )),
    ];

    let center_items = [StatusBarItem::span(Span::styled(
        if state.follow_mode {
            " follow on "
        } else {
            " follow off "
        },
        Style::default().fg(if state.follow_mode {
            Color::Rgb(146, 196, 125)
        } else {
            Color::Rgb(231, 179, 94)
        }),
    ))];

    let hints = [
        StatusBarHint::new("Tab", "focus"),
        StatusBarHint::new("Enter", "details"),
        StatusBarHint::new("f", "follow"),
        StatusBarHint::new("v", "view"),
        StatusBarHint::new("Esc", "close"),
        StatusBarHint::new("q", "quit"),
    ];

    <StatusBar as Component<Action>>::render(
        status_bar,
        frame,
        area,
        StatusBarProps {
            left: StatusBarSection::items(&left_items),
            center: StatusBarSection::items(&center_items),
            right: StatusBarSection::hints(&hints),
            style: status_style(),
            is_focused: false,
        },
    );
}

fn status_style() -> StatusBarStyle {
    StatusBarStyle {
        base: tui_dispatch_components::BaseStyle {
            bg: Some(Color::Rgb(9, 13, 17)),
            border: None,
            ..Default::default()
        },
        text: Style::default().fg(Color::Rgb(209, 212, 216)),
        hint_key: Style::default()
            .fg(Color::Rgb(123, 203, 216))
            .add_modifier(Modifier::BOLD),
        hint_label: Style::default().fg(Color::Rgb(130, 139, 149)),
        separator: Style::default().fg(Color::Rgb(73, 84, 94)),
    }
}
