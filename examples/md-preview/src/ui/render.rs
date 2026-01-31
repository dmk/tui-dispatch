use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
};
use tui_dispatch::RenderContext;

use super::components::{
    Component, ContentView, ContentViewProps, StatusBar, StatusBarProps, TitleBar, TitleBarProps,
};
use crate::state::AppState;

pub fn render_app(frame: &mut Frame, area: Rect, state: &AppState, _ctx: RenderContext) {
    let chunks = Layout::vertical([
        Constraint::Length(1), // Title bar
        Constraint::Min(1),    // Content
        Constraint::Length(1), // Status bar
    ])
    .split(area);

    let mut title_bar = TitleBar;
    title_bar.render(
        frame,
        chunks[0],
        TitleBarProps {
            file_path: &state.file_path,
        },
    );

    let mut content = ContentView;
    content.render(frame, chunks[1], ContentViewProps { state });

    let mut status_bar = StatusBar;
    status_bar.render(frame, chunks[2], StatusBarProps { state });
}
