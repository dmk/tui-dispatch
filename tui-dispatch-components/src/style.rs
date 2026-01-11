//! Shared styling types for tui-dispatch-components
//!
//! All component styles follow a standard pattern with these common fields:
//! - `border: Option<BorderStyle>` - optional border configuration
//! - `padding: Padding` - inner padding
//! - `bg: Option<Color>` - background color
//!
//! Use the [`ComponentStyle`] trait to access these in generic code.

pub use ratatui::style::{Color, Modifier, Style};
pub use ratatui::widgets::Borders;

/// Trait for component styles that follow the standard pattern.
///
/// All component styles in this crate implement this trait, ensuring
/// consistent access to common styling fields.
pub trait ComponentStyle {
    /// Get border configuration
    fn border(&self) -> Option<&BorderStyle>;
    /// Get padding
    fn padding(&self) -> &Padding;
    /// Get background color
    fn bg(&self) -> Option<Color>;
}

/// Padding configuration for components
#[derive(Debug, Clone, Copy, Default)]
pub struct Padding {
    pub top: u16,
    pub right: u16,
    pub bottom: u16,
    pub left: u16,
}

impl Padding {
    /// Create padding with the same value on all sides
    pub fn all(v: u16) -> Self {
        Self {
            top: v,
            right: v,
            bottom: v,
            left: v,
        }
    }

    /// Create padding with horizontal and vertical values
    pub fn xy(x: u16, y: u16) -> Self {
        Self {
            top: y,
            right: x,
            bottom: y,
            left: x,
        }
    }

    /// Create padding with individual values for each side
    pub fn new(top: u16, right: u16, bottom: u16, left: u16) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// Total horizontal padding (left + right)
    pub fn horizontal(&self) -> u16 {
        self.left + self.right
    }

    /// Total vertical padding (top + bottom)
    pub fn vertical(&self) -> u16 {
        self.top + self.bottom
    }
}

/// Border styling configuration
#[derive(Debug, Clone)]
pub struct BorderStyle {
    /// Which borders to show
    pub borders: Borders,
    /// Default border style
    pub style: Style,
    /// Style override when focused (if None, uses `style`)
    pub focused_style: Option<Style>,
}

impl Default for BorderStyle {
    fn default() -> Self {
        Self {
            borders: Borders::ALL,
            style: Style::default().fg(Color::DarkGray),
            focused_style: Some(Style::default().fg(Color::Cyan)),
        }
    }
}

impl BorderStyle {
    /// Create a border style with all borders
    pub fn all() -> Self {
        Self::default()
    }

    /// Create a border style with no borders
    pub fn none() -> Self {
        Self {
            borders: Borders::NONE,
            ..Default::default()
        }
    }

    /// Get the appropriate style based on focus state
    pub fn style_for_focus(&self, is_focused: bool) -> Style {
        if is_focused {
            self.focused_style.unwrap_or(self.style)
        } else {
            self.style
        }
    }
}

/// Selection styling for list components
#[derive(Debug, Clone)]
pub struct SelectionStyle {
    /// Style applied to selected item (default: Cyan + Bold)
    pub style: Option<Style>,
    /// Prefix marker for selected item (default: "> ")
    pub marker: Option<&'static str>,
    /// Set to true to disable all component selection styling
    /// (user handles it entirely in their Line rendering)
    pub disabled: bool,
}

impl Default for SelectionStyle {
    fn default() -> Self {
        Self {
            style: Some(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            marker: Some("> "),
            disabled: false,
        }
    }
}

impl SelectionStyle {
    /// Create selection style with no automatic styling (user handles it)
    pub fn disabled() -> Self {
        Self {
            style: None,
            marker: None,
            disabled: true,
        }
    }

    /// Create selection style with only a marker, no style change
    pub fn marker_only(marker: &'static str) -> Self {
        Self {
            style: None,
            marker: Some(marker),
            disabled: false,
        }
    }

    /// Create selection style with only a style change, no marker
    pub fn style_only(style: Style) -> Self {
        Self {
            style: Some(style),
            marker: None,
            disabled: false,
        }
    }
}

// ============================================================================
// Utility functions
// ============================================================================

use ratatui::text::{Line, Span};

/// Highlight substring matches in text (case-insensitive)
///
/// Returns a `Line` with matching portions styled using `highlight_style`.
/// Non-matching portions use the `base_style`.
///
/// # Example
///
/// ```ignore
/// use tui_dispatch_components::style::{highlight_substring, Style, Color, Modifier};
///
/// let base = Style::default();
/// let highlight = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
/// let line = highlight_substring("Hello World", "wor", base, highlight);
/// // Results in: "Hello " (base) + "Wor" (highlight) + "ld" (base)
/// ```
///
/// # Notes
///
/// - Matching is case-insensitive
/// - Only works with ASCII text; non-ASCII returns the text with base style
/// - Empty query returns the text with base style
pub fn highlight_substring(
    text: &str,
    query: &str,
    base_style: Style,
    highlight_style: Style,
) -> Line<'static> {
    if query.is_empty() {
        return Line::styled(text.to_string(), base_style);
    }

    // Fall back for non-ASCII to avoid indexing issues
    if !text.is_ascii() || !query.is_ascii() {
        return Line::styled(text.to_string(), base_style);
    }

    let text_lower = text.to_lowercase();
    let query_lower = query.to_lowercase();

    let mut spans = Vec::new();
    let mut last_end = 0;

    for (start, _) in text_lower.match_indices(&query_lower) {
        // Add non-matching part before this match
        if start > last_end {
            spans.push(Span::styled(text[last_end..start].to_string(), base_style));
        }

        // Add matching part with highlight
        let end = start + query.len();
        spans.push(Span::styled(text[start..end].to_string(), highlight_style));
        last_end = end;
    }

    // Add remaining part after last match
    if last_end < text.len() {
        spans.push(Span::styled(text[last_end..].to_string(), base_style));
    }

    if spans.is_empty() {
        Line::styled(text.to_string(), base_style)
    } else {
        Line::from(spans)
    }
}
