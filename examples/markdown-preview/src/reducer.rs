//! Reducer for markdown preview app

use crate::action::Action;
use crate::state::AppState;

/// Handle state transitions
pub fn reducer(state: &mut AppState, action: Action) -> bool {
    match action {
        // ===== Navigation =====
        Action::NavScroll(delta) => {
            let old = state.scroll_offset;
            state.scroll(delta);
            state.scroll_offset != old
        }

        Action::NavScrollPage(direction) => {
            let old = state.scroll_offset;
            state.scroll_page(direction);
            state.scroll_offset != old
        }

        Action::NavJumpTop => {
            if state.scroll_offset != 0 {
                state.scroll_offset = 0;
                true
            } else {
                false
            }
        }

        Action::NavJumpBottom => {
            let max = state.max_scroll();
            if state.scroll_offset != max {
                state.scroll_offset = max;
                true
            } else {
                false
            }
        }

        // ===== File =====
        Action::FileReload => {
            state.reload();
            true
        }

        // ===== Search =====
        Action::SearchStart => {
            state.search.active = true;
            state.search.query.clear();
            state.search.matches.clear();
            true
        }

        Action::SearchInput(c) => {
            state.search.query.push(c);
            state.update_search_matches();
            true
        }

        Action::SearchBackspace => {
            state.search.query.pop();
            state.update_search_matches();
            true
        }

        Action::SearchSubmit => {
            state.search.active = false;
            if !state.search.matches.is_empty() {
                state.scroll_to_current_match();
            }
            true
        }

        Action::SearchCancel => {
            state.search.active = false;
            state.search.query.clear();
            state.search.matches.clear();
            true
        }

        Action::SearchNext => {
            state.next_match();
            true
        }

        Action::SearchPrev => {
            state.prev_match();
            true
        }

        // ===== App =====
        Action::Quit => false,
    }
}
