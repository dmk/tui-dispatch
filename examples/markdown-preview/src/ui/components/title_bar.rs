use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::Component;
use crate::action::Action;

pub struct TitleBar;

pub struct TitleBarProps<'a> {
    pub file_path: &'a str,
}

impl Component<Action> for TitleBar {
    type Props<'a> = TitleBarProps<'a>;

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        let title = Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled(
                props.file_path,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ", Style::default()),
        ]);
        frame.render_widget(
            Paragraph::new(title).style(Style::default().bg(Color::Rgb(30, 30, 40))),
            area,
        );
    }
}
