use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::{Component, LOCATION_ICON};
use crate::state::Location;

pub struct LocationHeader;

pub struct LocationHeaderProps<'a> {
    pub location: &'a Location,
}

impl LocationHeader {
    pub const HEIGHT: u16 = 2;
}

impl Component for LocationHeader {
    type Props<'a> = LocationHeaderProps<'a>;

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        let chunks = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(area);

        let location_line = Line::from(vec![
            Span::styled(LOCATION_ICON, Style::default()),
            Span::styled(
                &props.location.name,
                Style::default().fg(Color::White).bold(),
            ),
        ])
        .centered();
        frame.render_widget(Paragraph::new(location_line), chunks[0]);

        let coords_line = Line::from(vec![Span::styled(
            format!("{:.2}°N, {:.2}°E", props.location.lat, props.location.lon),
            Style::default().fg(Color::DarkGray),
        )])
        .centered();
        frame.render_widget(Paragraph::new(coords_line), chunks[1]);
    }
}
