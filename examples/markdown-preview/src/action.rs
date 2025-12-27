//! Actions for the markdown preview app

#[derive(tui_dispatch::Action, Clone, Debug, PartialEq)]
#[action(infer_categories)]
pub enum Action {
    // ===== Navigation =====
    /// Scroll by N lines (positive = down, negative = up)
    NavScroll(i16),

    /// Scroll by page (positive = down, negative = up)
    NavScrollPage(i16),

    /// Jump to top of document
    NavJumpTop,

    /// Jump to bottom of document
    NavJumpBottom,

    // ===== File =====
    /// Reload the current file
    FileReload,

    // ===== Search =====
    /// Enter search mode
    SearchStart,

    /// Input a character to the search query
    SearchInput(char),

    /// Delete last character from search query
    SearchBackspace,

    /// Submit the search query
    SearchSubmit,

    /// Cancel search mode
    SearchCancel,

    /// Jump to next match
    SearchNext,

    /// Jump to previous match
    SearchPrev,

    // ===== App =====
    /// Exit the application
    Quit,
}
