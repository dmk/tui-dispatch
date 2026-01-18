use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Span,
};
use tui_dispatch_components::{
    StatusBar as ComponentStatusBar, StatusBarHint, StatusBarItem,
    StatusBarProps as ComponentStatusBarProps, StatusBarSection, StatusBarStyle,
};

use super::Component;
use crate::action::Action;
use crate::state::AppState;

pub struct StatusBar;

pub struct StatusBarProps<'a> {
    pub state: &'a AppState,
}

impl Component<Action> for StatusBar {
    type Props<'a> = StatusBarProps<'a>;

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        let state = props.state;

        let style = StatusBarStyle {
            base: tui_dispatch_components::BaseStyle {
                bg: Some(Color::Rgb(30, 30, 40)),
                ..Default::default()
            },
            text: Style::default().fg(Color::DarkGray),
            hint_key: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            hint_label: Style::default().fg(Color::Rgb(80, 80, 90)),
            separator: Style::default().fg(Color::DarkGray),
        };

        if state.search.active {
            // Search mode: show search query with cursor
            let search_items = [
                StatusBarItem::span(Span::styled(
                    " /",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )),
                StatusBarItem::span(Span::styled(
                    &state.search.query,
                    Style::default().fg(Color::White),
                )),
                StatusBarItem::span(Span::styled(
                    "_",
                    Style::default()
                        .fg(Color::Gray)
                        .add_modifier(Modifier::SLOW_BLINK),
                )),
            ];
            let left = StatusBarSection::items(&search_items);

            let mut status_bar = ComponentStatusBar::new();
            <ComponentStatusBar as Component<Action>>::render(
                &mut status_bar,
                frame,
                area,
                ComponentStatusBarProps {
                    left,
                    center: StatusBarSection::empty(),
                    right: StatusBarSection::empty(),
                    style,
                    is_focused: false,
                },
            );
        } else {
            // Normal mode: line info, match info, stats, help
            let line_info = format!(
                " {}:{} ",
                state.scroll_offset + 1,
                state.rendered_lines.len()
            );

            let match_info = if !state.search.matches.is_empty() {
                format!(
                    "[{}/{}]",
                    state.search.current_match + 1,
                    state.search.matches.len()
                )
            } else {
                String::new()
            };

            let stats_info = if state.features.show_stats {
                format!(
                    " §{} ¶{} ",
                    state.stats.heading_count, state.stats.paragraph_count
                )
            } else {
                String::new()
            };

            let mut left_items: Vec<StatusBarItem> = vec![StatusBarItem::span(Span::styled(
                line_info,
                Style::default().fg(Color::DarkGray),
            ))];

            if !match_info.is_empty() {
                left_items.push(StatusBarItem::span(Span::styled(
                    match_info,
                    Style::default().fg(Color::Yellow),
                )));
            }

            if !stats_info.is_empty() {
                left_items.push(StatusBarItem::span(Span::styled(
                    stats_info,
                    Style::default().fg(Color::Cyan),
                )));
            }

            let left = StatusBarSection::items(&left_items);

            let hints = [
                StatusBarHint::new("j/k", "scroll"),
                StatusBarHint::new("/", "search"),
                StatusBarHint::new("n/N", "next/prev"),
                StatusBarHint::new("F12", "debug"),
                StatusBarHint::new("q", "quit"),
            ];
            let right = StatusBarSection::hints(&hints);

            let mut status_bar = ComponentStatusBar::new();
            <ComponentStatusBar as Component<Action>>::render(
                &mut status_bar,
                frame,
                area,
                ComponentStatusBarProps {
                    left,
                    center: StatusBarSection::empty(),
                    right,
                    style,
                    is_focused: false,
                },
            );
        }
    }
}
