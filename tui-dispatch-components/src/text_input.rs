//! Single-line text input component

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Paragraph},
    Frame,
};
use tui_dispatch_core::{Component, EventKind};

use crate::style::{BorderStyle, ComponentStyle, Padding};

/// Unified styling for TextInput
#[derive(Debug, Clone)]
pub struct InputStyle {
    /// Border configuration (None = no border)
    pub border: Option<BorderStyle>,
    /// Padding inside the component
    pub padding: Padding,
    /// Background color
    pub bg: Option<Color>,
    /// Foreground (text) color
    pub fg: Option<Color>,
    /// Style for placeholder text
    pub placeholder_style: Option<Style>,
    /// Style for cursor (when focused)
    pub cursor_style: Option<Style>,
}

impl Default for InputStyle {
    fn default() -> Self {
        Self {
            border: Some(BorderStyle::default()),
            padding: Padding::default(),
            bg: None,
            fg: None,
            placeholder_style: Some(Style::default().fg(Color::DarkGray)),
            cursor_style: None,
        }
    }
}

impl InputStyle {
    /// Create a style with no border
    pub fn borderless() -> Self {
        Self {
            border: None,
            ..Default::default()
        }
    }

    /// Create a minimal style (no border, no padding)
    pub fn minimal() -> Self {
        Self {
            border: None,
            padding: Padding::default(),
            bg: None,
            fg: None,
            placeholder_style: Some(Style::default().fg(Color::DarkGray)),
            cursor_style: None,
        }
    }
}

impl ComponentStyle for InputStyle {
    fn border(&self) -> Option<&BorderStyle> {
        self.border.as_ref()
    }
    fn padding(&self) -> &Padding {
        &self.padding
    }
    fn bg(&self) -> Option<Color> {
        self.bg
    }
}

/// Props for TextInput component
pub struct TextInputProps<'a, A> {
    /// Current input value
    pub value: &'a str,
    /// Placeholder text when empty
    pub placeholder: &'a str,
    /// Whether this component has focus
    pub is_focused: bool,
    /// Unified styling
    pub style: InputStyle,
    /// Callback when value changes
    pub on_change: fn(String) -> A,
    /// Callback when user submits (Enter)
    pub on_submit: fn(String) -> A,
}

/// A single-line text input with cursor
///
/// Handles typing, backspace, delete, and cursor movement.
/// Emits on_change for each keystroke and on_submit for Enter.
#[derive(Default)]
pub struct TextInput {
    /// Cursor position (byte index)
    cursor: usize,
}

impl TextInput {
    /// Create a new TextInput
    pub fn new() -> Self {
        Self::default()
    }

    /// Clamp cursor to valid range for the given value
    fn clamp_cursor(&mut self, value: &str) {
        self.cursor = self.cursor.min(value.len());
    }

    /// Move cursor left by one character
    fn move_cursor_left(&mut self, value: &str) {
        if self.cursor > 0 {
            // Find previous char boundary
            let mut new_pos = self.cursor - 1;
            while new_pos > 0 && !value.is_char_boundary(new_pos) {
                new_pos -= 1;
            }
            self.cursor = new_pos;
        }
    }

    /// Move cursor right by one character
    fn move_cursor_right(&mut self, value: &str) {
        if self.cursor < value.len() {
            // Find next char boundary
            let mut new_pos = self.cursor + 1;
            while new_pos < value.len() && !value.is_char_boundary(new_pos) {
                new_pos += 1;
            }
            self.cursor = new_pos;
        }
    }

    /// Insert character at cursor position
    fn insert_char(&mut self, value: &str, c: char) -> String {
        let mut new_value = String::with_capacity(value.len() + c.len_utf8());
        new_value.push_str(&value[..self.cursor]);
        new_value.push(c);
        new_value.push_str(&value[self.cursor..]);
        self.cursor += c.len_utf8();
        new_value
    }

    /// Delete character before cursor (backspace)
    fn delete_char_before(&mut self, value: &str) -> Option<String> {
        if self.cursor == 0 {
            return None;
        }

        let mut new_value = String::with_capacity(value.len());
        let before_cursor = &value[..self.cursor];

        // Find the previous character boundary
        let char_start = before_cursor
            .char_indices()
            .last()
            .map(|(i, _)| i)
            .unwrap_or(0);

        new_value.push_str(&value[..char_start]);
        new_value.push_str(&value[self.cursor..]);
        self.cursor = char_start;
        Some(new_value)
    }

    /// Delete character at cursor (delete key)
    fn delete_char_at(&self, value: &str) -> Option<String> {
        if self.cursor >= value.len() {
            return None;
        }

        let mut new_value = String::with_capacity(value.len());
        new_value.push_str(&value[..self.cursor]);

        // Find the next character boundary
        let after_cursor = &value[self.cursor..];
        if let Some((_, c)) = after_cursor.char_indices().next() {
            new_value.push_str(&value[self.cursor + c.len_utf8()..]);
        }

        Some(new_value)
    }
}

impl<A> Component<A> for TextInput {
    type Props<'a> = TextInputProps<'a, A>;

    fn handle_event(
        &mut self,
        event: &EventKind,
        props: Self::Props<'_>,
    ) -> impl IntoIterator<Item = A> {
        if !props.is_focused {
            return None;
        }

        // Ensure cursor is valid for current value
        self.clamp_cursor(props.value);

        match event {
            EventKind::Key(key) => {
                // Handle Ctrl+key shortcuts
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    return match key.code {
                        // Ctrl+A: move to start
                        KeyCode::Char('a') => {
                            self.cursor = 0;
                            None
                        }
                        // Ctrl+E: move to end
                        KeyCode::Char('e') => {
                            self.cursor = props.value.len();
                            None
                        }
                        // Ctrl+U: clear line
                        KeyCode::Char('u') => {
                            self.cursor = 0;
                            Some((props.on_change)(String::new()))
                        }
                        _ => None,
                    };
                }

                match key.code {
                    // Character input
                    KeyCode::Char(c) => {
                        let new_value = self.insert_char(props.value, c);
                        Some((props.on_change)(new_value))
                    }
                    // Backspace
                    KeyCode::Backspace => self
                        .delete_char_before(props.value)
                        .map(|v| (props.on_change)(v)),
                    // Delete
                    KeyCode::Delete => self
                        .delete_char_at(props.value)
                        .map(|v| (props.on_change)(v)),
                    // Cursor movement
                    KeyCode::Left => {
                        self.move_cursor_left(props.value);
                        None
                    }
                    KeyCode::Right => {
                        self.move_cursor_right(props.value);
                        None
                    }
                    KeyCode::Home => {
                        self.cursor = 0;
                        None
                    }
                    KeyCode::End => {
                        self.cursor = props.value.len();
                        None
                    }
                    // Submit
                    KeyCode::Enter => Some((props.on_submit)(props.value.to_string())),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        let style = &props.style;

        // Ensure cursor is valid
        self.clamp_cursor(props.value);

        // Fill background if color provided
        if let Some(bg) = style.bg {
            for y in area.y..area.y.saturating_add(area.height) {
                for x in area.x..area.x.saturating_add(area.width) {
                    frame.buffer_mut()[(x, y)].set_bg(bg);
                    frame.buffer_mut()[(x, y)].set_symbol(" ");
                }
            }
        }

        // Apply padding
        let content_area = Rect {
            x: area.x + style.padding.left,
            y: area.y + style.padding.top,
            width: area.width.saturating_sub(style.padding.horizontal()),
            height: area.height.saturating_sub(style.padding.vertical()),
        };

        // Determine display text
        let display_text = if props.value.is_empty() {
            props.placeholder
        } else {
            props.value
        };

        // Build text style
        let mut text_style = if props.value.is_empty() {
            style
                .placeholder_style
                .unwrap_or_else(|| Style::default().fg(Color::DarkGray))
        } else {
            let mut s = Style::default();
            if let Some(fg) = style.fg {
                s = s.fg(fg);
            }
            s
        };

        // Preserve background color in text style
        if let Some(bg) = style.bg {
            text_style = text_style.bg(bg);
        }

        let mut paragraph = Paragraph::new(display_text).style(text_style);

        if let Some(border) = &style.border {
            paragraph = paragraph.block(
                Block::default()
                    .borders(border.borders)
                    .border_style(border.style_for_focus(props.is_focused)),
            );
        }

        frame.render_widget(paragraph, content_area);

        // Show cursor if focused
        if props.is_focused {
            // Calculate cursor screen position (account for border and padding)
            let border_offset = if style.border.is_some() { 1 } else { 0 };
            let cursor_x = content_area.x + border_offset + self.cursor as u16;
            let cursor_y = content_area.y + border_offset;

            // Only show cursor if within bounds
            let max_x = if style.border.is_some() {
                content_area.x + content_area.width - 1
            } else {
                content_area.x + content_area.width
            };
            if cursor_x < max_x {
                if let Some(cursor_style) = style.cursor_style {
                    frame.buffer_mut()[(cursor_x, cursor_y)].set_style(cursor_style);
                }
                frame.set_cursor_position((cursor_x, cursor_y));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tui_dispatch_core::testing::{key, RenderHarness};

    #[derive(Debug, Clone, PartialEq)]
    enum TestAction {
        Change(String),
        Submit(String),
    }

    #[test]
    fn test_typing() {
        let mut input = TextInput::new();
        let props = TextInputProps {
            value: "",
            placeholder: "",
            is_focused: true,
            style: InputStyle::default(),
            on_change: TestAction::Change,
            on_submit: TestAction::Submit,
        };

        let actions: Vec<_> = input
            .handle_event(&EventKind::Key(key("a")), props)
            .into_iter()
            .collect();

        assert_eq!(actions, vec![TestAction::Change("a".into())]);
    }

    #[test]
    fn test_typing_appends() {
        let mut input = TextInput::new();
        input.cursor = 5; // At end of "hello"

        let props = TextInputProps {
            value: "hello",
            placeholder: "",
            is_focused: true,
            style: InputStyle::default(),
            on_change: TestAction::Change,
            on_submit: TestAction::Submit,
        };

        let actions: Vec<_> = input
            .handle_event(&EventKind::Key(key("!")), props)
            .into_iter()
            .collect();

        assert_eq!(actions, vec![TestAction::Change("hello!".into())]);
    }

    #[test]
    fn test_backspace() {
        let mut input = TextInput::new();
        input.cursor = 5;

        let props = TextInputProps {
            value: "hello",
            placeholder: "",
            is_focused: true,
            style: InputStyle::default(),
            on_change: TestAction::Change,
            on_submit: TestAction::Submit,
        };

        let actions: Vec<_> = input
            .handle_event(&EventKind::Key(key("backspace")), props)
            .into_iter()
            .collect();

        assert_eq!(actions, vec![TestAction::Change("hell".into())]);
        assert_eq!(input.cursor, 4);
    }

    #[test]
    fn test_backspace_at_start() {
        let mut input = TextInput::new();
        input.cursor = 0;

        let props = TextInputProps {
            value: "hello",
            placeholder: "",
            is_focused: true,
            style: InputStyle::default(),
            on_change: TestAction::Change,
            on_submit: TestAction::Submit,
        };

        let actions: Vec<_> = input
            .handle_event(&EventKind::Key(key("backspace")), props)
            .into_iter()
            .collect();

        assert!(actions.is_empty());
    }

    #[test]
    fn test_submit() {
        let mut input = TextInput::new();

        let props = TextInputProps {
            value: "hello",
            placeholder: "",
            is_focused: true,
            style: InputStyle::default(),
            on_change: TestAction::Change,
            on_submit: TestAction::Submit,
        };

        let actions: Vec<_> = input
            .handle_event(&EventKind::Key(key("enter")), props)
            .into_iter()
            .collect();

        assert_eq!(actions, vec![TestAction::Submit("hello".into())]);
    }

    #[test]
    fn test_unfocused_ignores() {
        let mut input = TextInput::new();

        let props = TextInputProps {
            value: "",
            placeholder: "",
            is_focused: false,
            style: InputStyle::default(),
            on_change: TestAction::Change,
            on_submit: TestAction::Submit,
        };

        let actions: Vec<_> = input
            .handle_event(&EventKind::Key(key("a")), props)
            .into_iter()
            .collect();

        assert!(actions.is_empty());
    }

    #[test]
    fn test_render_with_value() {
        let mut render = RenderHarness::new(30, 3);
        let mut input = TextInput::new();

        let output = render.render_to_string_plain(|frame| {
            let props = TextInputProps {
                value: "hello",
                placeholder: "Type here...",
                is_focused: true,
                style: InputStyle::default(),
                on_change: |_| (),
                on_submit: |_| (),
            };
            input.render(frame, frame.area(), props);
        });

        assert!(output.contains("hello"));
    }

    #[test]
    fn test_render_placeholder() {
        let mut render = RenderHarness::new(30, 3);
        let mut input = TextInput::new();

        let output = render.render_to_string_plain(|frame| {
            let props = TextInputProps {
                value: "",
                placeholder: "Type here...",
                is_focused: true,
                style: InputStyle::default(),
                on_change: |_| (),
                on_submit: |_| (),
            };
            input.render(frame, frame.area(), props);
        });

        assert!(output.contains("Type here..."));
    }

    #[test]
    fn test_render_with_custom_style() {
        let mut render = RenderHarness::new(30, 3);
        let mut input = TextInput::new();

        let output = render.render_to_string_plain(|frame| {
            let props = TextInputProps {
                value: "test",
                placeholder: "",
                is_focused: true,
                style: InputStyle {
                    border: None,
                    padding: Padding::xy(1, 0),
                    bg: Some(Color::Blue),
                    fg: Some(Color::White),
                    placeholder_style: None,
                    cursor_style: None,
                },
                on_change: |_| (),
                on_submit: |_| (),
            };
            input.render(frame, frame.area(), props);
        });

        assert!(output.contains("test"));
    }
}
