//! Single-line text input component

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Paragraph},
    Frame,
};
use tui_dispatch_core::{Component, EventKind};

use crate::style::{BaseStyle, ComponentStyle, Padding};

/// Unified styling for TextInput
#[derive(Debug, Clone)]
pub struct TextInputStyle {
    /// Shared base style
    pub base: BaseStyle,
    /// Style for placeholder text
    pub placeholder_style: Option<Style>,
    /// Style for cursor (when focused)
    pub cursor_style: Option<Style>,
}

impl Default for TextInputStyle {
    fn default() -> Self {
        Self {
            base: BaseStyle {
                fg: None,
                ..Default::default()
            },
            placeholder_style: Some(Style::default().fg(Color::DarkGray)),
            cursor_style: None,
        }
    }
}

impl TextInputStyle {
    /// Create a style with no border
    pub fn borderless() -> Self {
        let mut style = Self::default();
        style.base.border = None;
        style
    }

    /// Create a minimal style (no border, no padding)
    pub fn minimal() -> Self {
        let mut style = Self::default();
        style.base.border = None;
        style.base.padding = Padding::default();
        style
    }
}

impl ComponentStyle for TextInputStyle {
    fn base(&self) -> &BaseStyle {
        &self.base
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
    pub style: TextInputStyle,
    /// Callback when value changes
    pub on_change: fn(String) -> A,
    /// Callback when user submits (Enter)
    pub on_submit: fn(String) -> A,
    /// Callback when cursor moves without content change
    pub on_cursor_move: Option<fn(usize) -> A>,
}

/// A single-line text input with cursor
///
/// Handles typing, backspace, delete, and cursor movement.
/// Emits on_change for each keystroke, on_submit for Enter,
/// and on_cursor_move for cursor-only movement if provided.
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

    // ========================================================================
    // Word-based operations (readline/emacs style)
    // ========================================================================

    /// Find the byte position of the previous word boundary
    fn prev_word_boundary(&self, value: &str) -> usize {
        if self.cursor == 0 {
            return 0;
        }

        let before = &value[..self.cursor];
        let mut chars: Vec<(usize, char)> = before.char_indices().collect();

        // Skip trailing non-word, non-whitespace (punctuation)
        while let Some(&(_, c)) = chars.last() {
            if c.is_alphanumeric() || c == '_' || c.is_whitespace() {
                break;
            }
            chars.pop();
        }

        // Skip whitespace
        while let Some(&(_, c)) = chars.last() {
            if !c.is_whitespace() {
                break;
            }
            chars.pop();
        }

        // Skip word characters (alphanumeric or _)
        while let Some(&(_, c)) = chars.last() {
            if !c.is_alphanumeric() && c != '_' {
                break;
            }
            chars.pop();
        }

        chars.last().map(|&(i, c)| i + c.len_utf8()).unwrap_or(0)
    }

    /// Find the byte position of the next word boundary
    fn next_word_boundary(&self, value: &str) -> usize {
        if self.cursor >= value.len() {
            return value.len();
        }

        let after = &value[self.cursor..];
        let mut pos = self.cursor;

        let mut chars = after.chars().peekable();

        // Skip current word characters
        while let Some(&c) = chars.peek() {
            if !c.is_alphanumeric() && c != '_' {
                break;
            }
            pos += c.len_utf8();
            chars.next();
        }

        // Skip whitespace
        while let Some(&c) = chars.peek() {
            if !c.is_whitespace() {
                break;
            }
            pos += c.len_utf8();
            chars.next();
        }

        // If we haven't moved (started on whitespace/punctuation), skip to next word
        if pos == self.cursor {
            // Skip non-word, non-whitespace
            while let Some(&c) = chars.peek() {
                if c.is_alphanumeric() || c == '_' || c.is_whitespace() {
                    break;
                }
                pos += c.len_utf8();
                chars.next();
            }
            // Skip whitespace
            while let Some(&c) = chars.peek() {
                if !c.is_whitespace() {
                    break;
                }
                pos += c.len_utf8();
                chars.next();
            }
        }

        pos
    }

    /// Move cursor backward by one word
    fn move_word_backward(&mut self, value: &str) {
        self.cursor = self.prev_word_boundary(value);
    }

    /// Move cursor forward by one word
    fn move_word_forward(&mut self, value: &str) {
        self.cursor = self.next_word_boundary(value);
    }

    /// Delete from cursor to end of line (Ctrl+K)
    fn kill_line(&self, value: &str) -> Option<String> {
        if self.cursor >= value.len() {
            return None;
        }
        Some(value[..self.cursor].to_string())
    }

    /// Delete word backward (Ctrl+W / Alt+Backspace)
    fn kill_word_backward(&mut self, value: &str) -> Option<String> {
        let boundary = self.prev_word_boundary(value);
        if boundary == self.cursor {
            return None;
        }

        let mut new_value = String::with_capacity(value.len());
        new_value.push_str(&value[..boundary]);
        new_value.push_str(&value[self.cursor..]);
        self.cursor = boundary;
        Some(new_value)
    }

    /// Delete word forward (Alt+D)
    fn kill_word_forward(&self, value: &str) -> Option<String> {
        let boundary = self.next_word_boundary(value);
        if boundary == self.cursor {
            return None;
        }

        let mut new_value = String::with_capacity(value.len());
        new_value.push_str(&value[..self.cursor]);
        new_value.push_str(&value[boundary..]);
        Some(new_value)
    }

    /// Transpose characters at cursor (Ctrl+T)
    fn transpose_chars(&mut self, value: &str) -> Option<String> {
        // Need at least 2 characters and cursor not at start
        if value.len() < 2 || self.cursor == 0 {
            return None;
        }

        // If at end, transpose last two chars
        let pos = if self.cursor >= value.len() {
            // Find start of second-to-last char
            let mut idx = value.len();
            let mut count = 0;
            for (i, _) in value.char_indices().rev() {
                idx = i;
                count += 1;
                if count == 2 {
                    break;
                }
            }
            idx
        } else {
            // Find start of char before cursor
            let before = &value[..self.cursor];
            before.char_indices().last().map(|(i, _)| i).unwrap_or(0)
        };

        // Get the two characters to swap
        let chars: Vec<char> = value[pos..].chars().take(2).collect();
        if chars.len() < 2 {
            return None;
        }

        let mut new_value = String::with_capacity(value.len());
        new_value.push_str(&value[..pos]);
        new_value.push(chars[1]);
        new_value.push(chars[0]);
        new_value.push_str(&value[pos + chars[0].len_utf8() + chars[1].len_utf8()..]);

        // Move cursor forward if not at end
        if self.cursor < value.len() {
            self.cursor += chars[1].len_utf8();
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
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                let alt = key.modifiers.contains(KeyModifiers::ALT);
                let mut did_move = false;

                let action = match (key.code, ctrl, alt) {
                    // ============================================================
                    // Ctrl+key shortcuts (readline/emacs style)
                    // ============================================================

                    // Movement
                    (KeyCode::Char('a'), true, false) => {
                        self.cursor = 0;
                        did_move = true;
                        None
                    }
                    (KeyCode::Char('e'), true, false) => {
                        self.cursor = props.value.len();
                        did_move = true;
                        None
                    }
                    (KeyCode::Char('b'), true, false) => {
                        self.move_cursor_left(props.value);
                        did_move = true;
                        None
                    }
                    (KeyCode::Char('f'), true, false) => {
                        self.move_cursor_right(props.value);
                        did_move = true;
                        None
                    }

                    // Word movement (Ctrl+Arrow - Mac friendly)
                    (KeyCode::Left, true, false) => {
                        self.move_word_backward(props.value);
                        did_move = true;
                        None
                    }
                    (KeyCode::Right, true, false) => {
                        self.move_word_forward(props.value);
                        did_move = true;
                        None
                    }

                    // Deletion
                    (KeyCode::Char('u'), true, false) => {
                        self.cursor = 0;
                        Some((props.on_change)(String::new()))
                    }
                    (KeyCode::Char('k'), true, false) => {
                        self.kill_line(props.value).map(|v| (props.on_change)(v))
                    }
                    (KeyCode::Char('w'), true, false) => self
                        .kill_word_backward(props.value)
                        .map(|v| (props.on_change)(v)),
                    (KeyCode::Char('d'), true, false) => self
                        .delete_char_at(props.value)
                        .map(|v| (props.on_change)(v)),
                    (KeyCode::Char('h'), true, false) => self
                        .delete_char_before(props.value)
                        .map(|v| (props.on_change)(v)),

                    // Transpose
                    (KeyCode::Char('t'), true, false) => self
                        .transpose_chars(props.value)
                        .map(|v| (props.on_change)(v)),

                    // ============================================================
                    // Alt+key shortcuts (when terminal sends escape sequences)
                    // ============================================================

                    // Word movement
                    (KeyCode::Char('b'), false, true) => {
                        self.move_word_backward(props.value);
                        did_move = true;
                        None
                    }
                    (KeyCode::Char('f'), false, true) => {
                        self.move_word_forward(props.value);
                        did_move = true;
                        None
                    }

                    // Word deletion
                    (KeyCode::Char('d'), false, true) => self
                        .kill_word_forward(props.value)
                        .map(|v| (props.on_change)(v)),
                    (KeyCode::Backspace, false, true) => self
                        .kill_word_backward(props.value)
                        .map(|v| (props.on_change)(v)),

                    // ============================================================
                    // Basic keys
                    // ============================================================

                    // Backspace (no modifiers - Alt+Backspace handled above)
                    (KeyCode::Backspace, false, false) => self
                        .delete_char_before(props.value)
                        .map(|v| (props.on_change)(v)),

                    // Delete
                    (KeyCode::Delete, _, _) => self
                        .delete_char_at(props.value)
                        .map(|v| (props.on_change)(v)),

                    // Cursor movement (no Ctrl - Ctrl+Arrow handled above)
                    (KeyCode::Left, false, _) => {
                        self.move_cursor_left(props.value);
                        did_move = true;
                        None
                    }
                    (KeyCode::Right, false, _) => {
                        self.move_cursor_right(props.value);
                        did_move = true;
                        None
                    }
                    (KeyCode::Home, _, _) => {
                        self.cursor = 0;
                        did_move = true;
                        None
                    }
                    (KeyCode::End, _, _) => {
                        self.cursor = props.value.len();
                        did_move = true;
                        None
                    }

                    // Submit
                    (KeyCode::Enter, _, _) => Some((props.on_submit)(props.value.to_string())),

                    // ============================================================
                    // Character input - MUST be last to catch all printable chars
                    // ============================================================
                    // Any character not handled above gets inserted (space, etc.)
                    (KeyCode::Char(c), _, _) => {
                        let new_value = self.insert_char(props.value, c);
                        Some((props.on_change)(new_value))
                    }

                    _ => None,
                };

                if action.is_none() && did_move {
                    props.on_cursor_move.map(|callback| callback(self.cursor))
                } else {
                    action
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
        if let Some(bg) = style.base.bg {
            for y in area.y..area.y.saturating_add(area.height) {
                for x in area.x..area.x.saturating_add(area.width) {
                    frame.buffer_mut()[(x, y)].set_bg(bg);
                    frame.buffer_mut()[(x, y)].set_symbol(" ");
                }
            }
        }

        // Apply padding
        let content_area = Rect {
            x: area.x + style.base.padding.left,
            y: area.y + style.base.padding.top,
            width: area.width.saturating_sub(style.base.padding.horizontal()),
            height: area.height.saturating_sub(style.base.padding.vertical()),
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
            if let Some(fg) = style.base.fg {
                s = s.fg(fg);
            }
            s
        };

        // Preserve background color in text style
        if let Some(bg) = style.base.bg {
            text_style = text_style.bg(bg);
        }

        let mut paragraph = Paragraph::new(display_text).style(text_style);

        if let Some(border) = &style.base.border {
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
            let border_offset = if style.base.border.is_some() { 1 } else { 0 };
            let cursor_x = content_area.x + border_offset + self.cursor as u16;
            let cursor_y = content_area.y + border_offset;

            // Only show cursor if within bounds
            let max_x = if style.base.border.is_some() {
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
            style: TextInputStyle::default(),
            on_change: TestAction::Change,
            on_submit: TestAction::Submit,
            on_cursor_move: None,
        };

        let actions: Vec<_> = input
            .handle_event(&EventKind::Key(key("a")), props)
            .into_iter()
            .collect();

        assert_eq!(actions, vec![TestAction::Change("a".into())]);
    }

    #[test]
    fn test_typing_space() {
        let mut input = TextInput::new();
        input.cursor = 5; // After "hello"

        let props = TextInputProps {
            value: "hello",
            placeholder: "",
            is_focused: true,
            style: TextInputStyle::default(),
            on_change: TestAction::Change,
            on_submit: TestAction::Submit,
            on_cursor_move: None,
        };

        // Space character
        let space_key = crossterm::event::KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);

        let actions: Vec<_> = input
            .handle_event(&EventKind::Key(space_key), props)
            .into_iter()
            .collect();

        assert_eq!(actions, vec![TestAction::Change("hello ".into())]);
    }

    #[test]
    fn test_typing_appends() {
        let mut input = TextInput::new();
        input.cursor = 5; // At end of "hello"

        let props = TextInputProps {
            value: "hello",
            placeholder: "",
            is_focused: true,
            style: TextInputStyle::default(),
            on_change: TestAction::Change,
            on_submit: TestAction::Submit,
            on_cursor_move: None,
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
            style: TextInputStyle::default(),
            on_change: TestAction::Change,
            on_submit: TestAction::Submit,
            on_cursor_move: None,
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
            style: TextInputStyle::default(),
            on_change: TestAction::Change,
            on_submit: TestAction::Submit,
            on_cursor_move: None,
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
            style: TextInputStyle::default(),
            on_change: TestAction::Change,
            on_submit: TestAction::Submit,
            on_cursor_move: None,
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
            style: TextInputStyle::default(),
            on_change: TestAction::Change,
            on_submit: TestAction::Submit,
            on_cursor_move: None,
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
                style: TextInputStyle::default(),
                on_change: |_| (),
                on_submit: |_| (),
                on_cursor_move: None,
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
                style: TextInputStyle::default(),
                on_change: |_| (),
                on_submit: |_| (),
                on_cursor_move: None,
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
                style: TextInputStyle {
                    base: BaseStyle {
                        border: None,
                        padding: Padding::xy(1, 0),
                        bg: Some(Color::Blue),
                        fg: Some(Color::White),
                    },
                    placeholder_style: None,
                    cursor_style: None,
                },
                on_change: |_| (),
                on_submit: |_| (),
                on_cursor_move: None,
            };
            input.render(frame, frame.area(), props);
        });

        assert!(output.contains("test"));
    }

    // ========================================================================
    // Readline/Emacs keybinding tests
    // ========================================================================

    fn ctrl_key(c: char) -> crossterm::event::KeyEvent {
        crossterm::event::KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    fn alt_key(c: char) -> crossterm::event::KeyEvent {
        crossterm::event::KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT)
    }

    fn ctrl_arrow(code: KeyCode) -> crossterm::event::KeyEvent {
        crossterm::event::KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    #[test]
    fn test_ctrl_k_kill_line() {
        let mut input = TextInput::new();
        input.cursor = 5; // After "hello"

        let props = TextInputProps {
            value: "hello world",
            placeholder: "",
            is_focused: true,
            style: TextInputStyle::default(),
            on_change: TestAction::Change,
            on_submit: TestAction::Submit,
            on_cursor_move: None,
        };

        let actions: Vec<_> = input
            .handle_event(&EventKind::Key(ctrl_key('k')), props)
            .into_iter()
            .collect();

        assert_eq!(actions, vec![TestAction::Change("hello".into())]);
    }

    #[test]
    fn test_ctrl_w_kill_word_backward() {
        let mut input = TextInput::new();
        input.cursor = 11; // At end of "hello world"

        let props = TextInputProps {
            value: "hello world",
            placeholder: "",
            is_focused: true,
            style: TextInputStyle::default(),
            on_change: TestAction::Change,
            on_submit: TestAction::Submit,
            on_cursor_move: None,
        };

        let actions: Vec<_> = input
            .handle_event(&EventKind::Key(ctrl_key('w')), props)
            .into_iter()
            .collect();

        assert_eq!(actions, vec![TestAction::Change("hello ".into())]);
        assert_eq!(input.cursor, 6);
    }

    #[test]
    fn test_ctrl_left_word_backward() {
        let mut input = TextInput::new();
        input.cursor = 11; // At end of "hello world"

        let props = TextInputProps {
            value: "hello world",
            placeholder: "",
            is_focused: true,
            style: TextInputStyle::default(),
            on_change: TestAction::Change,
            on_submit: TestAction::Submit,
            on_cursor_move: None,
        };

        let actions: Vec<_> = input
            .handle_event(&EventKind::Key(ctrl_arrow(KeyCode::Left)), props)
            .into_iter()
            .collect();

        assert!(actions.is_empty()); // Movement doesn't emit action
        assert_eq!(input.cursor, 6); // At start of "world"
    }

    #[test]
    fn test_ctrl_right_word_forward() {
        let mut input = TextInput::new();
        input.cursor = 0;

        let props = TextInputProps {
            value: "hello world",
            placeholder: "",
            is_focused: true,
            style: TextInputStyle::default(),
            on_change: TestAction::Change,
            on_submit: TestAction::Submit,
            on_cursor_move: None,
        };

        let actions: Vec<_> = input
            .handle_event(&EventKind::Key(ctrl_arrow(KeyCode::Right)), props)
            .into_iter()
            .collect();

        assert!(actions.is_empty());
        assert_eq!(input.cursor, 6); // After "hello "
    }

    #[test]
    fn test_alt_d_kill_word_forward() {
        let mut input = TextInput::new();
        input.cursor = 0;

        let props = TextInputProps {
            value: "hello world",
            placeholder: "",
            is_focused: true,
            style: TextInputStyle::default(),
            on_change: TestAction::Change,
            on_submit: TestAction::Submit,
            on_cursor_move: None,
        };

        let actions: Vec<_> = input
            .handle_event(&EventKind::Key(alt_key('d')), props)
            .into_iter()
            .collect();

        assert_eq!(actions, vec![TestAction::Change("world".into())]);
    }

    #[test]
    fn test_ctrl_t_transpose() {
        let mut input = TextInput::new();
        input.cursor = 2; // Between 'e' and 'l' in "hello"

        let props = TextInputProps {
            value: "hello",
            placeholder: "",
            is_focused: true,
            style: TextInputStyle::default(),
            on_change: TestAction::Change,
            on_submit: TestAction::Submit,
            on_cursor_move: None,
        };

        let actions: Vec<_> = input
            .handle_event(&EventKind::Key(ctrl_key('t')), props)
            .into_iter()
            .collect();

        assert_eq!(actions, vec![TestAction::Change("hlelo".into())]);
    }

    #[test]
    fn test_ctrl_b_f_movement() {
        let mut input = TextInput::new();
        input.cursor = 5;

        let props = TextInputProps {
            value: "hello world",
            placeholder: "",
            is_focused: true,
            style: TextInputStyle::default(),
            on_change: TestAction::Change,
            on_submit: TestAction::Submit,
            on_cursor_move: None,
        };

        // Ctrl+B moves back
        let _: Vec<_> = input
            .handle_event(&EventKind::Key(ctrl_key('b')), props)
            .into_iter()
            .collect();
        assert_eq!(input.cursor, 4);

        // Ctrl+F moves forward
        let props = TextInputProps {
            value: "hello world",
            placeholder: "",
            is_focused: true,
            style: TextInputStyle::default(),
            on_change: TestAction::Change,
            on_submit: TestAction::Submit,
            on_cursor_move: None,
        };
        let _: Vec<_> = input
            .handle_event(&EventKind::Key(ctrl_key('f')), props)
            .into_iter()
            .collect();
        assert_eq!(input.cursor, 5);
    }

    #[test]
    fn test_word_boundary_multiple_spaces() {
        let mut input = TextInput::new();
        input.cursor = 14; // At end of "hello   world" (3 spaces)

        // Test backward over multiple spaces
        let props = TextInputProps {
            value: "hello   world!",
            placeholder: "",
            is_focused: true,
            style: TextInputStyle::default(),
            on_change: TestAction::Change,
            on_submit: TestAction::Submit,
            on_cursor_move: None,
        };

        let _: Vec<_> = input
            .handle_event(&EventKind::Key(ctrl_arrow(KeyCode::Left)), props)
            .into_iter()
            .collect();

        assert_eq!(input.cursor, 8); // At start of "world"
    }
}
