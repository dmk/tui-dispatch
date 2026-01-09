use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::Component;

pub struct HelpBar;

pub struct HelpBarProps;

impl Component for HelpBar {
    type Props<'a> = HelpBarProps;

    fn render(&mut self, frame: &mut Frame, area: Rect, _props: Self::Props<'_>) {
        let help = Line::from(vec![
            Span::styled(" r", Style::default().fg(Color::Cyan).bold()),
            Span::styled(" refresh  ", Style::default().fg(Color::DarkGray)),
            Span::styled("u", Style::default().fg(Color::Cyan).bold()),
            Span::styled(" units  ", Style::default().fg(Color::DarkGray)),
            Span::styled("q", Style::default().fg(Color::Cyan).bold()),
            Span::styled(" quit ", Style::default().fg(Color::DarkGray)),
        ])
        .centered();
        frame.render_widget(Paragraph::new(help), area);
    }
}
