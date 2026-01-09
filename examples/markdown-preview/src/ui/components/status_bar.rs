use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::Component;
use crate::state::AppState;

pub struct StatusBar;

pub struct StatusBarProps<'a> {
    pub state: &'a AppState,
}

impl Component for StatusBar {
    type Props<'a> = StatusBarProps<'a>;

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        let state = props.state;
        let style = Style::default().bg(Color::Rgb(30, 30, 40));

        if state.search.active {
            let search_line = Line::from(vec![
                Span::styled(
                    " /",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(&state.search.query, Style::default().fg(Color::White)),
                Span::styled(
                    "_",
                    Style::default()
                        .fg(Color::Gray)
                        .add_modifier(Modifier::SLOW_BLINK),
                ),
            ]);
            frame.render_widget(Paragraph::new(search_line).style(style), area);
        } else {
            let match_info = if !state.search.matches.is_empty() {
                format!(
                    " [{}/{}]",
                    state.search.current_match + 1,
                    state.search.matches.len()
                )
            } else {
                String::new()
            };

            let line_info = format!(
                " {}:{} ",
                state.scroll_offset + 1,
                state.rendered_lines.len()
            );

            let stats_info = if state.features.show_stats {
                format!(
                    " §{} ¶{} ",
                    state.stats.heading_count, state.stats.paragraph_count
                )
            } else {
                String::new()
            };

            let help = " j/k:scroll  /:search  n/N:next/prev  F12:debug  q:quit ";

            let status = Line::from(vec![
                Span::styled(line_info, Style::default().fg(Color::DarkGray)),
                Span::styled(match_info, Style::default().fg(Color::Yellow)),
                Span::styled(stats_info, Style::default().fg(Color::Cyan)),
                Span::styled(help, Style::default().fg(Color::Rgb(80, 80, 90))),
            ]);
            frame.render_widget(Paragraph::new(status).style(style), area);
        }
    }
}
