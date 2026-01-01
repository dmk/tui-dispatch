//! Application state for markdown preview

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use syntect::easy::HighlightLines;
use syntect::highlighting::{self, ThemeSet};
use syntect::parsing::SyntaxSet;

/// Code block background color
pub const CODE_BG: Color = Color::Rgb(30, 30, 40);

/// A rendered line with metadata
#[derive(Debug, Clone)]
pub struct RenderedLine {
    /// The line content
    pub line: Line<'static>,
    /// Whether this is a code block line (needs full-width background)
    pub is_code: bool,
    /// Language label to show in top-right (only for first line of code block)
    pub lang: Option<String>,
}

impl RenderedLine {
    fn text(line: Line<'static>) -> Self {
        Self {
            line,
            is_code: false,
            lang: None,
        }
    }

    fn code_with_lang(line: Line<'static>, lang: Option<String>) -> Self {
        Self {
            line,
            is_code: true,
            lang,
        }
    }
}

/// Application state
pub struct AppState {
    /// Path to the markdown file
    pub file_path: String,

    /// Raw markdown content
    pub raw_content: String,

    /// Rendered lines for display
    pub rendered_lines: Vec<RenderedLine>,

    /// Current scroll offset (line index)
    pub scroll_offset: usize,

    /// Terminal height (for page scrolling)
    pub terminal_height: u16,

    /// Search mode state
    pub search: SearchState,

    /// Document statistics (for debug overlay)
    pub stats: DocStats,

    /// Syntax highlighting resources (not Debug)
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("file_path", &self.file_path)
            .field("scroll_offset", &self.scroll_offset)
            .field("terminal_height", &self.terminal_height)
            .field("search", &self.search)
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
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
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
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

    /// Convert syntect color to ratatui color
    fn syntect_to_ratatui(color: highlighting::Color) -> Color {
        Color::Rgb(color.r, color.g, color.b)
    }

    /// Highlight code with syntect
    fn highlight_code(&self, code: &str, lang: &str) -> Vec<Line<'static>> {
        let syntax = self
            .syntax_set
            .find_syntax_by_token(lang)
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let theme = &self.theme_set.themes["base16-ocean.dark"];
        let mut highlighter = HighlightLines::new(syntax, theme);

        let mut lines = Vec::new();
        for line in code.lines() {
            let highlighted = highlighter
                .highlight_line(line, &self.syntax_set)
                .unwrap_or_default();

            let spans: Vec<Span<'static>> = highlighted
                .into_iter()
                .map(|(style, text)| {
                    Span::styled(
                        text.to_string(),
                        Style::default()
                            .fg(Self::syntect_to_ratatui(style.foreground))
                            .bg(CODE_BG),
                    )
                })
                .collect();

            lines.push(Line::from(spans));
        }
        lines
    }

    /// Render markdown to styled lines
    fn render_markdown(&mut self) {
        // Enable tables
        let options = Options::ENABLE_TABLES;
        let parser = Parser::new_ext(&self.raw_content, options);
        let mut lines: Vec<RenderedLine> = Vec::new();
        let mut current_spans: Vec<Span<'static>> = Vec::new();
        let mut stats = DocStats::default();

        // Style stack for nested formatting
        let mut style_stack: Vec<Style> = vec![Style::default()];
        let mut in_code_block = false;
        let mut code_block_lang = String::new();
        let mut code_block_content = String::new();

        // Table state
        let mut in_table = false;
        let mut table_rows: Vec<Vec<String>> = Vec::new();
        let mut current_row: Vec<String> = Vec::new();
        let mut current_cell = String::new();

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
                        Tag::CodeBlock(kind) => {
                            stats.code_block_count += 1;
                            in_code_block = true;
                            code_block_content.clear();
                            code_block_lang = match kind {
                                CodeBlockKind::Fenced(lang) => lang.to_string(),
                                CodeBlockKind::Indented => String::new(),
                            };
                            Style::default()
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
                            current_spans
                                .push(Span::styled("  • ", Style::default().fg(Color::DarkGray)));
                            Style::default()
                        }
                        Tag::Paragraph => {
                            stats.paragraph_count += 1;
                            Style::default()
                        }
                        Tag::Table(_alignments) => {
                            in_table = true;
                            table_rows.clear();
                            Style::default()
                        }
                        Tag::TableHead => {
                            current_row.clear();
                            Style::default()
                        }
                        Tag::TableRow => {
                            current_row.clear();
                            Style::default()
                        }
                        Tag::TableCell => {
                            current_cell.clear();
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
                                lines.push(RenderedLine::text(Line::from(std::mem::take(
                                    &mut current_spans,
                                ))));
                            }
                            lines.push(RenderedLine::text(Line::from("")));
                        }
                        TagEnd::CodeBlock => {
                            in_code_block = false;

                            // Highlight code lines
                            let highlighted =
                                self.highlight_code(&code_block_content, &code_block_lang);

                            for (i, code_line) in highlighted.into_iter().enumerate() {
                                let mut spans =
                                    vec![Span::styled("  ", Style::default().bg(CODE_BG))];
                                spans.extend(code_line.spans);

                                // Add language label on first line (will be right-aligned at render)
                                let is_first = i == 0;
                                lines.push(RenderedLine::code_with_lang(
                                    Line::from(spans).style(Style::default().bg(CODE_BG)),
                                    if is_first {
                                        Some(code_block_lang.clone())
                                    } else {
                                        None
                                    },
                                ));
                            }

                            lines.push(RenderedLine::text(Line::from("")));
                            code_block_content.clear();
                        }
                        TagEnd::Item => {
                            if !current_spans.is_empty() {
                                lines.push(RenderedLine::text(Line::from(std::mem::take(
                                    &mut current_spans,
                                ))));
                            }
                        }
                        TagEnd::TableCell => {
                            current_row.push(std::mem::take(&mut current_cell));
                        }
                        TagEnd::TableHead => {
                            if !current_row.is_empty() {
                                table_rows.push(std::mem::take(&mut current_row));
                            }
                        }
                        TagEnd::TableRow => {
                            if !current_row.is_empty() {
                                table_rows.push(std::mem::take(&mut current_row));
                            }
                        }
                        TagEnd::Table => {
                            // Render the table
                            if !table_rows.is_empty() {
                                // Calculate column widths
                                let col_count =
                                    table_rows.iter().map(|r| r.len()).max().unwrap_or(0);
                                let mut col_widths = vec![0usize; col_count];
                                for row in &table_rows {
                                    for (i, cell) in row.iter().enumerate() {
                                        col_widths[i] = col_widths[i].max(cell.len());
                                    }
                                }

                                // Render header row
                                if let Some(header) = table_rows.first() {
                                    let mut spans = Vec::new();
                                    for (i, cell) in header.iter().enumerate() {
                                        let width = col_widths.get(i).copied().unwrap_or(0);
                                        spans.push(Span::styled(
                                            format!(" {:width$} ", cell, width = width),
                                            Style::default()
                                                .fg(Color::Cyan)
                                                .add_modifier(Modifier::BOLD),
                                        ));
                                        if i < header.len() - 1 {
                                            spans.push(Span::styled(
                                                "│",
                                                Style::default().fg(Color::DarkGray),
                                            ));
                                        }
                                    }
                                    lines.push(RenderedLine::text(Line::from(spans)));

                                    // Separator line
                                    let sep: String = col_widths
                                        .iter()
                                        .map(|w| "─".repeat(w + 2))
                                        .collect::<Vec<_>>()
                                        .join("┼");
                                    lines.push(RenderedLine::text(Line::from(Span::styled(
                                        sep,
                                        Style::default().fg(Color::DarkGray),
                                    ))));
                                }

                                // Render data rows
                                for row in table_rows.iter().skip(1) {
                                    let mut spans = Vec::new();
                                    for (i, cell) in row.iter().enumerate() {
                                        let width = col_widths.get(i).copied().unwrap_or(0);
                                        spans.push(Span::styled(
                                            format!(" {:width$} ", cell, width = width),
                                            Style::default(),
                                        ));
                                        if i < row.len() - 1 {
                                            spans.push(Span::styled(
                                                "│",
                                                Style::default().fg(Color::DarkGray),
                                            ));
                                        }
                                    }
                                    lines.push(RenderedLine::text(Line::from(spans)));
                                }

                                lines.push(RenderedLine::text(Line::from("")));
                            }
                            in_table = false;
                            table_rows.clear();
                        }
                        _ => {}
                    }
                }
                Event::Text(text) => {
                    if in_code_block {
                        code_block_content.push_str(&text);
                    } else if in_table {
                        current_cell.push_str(&text);
                    } else {
                        let style = *style_stack.last().unwrap_or(&Style::default());
                        current_spans.push(Span::styled(text.to_string(), style));
                    }
                }
                Event::Code(code) => {
                    if in_table {
                        current_cell.push_str(&format!("`{}`", code));
                    } else {
                        let style = Style::default()
                            .fg(Color::Rgb(220, 180, 100))
                            .bg(Color::Rgb(40, 40, 50));
                        current_spans.push(Span::styled(format!(" {} ", code), style));
                    }
                }
                Event::SoftBreak => {
                    current_spans.push(Span::raw(" "));
                }
                Event::HardBreak => {
                    if !current_spans.is_empty() {
                        lines.push(RenderedLine::text(Line::from(std::mem::take(
                            &mut current_spans,
                        ))));
                    }
                }
                Event::Rule => {
                    if !current_spans.is_empty() {
                        lines.push(RenderedLine::text(Line::from(std::mem::take(
                            &mut current_spans,
                        ))));
                    }
                    lines.push(RenderedLine::text(Line::from(Span::styled(
                        "─".repeat(40),
                        Style::default().fg(Color::DarkGray),
                    ))));
                    lines.push(RenderedLine::text(Line::from("")));
                }
                _ => {}
            }
        }

        // Flush remaining spans
        if !current_spans.is_empty() {
            lines.push(RenderedLine::text(Line::from(current_spans)));
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
        for (i, rendered) in self.rendered_lines.iter().enumerate() {
            let line_text: String = rendered
                .line
                .spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect();
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
