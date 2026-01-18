use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use tui_dispatch_components::{
    ScrollView, ScrollViewBehavior, ScrollViewProps, ScrollViewStyle, VisibleRange,
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

        let gutter_area = if state.features.line_numbers {
            Rect {
                width: gutter_width,
                ..inner
            }
        } else {
            Rect::default()
        };

        // Capture state for the render callback
        let rendered_lines = &state.rendered_lines;
        let search = &state.search;
        let features = &state.features;
        let show_line_numbers = state.features.line_numbers;

        // Custom render callback that handles line numbers, code blocks, search highlighting
        let mut render_content = |frame: &mut Frame, area: Rect, range: VisibleRange| {
            let start = range.start;
            let end = range.end;

            // Render line numbers if enabled
            if show_line_numbers && gutter_area.width > 0 {
                let adjusted_gutter = Rect {
                    y: gutter_area.y,
                    height: area.height,
                    ..gutter_area
                };
                let line_nums: Vec<Line> = (start..end)
                    .map(|i| {
                        Line::from(Span::styled(
                            format!("{:>width$} ", i + 1, width = gutter_width as usize - 1),
                            Style::default().fg(Color::DarkGray),
                        ))
                    })
                    .collect();
                frame.render_widget(Paragraph::new(line_nums), adjusted_gutter);
            }

            let content_width = area.width as usize;

            // Render each line, handling code blocks specially for full-width background
            for (i, rendered) in rendered_lines[start..end].iter().enumerate() {
                let line_idx = start + i;
                let y = area.y + i as u16;

                if y >= area.y + area.height {
                    break;
                }

                let line_area = Rect {
                    x: area.x,
                    y,
                    width: area.width,
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
                let line = if !search.query.is_empty() && search.matches.contains(&line_idx) {
                    let is_current = search.matches.get(search.current_match) == Some(&line_idx);
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
                if features.wrap_lines && !rendered.is_code {
                    paragraph = paragraph.wrap(Wrap { trim: false });
                }
                frame.render_widget(paragraph, line_area);
            }
        };

        // Use ScrollView component
        let scroll_style = ScrollViewStyle {
            base: tui_dispatch_components::BaseStyle {
                border: None,
                padding: tui_dispatch_components::Padding::default(),
                bg: None,
                fg: None,
            },
            scrollbar: tui_dispatch_components::ScrollbarStyle::default(),
        };

        let mut scroll_view = ScrollView::new();
        <ScrollView as Component<Action>>::render(
            &mut scroll_view,
            frame,
            content_area,
            ScrollViewProps {
                content_height: rendered_lines.len(),
                scroll_offset: state.scroll_offset,
                is_focused: true,
                style: scroll_style,
                behavior: ScrollViewBehavior {
                    show_scrollbar: true,
                    scroll_step: 1,
                    page_step: 0,
                },
                on_scroll: |_| Action::NavScroll(0), // No-op for render-only
                render_content: &mut render_content,
            },
        );
    }
}
