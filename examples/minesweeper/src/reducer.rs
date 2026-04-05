use crate::action::Action;
use crate::state::{AppState, GameState};

/// Reducer trusts middleware already validated guards (game over, revealed, flagged).
pub fn reducer(state: &mut AppState, action: Action) -> bool {
    match action {
        Action::Reveal(x, y) => {
            let Some(cell) = state.grid.get_mut(y).and_then(|row| row.get_mut(x)) else {
                return false;
            };
            cell.revealed = true;

            if cell.mine {
                state.game_state = GameState::Lost;
            } else {
                state.cells_revealed += 1;
                if state.cells_revealed == state.total_safe {
                    state.game_state = GameState::Won;
                }
            }
            true
        }
        Action::ToggleFlag(x, y) => {
            let Some(cell) = state.grid.get_mut(y).and_then(|row| row.get_mut(x)) else {
                return false;
            };
            if cell.flagged {
                cell.flagged = false;
                state.flags_placed = state.flags_placed.saturating_sub(1);
            } else {
                cell.flagged = true;
                state.flags_placed += 1;
            }
            true
        }
        Action::CursorUp => {
            if state.cursor_y > 0 {
                state.cursor_y -= 1;
                true
            } else {
                false
            }
        }
        Action::CursorDown => {
            if state.cursor_y < state.height - 1 {
                state.cursor_y += 1;
                true
            } else {
                false
            }
        }
        Action::CursorLeft => {
            if state.cursor_x > 0 {
                state.cursor_x -= 1;
                true
            } else {
                false
            }
        }
        Action::CursorRight => {
            if state.cursor_x < state.width - 1 {
                state.cursor_x += 1;
                true
            } else {
                false
            }
        }
        Action::SetDifficulty(difficulty) => {
            *state = AppState::new(difficulty);
            true
        }
        Action::NewGame => {
            *state = AppState::new(state.difficulty);
            true
        }
        Action::Quit => false,
    }
}
