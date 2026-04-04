#[derive(tui_dispatch::Action, Clone, Debug, PartialEq)]
pub enum Action {
    LogsAppended(Vec<String>),
    ToggleLevel(String),
    ToggleTag(String),
    SelectLog(usize),
    OpenSelectedLog,
    CloseDetails,
    FocusNext,
    FocusPrev,
    ToggleFollow,
    Quit,
}
