//! UI components for the GitHub lookup app

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::action::Action;
use crate::state::AppState;

/// Render the application UI
#[allow(dead_code)]
pub fn render(frame: &mut Frame, state: &AppState) {
    render_area(frame, frame.area(), state);
}

/// Render the application UI within a specific area
pub fn render_area(frame: &mut Frame, area: Rect, state: &AppState) {
    let chunks = Layout::vertical([
        Constraint::Length(3), // Input
        Constraint::Min(10),   // Content
        Constraint::Length(1), // Help
    ])
    .split(area);

    render_input(frame, chunks[0], state);
    render_content(frame, chunks[1], state);
    render_help(frame, chunks[2]);
}

/// Render the search input box
fn render_input(frame: &mut Frame, area: Rect, state: &AppState) {
    let input = Paragraph::new(state.query.as_str())
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" GitHub Username "),
        );
    frame.render_widget(input, area);

    // Show cursor at end of input
    frame.set_cursor_position((area.x + state.query.len() as u16 + 1, area.y + 1));
}

/// Render the main content area
fn render_content(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default().borders(Borders::ALL).title(" User Info ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.is_loading {
        let loading = Paragraph::new("Loading...")
            .style(Style::default().fg(Color::Cyan))
            .alignment(Alignment::Center);
        frame.render_widget(loading, inner);
        return;
    }

    if let Some(error) = &state.error {
        let error_text = Paragraph::new(error.as_str())
            .style(Style::default().fg(Color::Red))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
        frame.render_widget(error_text, inner);
        return;
    }

    if let Some(user) = &state.user {
        render_user(frame, inner, user);
        return;
    }

    // Empty state
    let help = Paragraph::new("Enter a GitHub username and press Enter to search")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    frame.render_widget(help, inner);
}

/// Render user information
fn render_user(frame: &mut Frame, area: Rect, user: &crate::state::GitHubUser) {
    let name_display = user
        .name
        .as_ref()
        .map(|n| format!("{} (@{})", n, user.login))
        .unwrap_or_else(|| format!("@{}", user.login));

    let bio_display = user.bio.as_deref().unwrap_or("No bio");

    let lines = vec![
        Line::from(vec![Span::styled(
            name_display,
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Bio: ", Style::default().fg(Color::Cyan)),
            Span::raw(bio_display),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Repos: ", Style::default().fg(Color::Cyan)),
            Span::raw(user.public_repos.to_string()),
            Span::raw("  "),
            Span::styled("Followers: ", Style::default().fg(Color::Cyan)),
            Span::raw(user.followers.to_string()),
            Span::raw("  "),
            Span::styled("Following: ", Style::default().fg(Color::Cyan)),
            Span::raw(user.following.to_string()),
        ]),
    ];

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

/// Render the help bar
fn render_help(frame: &mut Frame, area: Rect) {
    let help = Paragraph::new(" Enter: Search | Esc: Clear | Ctrl+C: Quit ")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, area);
}

/// Handle keyboard input and return actions
pub fn handle_key(key: KeyEvent, state: &AppState) -> Vec<Action> {
    match key.code {
        KeyCode::Char(c)
            if key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL)
                && c == 'c' =>
        {
            vec![Action::Quit]
        }
        KeyCode::Enter => {
            if !state.query.is_empty() {
                vec![Action::UserFetch(state.query.clone())]
            } else {
                vec![]
            }
        }
        KeyCode::Esc => vec![Action::Clear],
        KeyCode::Backspace => {
            let mut new_query = state.query.clone();
            new_query.pop();
            vec![Action::QueryChange(new_query)]
        }
        KeyCode::Char(c) => {
            let new_query = format!("{}{}", state.query, c);
            vec![Action::QueryChange(new_query)]
        }
        _ => vec![],
    }
}
