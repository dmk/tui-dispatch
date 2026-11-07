use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{backend::TestBackend, Terminal};
use serde_json::{json, Value};
use tui_dispatch_core::EventKind;
use tui_dispatch_debug::{
    debug::{DebugInspector, DebugOverlay},
    inspector::state_diff,
    observe::DebugRecord,
    records::{DebugSnapshot, SnapshotOutcome, SnapshotRecord},
};

fn record(sequence: u64, name: &str, count: i32) -> SnapshotRecord {
    DebugRecord::dispatch(
        sequence,
        name,
        json!({"amount":count}),
        json!({"count":count}),
        true,
    )
    .into()
}
fn snapshot(history: Vec<SnapshotRecord>) -> DebugSnapshot {
    DebugSnapshot {
        state: history
            .last()
            .map(|r| r.state.clone())
            .unwrap_or(Value::Null),
        history,
        components: None,
        control: None,
    }
}
fn key(view: &mut DebugInspector, code: KeyCode) {
    assert!(!view.handle_event(&EventKind::Key(KeyEvent::new(code, KeyModifiers::NONE))));
}
fn render(view: &mut DebugInspector, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| view.render(f, "test")).unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect()
}

#[test]
fn selected_dispatch_has_payload_boundary_state_diff_and_rendered_details() {
    let mut view = DebugInspector::new()
        .fullscreen()
        .with_initial_state(json!({"count":0}));
    view.update_snapshot(snapshot(vec![record(1, "Add", 2), record(2, "Add", 5)]));
    view.set_search("aDd");
    assert_eq!(view.visible_sequences(), [2, 1]);
    assert!(view.select_sequence(2));
    assert_eq!(view.selected_payload(), Some(&json!({"amount":5})));
    assert_eq!(view.selected_state(), Some(&json!({"count":5})));
    let diff = view.selected_diff().unwrap();
    assert_eq!(diff.len(), 1);
    assert_eq!(diff[0].path, "/count");
    assert_eq!(diff[0].before, Some(json!(2)));
    assert_eq!(diff[0].after, Some(json!(5)));
    key(&mut view, KeyCode::Enter);
    let text = render(&mut view, 120, 40);
    for expected in [
        "Add", "payload", "amount", "state", "count", "diff", "/count", "before", "after",
    ] {
        assert!(text.contains(expected), "missing {expected}: {text}");
    }
    // Detail navigation and clipping reuse the existing scrolling renderer.
    key(&mut view, KeyCode::PageDown);
    render(&mut view, 12, 3);
    render(&mut view, 0, 0);
    key(&mut view, KeyCode::Esc);
    assert!(view.select_sequence(1));
    assert_eq!(view.selected_state(), Some(&json!({"count":2})));
    assert_eq!(view.selected_diff().unwrap()[0].before, Some(json!(0)));
    view.set_search("no-such-action");
    assert!(view.visible_sequences().is_empty());
    assert!(view.selected_record().is_none());
    assert!(view.selected_diff().is_none());
    assert!(!view.select_sequence(1));
    key(&mut view, KeyCode::Enter);
    assert!(matches!(view.overlay(), Some(DebugOverlay::ActionLog(_))));
    view.set_search("\"amount\": 2");
    assert_eq!(view.visible_sequences(), [1]);
}

#[test]
fn hidden_predecessors_are_used_but_gaps_and_evicted_baselines_are_unavailable() {
    let mut view = DebugInspector::new().with_initial_state(json!({"count":0}));
    view.update_snapshot(snapshot(vec![
        record(1, "Hidden", 2),
        record(2, "Visible", 5),
    ]));
    view.set_search("Visible");
    assert_eq!(view.visible_sequences(), [2]);
    assert_eq!(view.selected_diff().unwrap()[0].before, Some(json!(2)));
    assert!(!view.select_sequence(1));
    view.update_snapshot(snapshot(vec![record(2, "Visible", 5)]));
    assert!(
        view.selected_diff().is_none(),
        "initial state is not sequence 2's baseline"
    );
    view.update_snapshot(snapshot(vec![
        record(2, "Hidden", 2),
        record(4, "Visible", 5),
    ]));
    assert!(
        view.selected_diff().is_none(),
        "cannot diff across a sequence gap"
    );
    key(&mut view, KeyCode::Enter);
    let text = render(&mut view, 120, 40);
    assert!(text.contains("unavailable"));
    assert!(text.contains("state"));
}

#[test]
fn open_details_pin_the_selected_record_and_baseline_through_updates() {
    let mut view = DebugInspector::new();
    view.update_snapshot(snapshot(vec![record(1, "Add", 2), record(2, "Add", 5)]));
    assert!(view.select_sequence(2));
    key(&mut view, KeyCode::Enter);
    view.update_snapshot(snapshot(vec![record(3, "Add", 9)]));
    assert_eq!(view.selected_record().unwrap().sequence, 2);
    assert_eq!(view.selected_state(), Some(&json!({"count":5})));
    assert_eq!(view.selected_diff().unwrap()[0].before, Some(json!(2)));
    assert_eq!(view.visible_sequences(), [3]);
    key(&mut view, KeyCode::Esc);
    assert_eq!(view.selected_record().unwrap().sequence, 3);
    assert!(view.selected_diff().is_none());
    key(&mut view, KeyCode::Enter);
    assert_eq!(view.selected_state(), Some(&json!({"count":9})));
    key(&mut view, KeyCode::Char('s'));
    assert!(render(&mut view, 120, 40).contains("Application State"));
    view.update_snapshot(snapshot(Vec::new()));
    key(&mut view, KeyCode::Char('a'));
    assert!(view.selected_record().is_none());
}

#[test]
fn cancelled_rejected_and_unchanged_dispatches_have_real_boundary_states() {
    let mut view = DebugInspector::new().with_initial_state(json!({"count":2}));
    let mut cancelled = record(1, "Cancelled", 2);
    cancelled.changed = false;
    cancelled.outcome = SnapshotOutcome::Cancelled;
    let mut rejected = record(2, "Rejected", 2);
    rejected.changed = false;
    rejected.outcome = SnapshotOutcome::DepthExceeded {
        max_depth: 1,
        action: "Rejected".into(),
    };
    let mut changed = record(3, "Mutated", 3);
    changed.changed = false; // Diff uses snapshots, not an app-supplied redraw flag.
    view.update_snapshot(snapshot(vec![cancelled, rejected, changed]));
    for sequence in [1, 2] {
        assert!(view.select_sequence(sequence));
        assert_eq!(view.selected_state(), Some(&json!({"count":2})));
        assert_eq!(view.selected_diff(), Some(vec![]));
    }
    assert!(view.select_sequence(3));
    assert_eq!(view.selected_diff().unwrap().len(), 1);
    view.set_search("rejected");
    assert_eq!(view.visible_sequences(), [2]);
}

#[test]
fn failed_serialization_is_distinct_from_captured_json_null() {
    let mut view = DebugInspector::new().with_initial_state(json!(null));
    let mut null = record(1, "Null", 0);
    null.state = Value::Null;
    null.action = Value::Null;
    view.update_snapshot(snapshot(vec![null.clone()]));
    assert_eq!(view.selected_state(), Some(&Value::Null));
    assert_eq!(view.selected_payload(), Some(&Value::Null));
    assert_eq!(view.selected_diff(), Some(vec![]));
    null.serialization_errors.push("state: failed".into());
    null.serialization_errors.push("action: failed".into());
    view.update_snapshot(snapshot(vec![null.clone(), record(2, "Add", 5)]));
    assert!(
        view.selected_diff().is_none(),
        "predecessor serialization failed"
    );
    assert!(view.select_sequence(1));
    assert!(view.selected_state().is_none());
    assert!(view.selected_payload().is_none());
    assert!(view.selected_diff().is_none());
    key(&mut view, KeyCode::Enter);
    assert!(render(&mut view, 120, 40).contains("unavailable"));
}

#[test]
fn json_diff_covers_root_containers_arrays_escaped_keys_and_missing_vs_null() {
    let before = json!({"a/b":{"~key":1},"gone":null,"array":[1,2,3],"type":{},"same":true});
    let after = json!({"a/b":{"~key":2},"new":null,"array":[1,4],"type":[],"same":true});
    let changes = state_diff(&before, &after);
    assert_eq!(
        changes.iter().map(|d| d.path.as_str()).collect::<Vec<_>>(),
        [
            "/array/1",
            "/array/2",
            "/a~1b/~0key",
            "/gone",
            "/new",
            "/type"
        ]
    );
    assert_eq!(changes[1].before, Some(json!(3)));
    assert_eq!(changes[1].after, None);
    assert_eq!(changes[3].before, Some(Value::Null));
    assert_eq!(changes[3].after, None);
    assert_eq!(changes[4].before, None);
    assert_eq!(changes[4].after, Some(Value::Null));
    let serialized = serde_json::to_value(&changes).unwrap();
    assert!(serialized[3].get("after").is_none());
    assert_eq!(serialized[3]["before"], Value::Null);
    assert_eq!(serialized[4]["after"], Value::Null);
    for (a, b) in [
        (json!(1), json!(null)),
        (json!({}), json!([])),
        (json!([]), json!(true)),
    ] {
        let change = state_diff(&a, &b);
        assert_eq!(change.len(), 1);
        assert_eq!(change[0].path, "");
        assert_eq!(change[0].before.as_ref(), Some(&a));
        assert_eq!(change[0].after.as_ref(), Some(&b));
    }
    assert_eq!(state_diff(&json!([]), &json!([null]))[0].before, None);
    assert_eq!(state_diff(&json!({"":0}), &json!({"":1}))[0].path, "/");
    let state = json!({"text":"x".repeat(65536),"nested":[{"count":1}]});
    assert!(state_diff(&state, &state).is_empty());
}

#[test]
fn programmatic_selection_aligns_filtered_navigation_and_refresh_replaces_history() {
    let mut view = DebugInspector::new();
    view.update_snapshot(snapshot(vec![
        record(1, "Load", 1),
        record(2, "Load", 2),
        record(3, "Other", 3),
    ]));
    key(&mut view, KeyCode::Char('/'));
    view.set_search("Load"); // Finishes any keyboard query editing.
    assert!(view.select_sequence(1));
    key(&mut view, KeyCode::Down);
    assert_eq!(view.selected_record().unwrap().sequence, 1);
    key(&mut view, KeyCode::Up);
    assert_eq!(view.selected_record().unwrap().sequence, 2);
    key(&mut view, KeyCode::Enter);
    assert!(matches!(
        view.overlay(),
        Some(DebugOverlay::ActionDetail(_))
    ));
    for n in 4..132 {
        view.update_snapshot(snapshot(vec![
            record(n - 1, "Load", 0),
            record(n, "Load", 1),
        ]));
        assert_eq!(view.visible_sequences(), [n, n - 1]);
        assert_eq!(view.selected_record().unwrap().sequence, 2);
    }
    // Returning releases the old detail and selects a record in current history.
    key(&mut view, KeyCode::Char('a'));
    assert!(view.selected_record().unwrap().sequence >= 130);
    key(&mut view, KeyCode::Enter);
    assert!(view.selected_record().unwrap().sequence >= 130);
}

#[test]
fn shared_snapshots_are_not_copied_or_pinned_as_whole_histories() {
    use std::sync::Arc;
    let data = Arc::new(snapshot(vec![record(1, "Add", 1), record(2, "Add", 2)]));
    let mut view = DebugInspector::new();
    view.update_snapshot(Arc::clone(&data));
    assert_eq!(Arc::strong_count(&data), 2);
    assert!(std::ptr::eq(
        view.selected_record().unwrap(),
        &data.history[1]
    ));
    key(&mut view, KeyCode::Enter);
    view.update_snapshot(snapshot(vec![record(3, "Add", 3)]));
    assert_eq!(
        Arc::strong_count(&data),
        1,
        "an open detail must not retain evicted history"
    );
    assert_eq!(view.selected_record().unwrap().sequence, 2);
    assert_eq!(view.selected_diff().unwrap()[0].before, Some(json!(1)));
}
