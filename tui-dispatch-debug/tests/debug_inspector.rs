use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{backend::TestBackend, buffer::Buffer, widgets::Paragraph, Terminal};
use serde_json::json;
use tui_dispatch_core::EventKind;
use tui_dispatch_debug::control::{ComponentTarget, ControlStatus, DebugCommand};
use tui_dispatch_debug::debug::{
    ActionLogDisplayEntry, ActionLogOverlay, ComponentSnapshot, DebugInspector, DebugLayer,
    DebugOverlay, DebugStyle,
};

fn key(inspector: &mut DebugInspector, code: KeyCode) -> bool {
    inspector.handle_event(&EventKind::Key(KeyEvent::new(code, KeyModifiers::NONE)))
}
fn entries(sequences: &[u64]) -> Vec<ActionLogDisplayEntry> {
    sequences
        .iter()
        .map(|sequence| ActionLogDisplayEntry {
            sequence: *sequence,
            name: if *sequence <= 2 { "Load" } else { "Done" }.into(),
            params: format!("[applied] {{\"amount\":{sequence}}}"),
            params_detail: format!("{{\"amount\":{sequence}}}"),
            elapsed: "—".into(),
        })
        .collect()
}
fn log(inspector: &DebugInspector) -> &ActionLogOverlay {
    match inspector.overlay() {
        Some(DebugOverlay::ActionLog(log)) => log,
        other => panic!("expected action list, got {other:?}"),
    }
}
fn render(inspector: &mut DebugInspector, width: u16, height: u16) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| inspector.render(frame, "Connected · read only"))
        .unwrap();
    terminal.backend().buffer().clone()
}
fn text(buffer: &Buffer) -> String {
    buffer
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>()
}

#[test]
fn renders_shared_action_and_state_views_with_the_live_palette() {
    let mut inspector = DebugInspector::new().fullscreen();
    inspector.update(json!({"count":42,"label":"hello"}), entries(&[2, 1]));
    let buffer = render(&mut inspector, 110, 28);
    let rendered = text(&buffer);
    let first_row: String = (0..110).map(|x| buffer[(x, 0)].symbol()).collect();
    assert!(
        first_row.contains("Action Log"),
        "view must start at the top edge"
    );
    assert_eq!(buffer[(0, 0)].bg, DebugStyle::default().overlay_bg);
    assert_eq!(buffer[(109, 25)].bg, DebugStyle::default().overlay_bg);
    for expected in [
        "Action Log",
        "Load",
        "amount",
        "DEBUG",
        "Connected",
        "read only",
    ] {
        assert!(rendered.contains(expected), "missing {expected}");
    }
    for unavailable in ["resume", "mouse"] {
        assert!(!rendered.contains(unavailable));
    }
    assert!(buffer
        .content
        .iter()
        .any(|cell| cell.bg == DebugStyle::default().overlay_bg));
    key(&mut inspector, KeyCode::Char('s'));
    let state_buffer = render(&mut inspector, 110, 28);
    let first_row: String = (0..110).map(|x| state_buffer[(x, 0)].symbol()).collect();
    assert!(first_row.contains("Application State"));
    let rendered = text(&state_buffer);
    for expected in ["Application State", "count", "42", "label", "hello"] {
        assert!(rendered.contains(expected), "missing {expected}");
    }
    // Exercise shared view clipping on a small terminal.
    render(&mut inspector, 12, 3);
}

#[test]
fn embedded_inspector_keeps_modals_and_the_application_behind_them() {
    let mut inspector = DebugInspector::default();
    inspector.update(json!({"count":42}), entries(&[1]));
    let mut terminal = Terminal::new(TestBackend::new(110, 28)).unwrap();
    for title in ["Action Log", "Application State"] {
        terminal
            .draw(|frame| {
                frame.render_widget(Paragraph::new("application behind debugger"), frame.area());
                inspector.render(frame, "Embedded");
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let first_row: String = (0..110).map(|x| buffer[(x, 0)].symbol()).collect();
        assert!(first_row.contains("application behind debugger"));
        assert!(!first_row.contains(title));
        assert!(text(buffer).contains(title));
        key(&mut inspector, KeyCode::Char('s'));
    }
}

#[test]
fn interactive_debug_layer_keeps_action_and_state_overlays_modal() {
    #[derive(Clone, Debug)]
    struct AppAction;
    impl tui_dispatch_core::Action for AppAction {
        fn name(&self) -> &'static str {
            "AppAction"
        }
    }
    let mut layer = DebugLayer::<AppAction>::simple();
    layer.handle_event(&EventKind::Key(KeyEvent::new(
        KeyCode::F(12),
        KeyModifiers::NONE,
    )));
    assert!(layer.is_enabled());
    layer.show_action_log();
    let mut terminal = Terminal::new(TestBackend::new(110, 28)).unwrap();
    for title in ["Action Log", "Application State"] {
        terminal
            .draw(|frame| {
                layer.render_state(frame, &(), |frame, area| {
                    frame.render_widget(Paragraph::new("application behind debugger"), area);
                });
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let first_row: String = (0..110).map(|x| buffer[(x, 0)].symbol()).collect();
        assert!(first_row.contains("application behind debugger"));
        assert!(!first_row.contains(title));
        assert!(text(buffer).contains(title));
        layer.handle_event_with_state(
            &EventKind::Key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE)),
            &(),
        );
    }
}

#[test]
fn incoming_history_preserves_search_selection_and_open_details() {
    let mut inspector = DebugInspector::new();
    inspector.update(json!(0), entries(&[3, 2, 1]));
    key(&mut inspector, KeyCode::Char('/'));
    for ch in "Load".chars() {
        assert!(!key(&mut inspector, KeyCode::Char(ch)));
    }
    // A poll while editing must leave the query active.
    inspector.update(json!(1), entries(&[4, 3, 2, 1]));
    assert_eq!(log(&inspector).search_query, "Load");
    assert!(log(&inspector).search_input_active);
    key(&mut inspector, KeyCode::Enter);
    key(&mut inspector, KeyCode::Down);
    assert_eq!(log(&inspector).get_selected().unwrap().sequence, 1);
    inspector.update(json!(2), entries(&[5, 4, 3, 2, 1]));
    assert_eq!(log(&inspector).get_selected().unwrap().sequence, 1);
    assert_eq!(log(&inspector).search_match_count(), 2);
    key(&mut inspector, KeyCode::Enter);
    inspector.update(json!(3), entries(&[5, 4, 3, 2]));
    match inspector.overlay() {
        Some(DebugOverlay::ActionDetail(detail)) => {
            assert_eq!(detail.sequence, 1);
            assert!(detail.params.contains("1"));
        }
        other => panic!("poll replaced the selected detail: {other:?}"),
    }
    assert!(!key(&mut inspector, KeyCode::Esc));
    assert_eq!(log(&inspector).search_query, "Load");
    assert_eq!(log(&inspector).get_selected().unwrap().sequence, 2);
}

#[test]
fn state_search_and_details_survive_refresh_and_return_to_current_state() {
    let mut inspector = DebugInspector::new();
    inspector.update(json!({"count":1,"other":9}), entries(&[1]));
    key(&mut inspector, KeyCode::Char('s'));
    render(&mut inspector, 100, 24);
    key(&mut inspector, KeyCode::Char('/'));
    for ch in "count".chars() {
        key(&mut inspector, KeyCode::Char(ch));
    }
    inspector.update(json!({"count":2,"other":9}), entries(&[2, 1]));
    assert!(text(&render(&mut inspector, 100, 24)).contains("count"));
    key(&mut inspector, KeyCode::Enter); // finish search
    key(&mut inspector, KeyCode::Down); // select the field below its section
    key(&mut inspector, KeyCode::Enter);
    match inspector.overlay() {
        Some(DebugOverlay::StateDetail(detail)) => assert_eq!(detail.value, "2"),
        other => panic!("expected state field detail: {other:?}"),
    }
    inspector.update(json!({"count":3,"other":9}), entries(&[3, 2, 1]));
    match inspector.overlay() {
        Some(DebugOverlay::StateDetail(detail)) => assert_eq!(detail.value, "2"),
        other => panic!("poll replaced the state detail: {other:?}"),
    }
    assert!(!key(&mut inspector, KeyCode::Esc));
    let rendered = text(&render(&mut inspector, 100, 24));
    assert!(rendered.contains("count"));
    assert!(!rendered.contains("other"), "state filter was lost");
    key(&mut inspector, KeyCode::Enter);
    match inspector.overlay() {
        Some(DebugOverlay::StateDetail(detail)) => assert_eq!(detail.value, "3"),
        other => panic!("expected refreshed state detail: {other:?}"),
    }
}

#[test]
fn search_accepts_shortcut_letters_and_remote_view_cannot_freeze_the_app() {
    let mut inspector = DebugInspector::new();
    inspector.update(json!(0), entries(&[1]));
    key(&mut inspector, KeyCode::Char('/'));
    for ch in "qasciy".chars() {
        assert!(!key(&mut inspector, KeyCode::Char(ch)));
    }
    assert_eq!(log(&inspector).search_query, "qasciy");
    assert!(!key(&mut inspector, KeyCode::Esc)); // finish editing
    for code in [KeyCode::F(12), KeyCode::Char('i'), KeyCode::Char('y')] {
        assert!(!key(&mut inspector, code));
        assert!(matches!(
            inspector.overlay(),
            Some(DebugOverlay::ActionLog(_))
        ));
    }
    let release = KeyEvent::new_with_kind(
        KeyCode::Char('q'),
        KeyModifiers::NONE,
        KeyEventKind::Release,
    );
    assert!(!inspector.handle_event(&EventKind::Key(release)));
    assert!(key(&mut inspector, KeyCode::Char('q')));
}

fn component(raw_id: u32, name: &str, hp: &str) -> ComponentSnapshot {
    ComponentSnapshot {
        raw_id,
        type_name: name.into(),
        type_name_full: format!("game::{name}"),
        bound_id: Some(format!("component-{raw_id}")),
        last_area: Some(ratatui::layout::Rect::new(3, 4, 10, 5)),
        debug_entries: vec![("hp".into(), hp.into())],
    }
}

#[test]
fn component_search_selection_and_details_survive_new_frames() {
    let mut inspector = DebugInspector::new().fullscreen();
    inspector.update_components(Some(vec![
        component(1, "Enemy", "3"),
        component(2, "Hud", "0"),
    ]));
    assert!(!key(&mut inspector, KeyCode::Char('c')));
    let screen = text(&render(&mut inspector, 110, 28));
    assert!(screen.contains("Components"));
    assert!(screen.contains("Enemy"));
    assert!(screen.contains("Hud"));
    key(&mut inspector, KeyCode::Char('/'));
    for ch in "Enemy".chars() {
        assert!(!key(&mut inspector, KeyCode::Char(ch)));
    }
    inspector.update_components(Some(vec![
        component(2, "Hud", "0"),
        component(1, "Enemy", "2"),
    ]));
    key(&mut inspector, KeyCode::Enter); // finish query
    let screen = text(&render(&mut inspector, 110, 28));
    assert!(screen.contains("Enemy"));
    assert!(!screen.contains("Hud"));
    key(&mut inspector, KeyCode::Enter);
    match inspector.overlay() {
        Some(DebugOverlay::ComponentDetail(detail)) => {
            assert_eq!(detail.bound_id.as_deref(), Some("component-1"));
            assert_eq!(detail.debug_entries, [("hp".into(), "2".into())]);
        }
        other => panic!("expected component detail: {other:?}"),
    }
    inspector.update_components(Some(vec![
        component(1, "Enemy", "1"),
        component(2, "Hud", "0"),
    ]));
    match inspector.overlay() {
        Some(DebugOverlay::ComponentDetail(detail)) => assert_eq!(detail.debug_entries[0].1, "2"),
        other => panic!("new frame interrupted inspection: {other:?}"),
    }
    assert!(!key(&mut inspector, KeyCode::Esc));
    let screen = text(&render(&mut inspector, 110, 28));
    assert!(!screen.contains("Hud"), "component filter was lost");
    key(&mut inspector, KeyCode::Enter);
    match inspector.overlay() {
        Some(DebugOverlay::ComponentDetail(detail)) => assert_eq!(detail.debug_entries[0].1, "1"),
        other => panic!("expected updated component detail: {other:?}"),
    }
}

#[test]
fn components_tab_distinguishes_unavailable_from_a_published_empty_frame() {
    let mut inspector = DebugInspector::new().fullscreen();
    key(&mut inspector, KeyCode::Char('c'));
    assert!(text(&render(&mut inspector, 110, 28)).contains("not published by app"));
    inspector.update_components(Some(Vec::new()));
    let screen = text(&render(&mut inspector, 110, 28));
    assert!(screen.contains("Components"));
    assert!(!screen.contains("not published by app"));
    inspector.update_components(None);
    assert!(text(&render(&mut inspector, 110, 28)).contains("not published by app"));
    key(&mut inspector, KeyCode::Char('/'));
    for ch in "cqs".chars() {
        assert!(!key(&mut inspector, KeyCode::Char(ch)));
    }
    assert!(matches!(
        inspector.overlay(),
        Some(DebugOverlay::Components(_))
    ));
}

#[test]
fn remote_pause_and_selection_emit_commands_without_viewer_highlights() {
    let mut inspector = DebugInspector::new();
    inspector.update_control_status(Some(ControlStatus {
        paused: false,
        pause_requested: false,
        highlight: None,
    }));
    let mut enemy = component(7, "Enemy", "3");
    enemy.last_area = Some(ratatui::layout::Rect::new(0, 0, 110, 27));
    inspector.update_components(Some(vec![enemy]));
    key(&mut inspector, KeyCode::Char('c'));
    assert_eq!(
        inspector.take_commands(),
        vec![DebugCommand::Highlight(Some(ComponentTarget {
            raw_id: 0,
            bound_id: Some("component-7".into()),
        }))]
    );
    let mut terminal = Terminal::new(TestBackend::new(110, 28)).unwrap();
    terminal
        .draw(|frame| {
            frame.render_widget(Paragraph::new("debugger background"), frame.area());
            inspector.render(frame, "Connected");
        })
        .unwrap();
    let first_row: String = (0..110)
        .map(|x| terminal.backend().buffer()[(x, 0)].symbol())
        .collect();
    assert!(
        first_row.starts_with("debugger background"),
        "app bounds must not be painted in the debugger"
    );
    key(&mut inspector, KeyCode::Char('P'));
    assert_eq!(inspector.take_commands(), vec![DebugCommand::TogglePause]);
    let repeat =
        KeyEvent::new_with_kind(KeyCode::Char('P'), KeyModifiers::NONE, KeyEventKind::Repeat);
    inspector.handle_event(&EventKind::Key(repeat));
    assert!(inspector.take_commands().is_empty());
    key(&mut inspector, KeyCode::Char('/'));
    key(&mut inspector, KeyCode::Char('p'));
    assert!(
        !inspector
            .take_commands()
            .contains(&DebugCommand::TogglePause),
        "P is text while searching"
    );
    key(&mut inspector, KeyCode::Enter);
    key(&mut inspector, KeyCode::Char('s'));
    assert_eq!(
        inspector.take_commands(),
        vec![DebugCommand::Highlight(None)]
    );
    inspector.update_control_status(None);
    key(&mut inspector, KeyCode::Char('P'));
    assert!(inspector.take_commands().is_empty());
}

#[test]
fn newest_follow_and_empty_history_are_safe_across_reconnects() {
    let mut inspector = DebugInspector::new();
    inspector.update(json!(1), entries(&[1]));
    inspector.update(json!(2), entries(&[2, 1]));
    assert_eq!(log(&inspector).get_selected().unwrap().sequence, 2);
    inspector.update(json!(null), Vec::new());
    assert!(log(&inspector).entries.is_empty());
    assert_eq!(log(&inspector).selected, 0);
    render(&mut inspector, 80, 24);
    inspector.update(json!(1), entries(&[1]));
    assert_eq!(log(&inspector).get_selected().unwrap().sequence, 1);
}

#[test]
fn a_filter_with_no_matches_does_not_open_an_unrelated_action() {
    let mut inspector = DebugInspector::new();
    inspector.update(json!(1), entries(&[1]));
    key(&mut inspector, KeyCode::Char('/'));
    key(&mut inspector, KeyCode::Char('z'));
    key(&mut inspector, KeyCode::Enter);
    key(&mut inspector, KeyCode::Enter);
    assert!(log(&inspector).get_selected().is_none());
    inspector.update(json!(2), entries(&[2, 1]));
    key(&mut inspector, KeyCode::Enter);
    assert!(log(&inspector).get_selected().is_none());
}
