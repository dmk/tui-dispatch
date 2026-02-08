use tui_dispatch::prelude::*;

use crate::action::Action;
use crate::state::{neighbors, AppState, GameState};

/// Middleware that enforces game rules and drives flood-fill reveals.
///
/// **Cancel** (`before`):
/// - Prevents revealing already-revealed, flagged, or out-of-game cells
/// - Prevents flagging revealed cells or acting after game over
///
/// **Inject** (`after`):
/// - When an empty cell (0 adjacent mines) is revealed, injects `Reveal`
///   for all 8 neighbors. Each goes through the full pipeline — cancelled
///   if already revealed, otherwise reducer marks it, then `after()` may
///   inject more reveals. This creates the classic minesweeper flood fill
///   through recursive middleware dispatch.
pub struct MinesweeperMiddleware;

impl Middleware<AppState, Action> for MinesweeperMiddleware {
    fn before(&mut self, action: &Action, state: &AppState) -> bool {
        match action {
            Action::Reveal(x, y) => {
                state.game_state == GameState::Playing
                    && !state.grid[*y][*x].revealed
                    && !state.grid[*y][*x].flagged
            }
            Action::ToggleFlag(x, y) => {
                state.game_state == GameState::Playing && !state.grid[*y][*x].revealed
            }
            _ => true,
        }
    }

    fn after(&mut self, action: &Action, _state_changed: bool, state: &AppState) -> Vec<Action> {
        match action {
            Action::Reveal(x, y) => {
                if state.game_state != GameState::Playing {
                    return vec![];
                }
                // Flood fill: empty cell reveals all neighbors
                if state.grid[*y][*x].adjacent == 0 {
                    neighbors(*x, *y, state.width, state.height)
                        .into_iter()
                        .map(|(nx, ny)| Action::Reveal(nx, ny))
                        .collect()
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }
}
