use std::collections::{HashSet, VecDeque};

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
/// - When an empty cell (0 adjacent mines) is revealed, computes flood fill
///   once and injects a flat list of `Reveal` actions. This keeps middleware
///   behavior deterministic on wasm and avoids deep recursive injection chains.
#[derive(Default)]
pub struct MinesweeperMiddleware {
    // Number of injected reveal actions that belong to the current flood-fill cascade.
    pending_flood_reveals: usize,
    // True when processing one of those injected reveal actions.
    in_injected_reveal: bool,
}

impl Middleware<AppState, Action> for MinesweeperMiddleware {
    fn before(&mut self, action: &Action, state: &AppState) -> bool {
        self.in_injected_reveal = false;
        if matches!(action, Action::Reveal(_, _)) && self.pending_flood_reveals > 0 {
            self.pending_flood_reveals -= 1;
            self.in_injected_reveal = true;
        }

        match action {
            Action::Reveal(x, y) => {
                state
                    .grid
                    .get(*y)
                    .and_then(|row| row.get(*x))
                    .is_some_and(|cell| {
                        state.game_state == GameState::Playing && !cell.revealed && !cell.flagged
                    })
            }
            Action::ToggleFlag(x, y) => state
                .grid
                .get(*y)
                .and_then(|row| row.get(*x))
                .is_some_and(|cell| state.game_state == GameState::Playing && !cell.revealed),
            _ => true,
        }
    }

    fn after(&mut self, action: &Action, _state_changed: bool, state: &AppState) -> Vec<Action> {
        if self.in_injected_reveal {
            self.in_injected_reveal = false;
            return vec![];
        }

        match action {
            Action::Reveal(x, y) => {
                let Some(cell) = state.grid.get(*y).and_then(|row| row.get(*x)) else {
                    return vec![];
                };

                if state.game_state != GameState::Playing || cell.mine || cell.adjacent != 0 {
                    return vec![];
                }

                // Compute flood-fill once and inject a flat reveal list.
                let mut queue = VecDeque::new();
                queue.push_back((*x, *y));
                let mut visited = HashSet::new();
                visited.insert((*x, *y));
                let mut revealed = Vec::new();

                while let Some((cx, cy)) = queue.pop_front() {
                    for (nx, ny) in neighbors(cx, cy, state.width, state.height) {
                        if !visited.insert((nx, ny)) {
                            continue;
                        }
                        let neighbor = &state.grid[ny][nx];
                        if neighbor.revealed || neighbor.flagged || neighbor.mine {
                            continue;
                        }
                        revealed.push(Action::Reveal(nx, ny));
                        if neighbor.adjacent == 0 {
                            queue.push_back((nx, ny));
                        }
                    }
                }

                self.pending_flood_reveals = revealed.len();
                revealed
            }
            _ => vec![],
        }
    }
}
