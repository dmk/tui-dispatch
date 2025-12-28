//! Application state for markdown preview

use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Application state
#[derive(Debug)]
pub struct AppState {
    /// Path to the markdown file
    pub file_path: String,

    /// Raw markdown content
    pub raw_content: String,

    /// Rendered lines for display
    pub rendered_lines: Vec<Line<'static>>,

    /// Current scroll offset (line index)
    pub scroll_offset: usize,

    /// Terminal height (for page scrolling)
    pub terminal_height: u16,

    /// Search mode state
    pub search: SearchState,

    /// Document statistics (for debug overlay)
    pub stats: DocStats,
}

/// Search mode state
#[derive(Debug, Default, Clone)]
pub struct SearchState {
    /// Whether search mode is active
    pub active: bool,

    /// Current search query
    pub query: String,

    /// Line indices of matches
    pub matches: Vec<usize>,

    /// Current match index
    pub current_match: usize,
}

/// Document statistics for debug overlay
#[derive(Debug, Default, Clone)]
pub struct DocStats {
    pub heading_count: usize,
    pub link_count: usize,
    pub code_block_count: usize,
    pub list_item_count: usize,
    pub paragraph_count: usize,
    pub total_lines: usize,
}

impl AppState {
    /// Create new state with the given file path
    pub fn new(file_path: String) -> Self {
        let mut state = Self {
            file_path,
            raw_content: String::new(),
            rendered_lines: Vec::new(),
            scroll_offset: 0,
            terminal_height: 24,
            search: SearchState::default(),
            stats: DocStats::default(),
        };
        state.reload();
        state
    }

    /// Reload the file from disk
    pub fn reload(&mut self) {
        self.raw_content = std::fs::read_to_string(&self.file_path)
            .unwrap_or_else(|e| format!("Error reading file: {}", e));
        self.render_markdown();
        self.scroll_offset = 0;
        self.search = SearchState::default();
    }

    /// Render markdown to styled lines
    fn render_markdown(&mut self) {
        let parser = Parser::new(&self.raw_content);
        let mut lines: Vec<Line<'static>> = Vec::new();
        let mut current_spans: Vec<Span<'static>> = Vec::new();
        let mut stats = DocStats::default();

        // Style stack for nested formatting
        let mut style_stack: Vec<Style> = vec![Style::default()];
        let mut in_code_block = false;

        for event in parser {
            match event {
                Event::Start(tag) => {
                    let style = match &tag {
                        Tag::Heading { level, .. } => {
                            stats.heading_count += 1;
                            let color = match *level {
                                pulldown_cmark::HeadingLevel::H1 => Color::Cyan,
                                pulldown_cmark::HeadingLevel::H2 => Color::Green,
                                pulldown_cmark::HeadingLevel::H3 => Color::Yellow,
                                _ => Color::Magenta,
                            };
                            Style::default().fg(color).add_modifier(Modifier::BOLD)
                        }
                        Tag::Strong => style_stack
                            .last()
                            .unwrap_or(&Style::default())
                            .add_modifier(Modifier::BOLD),
                        Tag::Emphasis => style_stack
                            .last()
                            .unwrap_or(&Style::default())
                            .add_modifier(Modifier::ITALIC),
                        Tag::CodeBlock(_) => {
                            stats.code_block_count += 1;
                            in_code_block = true;
                            Style::default()
                                .fg(Color::Rgb(180, 180, 180))
                                .bg(Color::Rgb(40, 40, 50))
                        }
                        Tag::Link { .. } => {
                            stats.link_count += 1;
                            Style::default()
                                .fg(Color::Blue)
                                .add_modifier(Modifier::UNDERLINED)
                        }
                        Tag::List(_) => Style::default(),
                        Tag::Item => {
                            stats.list_item_count += 1;
                            // Add bullet point
                            current_spans
                                .push(Span::styled("  - ", Style::default().fg(Color::DarkGray)));
                            Style::default()
                        }
                        Tag::Paragraph => {
                            stats.paragraph_count += 1;
                            Style::default()
                        }
                        _ => Style::default(),
                    };
                    style_stack.push(style);
                }
                Event::End(tag_end) => {
                    style_stack.pop();
                    match tag_end {
                        TagEnd::Heading(_) | TagEnd::Paragraph => {
                            if !current_spans.is_empty() {
                                lines.push(Line::from(std::mem::take(&mut current_spans)));
                            }
                            lines.push(Line::from(""));
                        }
                        TagEnd::CodeBlock => {
                            in_code_block = false;
                            if !current_spans.is_empty() {
                                lines.push(Line::from(std::mem::take(&mut current_spans)));
                            }
                            lines.push(Line::from(""));
                        }
                        TagEnd::Item => {
                            if !current_spans.is_empty() {
                                lines.push(Line::from(std::mem::take(&mut current_spans)));
                            }
                        }
                        _ => {}
                    }
                }
                Event::Text(text) => {
                    let style = *style_stack.last().unwrap_or(&Style::default());
                    if in_code_block {
                        // Handle code block lines
                        for line in text.lines() {
                            current_spans.push(Span::styled(format!("  {}", line), style));
                            lines.push(Line::from(std::mem::take(&mut current_spans)));
                        }
                    } else {
                        current_spans.push(Span::styled(text.to_string(), style));
                    }
                }
                Event::Code(code) => {
                    let style = Style::default()
                        .fg(Color::Rgb(220, 180, 100))
                        .bg(Color::Rgb(40, 40, 50));
                    current_spans.push(Span::styled(format!("`{}`", code), style));
                }
                Event::SoftBreak => {
                    current_spans.push(Span::raw(" "));
                }
                Event::HardBreak => {
                    if !current_spans.is_empty() {
                        lines.push(Line::from(std::mem::take(&mut current_spans)));
                    }
                }
                Event::Rule => {
                    if !current_spans.is_empty() {
                        lines.push(Line::from(std::mem::take(&mut current_spans)));
                    }
                    lines.push(Line::from(Span::styled(
                        "─".repeat(40),
                        Style::default().fg(Color::DarkGray),
                    )));
                    lines.push(Line::from(""));
                }
                _ => {}
            }
        }

        // Flush remaining spans
        if !current_spans.is_empty() {
            lines.push(Line::from(current_spans));
        }

        stats.total_lines = lines.len();
        self.rendered_lines = lines;
        self.stats = stats;
    }

    /// Maximum scroll offset
    pub fn max_scroll(&self) -> usize {
        let visible_lines = self.terminal_height.saturating_sub(4) as usize;
        self.rendered_lines.len().saturating_sub(visible_lines)
    }

    /// Scroll by delta lines
    pub fn scroll(&mut self, delta: i16) {
        if delta > 0 {
            self.scroll_offset = self
                .scroll_offset
                .saturating_add(delta as usize)
                .min(self.max_scroll());
        } else {
            self.scroll_offset = self.scroll_offset.saturating_sub((-delta) as usize);
        }
    }

    /// Scroll by page
    pub fn scroll_page(&mut self, direction: i16) {
        let page_size = self.terminal_height.saturating_sub(4) as i16;
        self.scroll(direction * page_size);
    }

    /// Update search matches
    pub fn update_search_matches(&mut self) {
        self.search.matches.clear();
        self.search.current_match = 0;

        if self.search.query.is_empty() {
            return;
        }

        let query_lower = self.search.query.to_lowercase();
        for (i, line) in self.rendered_lines.iter().enumerate() {
            let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            if line_text.to_lowercase().contains(&query_lower) {
                self.search.matches.push(i);
            }
        }
    }

    /// Jump to next search match
    pub fn next_match(&mut self) {
        if self.search.matches.is_empty() {
            return;
        }
        self.search.current_match = (self.search.current_match + 1) % self.search.matches.len();
        self.scroll_to_current_match();
    }

    /// Jump to previous search match
    pub fn prev_match(&mut self) {
        if self.search.matches.is_empty() {
            return;
        }
        self.search.current_match = if self.search.current_match == 0 {
            self.search.matches.len() - 1
        } else {
            self.search.current_match - 1
        };
        self.scroll_to_current_match();
    }

    /// Scroll to make current match visible
    pub fn scroll_to_current_match(&mut self) {
        if let Some(&line_idx) = self.search.matches.get(self.search.current_match) {
            let visible_lines = self.terminal_height.saturating_sub(4) as usize;
            if line_idx < self.scroll_offset {
                self.scroll_offset = line_idx;
            } else if line_idx >= self.scroll_offset + visible_lines {
                self.scroll_offset = line_idx.saturating_sub(visible_lines / 2);
            }
        }
    }
}
