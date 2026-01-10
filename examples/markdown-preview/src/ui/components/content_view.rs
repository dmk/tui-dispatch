use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use super::Component;
use crate::action::Action;
use crate::state::{AppState, CODE_BG};

pub struct ContentView;

pub struct ContentViewProps<'a> {
    pub state: &'a AppState,
}

impl Component<Action> for ContentView {
    type Props<'a> = ContentViewProps<'a>;

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        let state = props.state;

        let block = Block::default()
            .borders(Borders::LEFT | Borders::RIGHT)
            .border_style(Style::default().fg(Color::Rgb(60, 60, 70)));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Calculate gutter width for line numbers
        let gutter_width = if state.features.line_numbers {
            let max_line = state.rendered_lines.len();
            (max_line.to_string().len() + 1) as u16
        } else {
            0
        };

        // Split area for gutter and content
        let content_area = if state.features.line_numbers {
            Rect {
                x: inner.x + gutter_width,
                width: inner.width.saturating_sub(gutter_width),
                ..inner
            }
        } else {
            inner
        };

        let content_width = content_area.width as usize;

        // Get visible lines
        let visible_height = inner.height as usize;
        let start = state.scroll_offset;
        let end = (start + visible_height).min(state.rendered_lines.len());

        // Render line numbers if enabled
        if state.features.line_numbers {
            let gutter_area = Rect {
                width: gutter_width,
                ..inner
            };
            let line_nums: Vec<Line> = (start..end)
                .map(|i| {
                    Line::from(Span::styled(
                        format!("{:>width$} ", i + 1, width = gutter_width as usize - 1),
                        Style::default().fg(Color::DarkGray),
                    ))
                })
                .collect();
            frame.render_widget(Paragraph::new(line_nums), gutter_area);
        }

        // Render each line, handling code blocks specially for full-width background
        for (i, rendered) in state.rendered_lines[start..end].iter().enumerate() {
            let line_idx = start + i;
            let y = content_area.y + i as u16;

            if y >= content_area.y + content_area.height {
                break;
            }

            let line_area = Rect {
                x: content_area.x,
                y,
                width: content_area.width,
                height: 1,
            };

            // For code blocks, fill the entire line with background first
            if rendered.is_code {
                let bg_fill = " ".repeat(content_width);
                frame.render_widget(
                    Paragraph::new(Line::from(bg_fill)).style(Style::default().bg(CODE_BG)),
                    line_area,
                );

                // Render language label in top-right if present
                if let Some(ref lang) = rendered.lang
                    && !lang.is_empty()
                {
                    let label = format!(" {} ", lang);
                    let label_width = label.len() as u16;
                    let label_area = Rect {
                        x: line_area.x + line_area.width.saturating_sub(label_width + 1),
                        y: line_area.y,
                        width: label_width,
                        height: 1,
                    };
                    frame.render_widget(
                        Paragraph::new(Line::from(Span::styled(
                            label,
                            Style::default().fg(Color::Rgb(90, 90, 110)).bg(CODE_BG),
                        ))),
                        label_area,
                    );
                }
            }

            // Prepare the line (with search highlighting if needed)
            let line = if !state.search.query.is_empty() && state.search.matches.contains(&line_idx)
            {
                let is_current =
                    state.search.matches.get(state.search.current_match) == Some(&line_idx);
                let bg = if is_current {
                    Color::Rgb(80, 80, 40)
                } else {
                    Color::Rgb(50, 50, 30)
                };
                Line::from(
                    rendered
                        .line
                        .spans
                        .iter()
                        .map(|s| Span::styled(s.content.clone(), s.style.bg(bg)))
                        .collect::<Vec<_>>(),
                )
            } else {
                rendered.line.clone()
            };

            // Render the actual content
            let mut paragraph = Paragraph::new(line);
            if state.features.wrap_lines && !rendered.is_code {
                paragraph = paragraph.wrap(Wrap { trim: false });
            }
            frame.render_widget(paragraph, line_area);
        }
    }
}
