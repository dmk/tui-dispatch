//! Single-line text input component

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use tui_dispatch_core::{Component, EventKind};

/// Props for TextInput component
pub struct TextInputProps<'a, A> {
    /// Current input value
    pub value: &'a str,
    /// Placeholder text when empty
    pub placeholder: &'a str,
    /// Whether this component has focus
    pub is_focused: bool,
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
        // Ensure cursor is valid
        self.clamp_cursor(props.value);

        // Determine display text
        let display_text = if props.value.is_empty() {
            props.placeholder
        } else {
            props.value
        };

        let style = if props.value.is_empty() {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default()
        };

        let paragraph = Paragraph::new(display_text).style(style).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(if props.is_focused {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                }),
        );

        frame.render_widget(paragraph, area);

        // Show cursor if focused
        if props.is_focused {
            // Calculate cursor screen position
            // Account for border (1 char) and text before cursor
            let cursor_x = area.x + 1 + self.cursor as u16;
            let cursor_y = area.y + 1;

            // Only show cursor if within bounds
            if cursor_x < area.x + area.width - 1 {
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
                on_change: |_| (),
                on_submit: |_| (),
            };
            input.render(frame, frame.area(), props);
        });

        assert!(output.contains("Type here..."));
    }
}
