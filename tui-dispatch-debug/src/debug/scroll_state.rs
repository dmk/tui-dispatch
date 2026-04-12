//! Reusable scroll and cursor state for debug overlays.

use crossterm::event::KeyCode;

/// Scroll offset tracker for content rendered via `ScrollView`.
///
/// Tracks `offset` (current scroll position) and a cached `page_size`
/// (visible rows). All navigation methods clamp automatically.
#[derive(Debug, Clone)]
pub struct ScrollState {
    /// Current scroll offset (first visible row)
    pub offset: usize,
    /// Cached page size (visible rows in the viewport)
    pub page_size: usize,
}

impl Default for ScrollState {
    fn default() -> Self {
        Self {
            offset: 0,
            page_size: 1,
        }
    }
}

impl ScrollState {
    /// Create a new scroll state at position 0.
    pub fn new() -> Self {
        Self::default()
    }

    fn page_size_value(&self) -> usize {
        self.page_size.max(1)
    }

    fn max_offset(&self, content_len: usize) -> usize {
        content_len.saturating_sub(self.page_size_value())
    }

    /// Scroll up by one row.
    pub fn scroll_up(&mut self) {
        self.offset = self.offset.saturating_sub(1);
    }

    /// Scroll down by one row, clamped to `content_len`.
    pub fn scroll_down(&mut self, content_len: usize) {
        let max = self.max_offset(content_len);
        self.offset = (self.offset + 1).min(max);
    }

    /// Jump to the top.
    pub fn to_top(&mut self) {
        self.offset = 0;
    }

    /// Jump to the bottom, clamped to `content_len`.
    pub fn to_bottom(&mut self, content_len: usize) {
        self.offset = self.max_offset(content_len);
    }

    /// Scroll up by one page.
    pub fn page_up(&mut self) {
        let ps = self.page_size_value();
        self.offset = self.offset.saturating_sub(ps);
    }

    /// Scroll down by one page, clamped to `content_len`.
    pub fn page_down(&mut self, content_len: usize) {
        let ps = self.page_size_value();
        let max = self.max_offset(content_len);
        self.offset = (self.offset + ps).min(max);
    }

    /// Handle standard scroll keys (j/k, arrows, g/G, Home/End, PageUp/PageDown).
    ///
    /// Returns `true` if the key was consumed.
    pub fn handle_scroll_key(&mut self, key: KeyCode, content_len: usize) -> bool {
        match key {
            KeyCode::Char('j') | KeyCode::Down => {
                self.scroll_down(content_len);
                true
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.scroll_up();
                true
            }
            KeyCode::Char('g') | KeyCode::Home => {
                self.to_top();
                true
            }
            KeyCode::Char('G') | KeyCode::End => {
                self.to_bottom(content_len);
                true
            }
            KeyCode::PageDown => {
                self.page_down(content_len);
                true
            }
            KeyCode::PageUp => {
                self.page_up();
                true
            }
            _ => false,
        }
    }

    /// Reset to initial state.
    pub fn reset(&mut self) {
        self.offset = 0;
        self.page_size = 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_state_basic_navigation() {
        let mut s = ScrollState::new();
        s.page_size = 5;

        s.scroll_down(20);
        assert_eq!(s.offset, 1);

        s.scroll_up();
        assert_eq!(s.offset, 0);

        // Can't go below 0
        s.scroll_up();
        assert_eq!(s.offset, 0);

        s.to_bottom(20);
        assert_eq!(s.offset, 15); // 20 - 5

        s.to_top();
        assert_eq!(s.offset, 0);
    }

    #[test]
    fn scroll_state_page_navigation() {
        let mut s = ScrollState::new();
        s.page_size = 5;

        s.page_down(20);
        assert_eq!(s.offset, 5);

        s.page_down(20);
        assert_eq!(s.offset, 10);

        s.page_up();
        assert_eq!(s.offset, 5);

        // Clamp at bottom
        s.offset = 14;
        s.page_down(20);
        assert_eq!(s.offset, 15);
    }

    #[test]
    fn scroll_state_handle_key() {
        let mut s = ScrollState::new();
        s.page_size = 5;

        assert!(s.handle_scroll_key(KeyCode::Char('j'), 10));
        assert_eq!(s.offset, 1);

        assert!(s.handle_scroll_key(KeyCode::Char('k'), 10));
        assert_eq!(s.offset, 0);

        assert!(s.handle_scroll_key(KeyCode::Char('G'), 10));
        assert_eq!(s.offset, 5);

        assert!(s.handle_scroll_key(KeyCode::Char('g'), 10));
        assert_eq!(s.offset, 0);

        assert!(!s.handle_scroll_key(KeyCode::Char('x'), 10));
    }

    #[test]
    fn scroll_state_reset() {
        let mut s = ScrollState {
            offset: 5,
            page_size: 10,
        };
        s.reset();
        assert_eq!(s.offset, 0);
        assert_eq!(s.page_size, 1);
    }
}
