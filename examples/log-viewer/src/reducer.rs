use crate::action::Action;
use crate::state::AppState;
use tui_dispatch::ReducerResult;

pub fn reducer(state: &mut AppState, action: Action) -> ReducerResult {
    match action {
        Action::LogsAppended(lines) => state.append_lines(lines),
        Action::ToggleLevel(level) => state.toggle_level(&level),
        Action::ToggleTag(tag) => state.toggle_tag(&tag),
        Action::SelectLog(index) => state.select_visible(index),
        Action::OpenSelectedLog => state.open_selected_details(),
        Action::CloseDetails => state.close_details(),
        Action::FocusNext => state.focus_next(),
        Action::FocusPrev => state.focus_prev(),
        Action::ToggleFollow => state.toggle_follow(),
        Action::Quit => return ReducerResult::unchanged(),
    }

    ReducerResult::changed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Focus, LogLevel};

    fn make_lines(prefix: &str, count: usize) -> Vec<String> {
        (0..count)
            .map(|index| {
                format!(r#"{{"level":"info","target":"{prefix}","message":"line {index}"}}"#)
            })
            .collect()
    }

    #[test]
    fn append_respects_ring_buffer_cap() {
        let mut state = AppState::with_buffer_cap("stdin", 3);

        reducer(&mut state, Action::LogsAppended(make_lines("app", 5)));

        assert_eq!(state.total_count(), 3);
        assert_eq!(state.dropped_lines, 2);
        assert_eq!(state.entries[0].seq, 3);
    }

    #[test]
    fn filters_rebuild_visible_indices() {
        let mut state = AppState::new("stdin");
        reducer(
            &mut state,
            Action::LogsAppended(vec![
                r#"{"level":"info","target":"api","message":"ok"}"#.to_string(),
                r#"{"level":"error","target":"worker","message":"boom"}"#.to_string(),
            ]),
        );

        reducer(
            &mut state,
            Action::ToggleLevel(LogLevel::Info.label().to_string()),
        );

        assert_eq!(state.visible_count(), 1);
        assert_eq!(
            state.selected_entry().map(|entry| entry.level),
            Some(LogLevel::Error)
        );

        reducer(&mut state, Action::ToggleTag("target:worker".to_string()));
        assert_eq!(state.visible_count(), 0);
    }

    #[test]
    fn follow_mode_selects_latest_visible_entry() {
        let mut state = AppState::new("stdin");

        reducer(
            &mut state,
            Action::LogsAppended(vec![
                r#"{"level":"info","target":"api","message":"one"}"#.to_string(),
                r#"{"level":"info","target":"api","message":"two"}"#.to_string(),
            ]),
        );

        assert_eq!(
            state.selected_entry().map(|entry| entry.message.as_str()),
            Some("two")
        );
    }

    #[test]
    fn manual_navigation_disables_follow_until_reenabled() {
        let mut state = AppState::new("stdin");
        reducer(
            &mut state,
            Action::LogsAppended(vec![
                r#"{"level":"info","target":"api","message":"one"}"#.to_string(),
                r#"{"level":"info","target":"api","message":"two"}"#.to_string(),
            ]),
        );

        reducer(&mut state, Action::SelectLog(0));
        assert!(!state.follow_mode);
        assert_eq!(
            state.selected_entry().map(|entry| entry.message.as_str()),
            Some("one")
        );

        reducer(
            &mut state,
            Action::LogsAppended(vec![
                r#"{"level":"info","target":"api","message":"three"}"#.to_string()
            ]),
        );
        assert_eq!(
            state.selected_entry().map(|entry| entry.message.as_str()),
            Some("one")
        );

        reducer(&mut state, Action::ToggleFollow);
        assert!(state.follow_mode);
        assert_eq!(
            state.selected_entry().map(|entry| entry.message.as_str()),
            Some("three")
        );
    }

    #[test]
    fn focus_cycles_in_declared_order() {
        let mut state = AppState::new("stdin");

        reducer(&mut state, Action::FocusPrev);
        assert_eq!(state.focus, Focus::Tags);

        reducer(&mut state, Action::FocusNext);
        assert_eq!(state.focus, Focus::Logs);
    }

    #[test]
    fn details_only_join_focus_cycle_when_open() {
        let mut state = AppState::new("stdin");
        reducer(
            &mut state,
            Action::LogsAppended(vec![
                r#"{"level":"info","target":"api","message":"one"}"#.to_string()
            ]),
        );

        reducer(&mut state, Action::OpenSelectedLog);
        assert!(state.details_open());
        assert_eq!(state.focus, Focus::Details);

        reducer(&mut state, Action::CloseDetails);
        assert!(!state.details_open());
        assert_eq!(state.focus, Focus::Logs);
    }
}
