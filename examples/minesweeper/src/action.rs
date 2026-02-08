use tui_dispatch::prelude::*;

use crate::state::Difficulty;

#[derive(Action, Clone, Debug)]
pub enum Action {
    Reveal(usize, usize),
    ToggleFlag(usize, usize),
    CursorUp,
    CursorDown,
    CursorLeft,
    CursorRight,
    SetDifficulty(Difficulty),
    NewGame,
    Quit,
}
