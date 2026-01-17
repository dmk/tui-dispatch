//! Tests for #[derive(DebugState)] macro

#![allow(dead_code)]

use serde::Serialize;
use tui_dispatch::debug::DebugState;
use tui_dispatch::DebugState;

#[test]
fn test_basic_derive() {
    #[derive(DebugState, Serialize)]
    struct SimpleState {
        name: String,
        count: usize,
    }

    let state = SimpleState {
        name: "test".to_string(),
        count: 42,
    };

    let sections = state.debug_sections();
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].title, "SimpleState");
    assert_eq!(sections[0].entries.len(), 2);
    assert_eq!(sections[0].entries[0].key, "name");
    assert_eq!(sections[0].entries[0].value, "\"test\"");
    assert_eq!(sections[0].entries[1].key, "count");
    assert_eq!(sections[0].entries[1].value, "42");
}

#[test]
fn test_sections() {
    #[derive(DebugState, Serialize)]
    struct AppState {
        #[debug(section = "Connection")]
        host: String,
        #[debug(section = "Connection")]
        port: u16,

        #[debug(section = "UI")]
        scroll_offset: usize,
    }

    let state = AppState {
        host: "localhost".to_string(),
        port: 8080,
        scroll_offset: 10,
    };

    let sections = state.debug_sections();
    assert_eq!(sections.len(), 2);

    // First section: Connection
    assert_eq!(sections[0].title, "Connection");
    assert_eq!(sections[0].entries.len(), 2);
    assert_eq!(sections[0].entries[0].key, "host");
    assert_eq!(sections[0].entries[0].value, "\"localhost\"");
    assert_eq!(sections[0].entries[1].key, "port");
    assert_eq!(sections[0].entries[1].value, "8080");

    // Second section: UI
    assert_eq!(sections[1].title, "UI");
    assert_eq!(sections[1].entries.len(), 1);
    assert_eq!(sections[1].entries[0].key, "scroll_offset");
    assert_eq!(sections[1].entries[0].value, "10");
}

#[test]
fn test_skip() {
    #[derive(DebugState, Serialize)]
    struct StateWithSkip {
        visible: String,
        #[debug(skip)]
        internal: String,
    }

    let state = StateWithSkip {
        visible: "show".to_string(),
        internal: "hide".to_string(),
    };

    let sections = state.debug_sections();
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].entries.len(), 1);
    assert_eq!(sections[0].entries[0].key, "visible");
    assert_eq!(sections[0].entries[0].value, "\"show\"");
}

#[test]
fn test_custom_label() {
    #[derive(DebugState, Serialize)]
    struct LabeledState {
        #[debug(label = "Server Host")]
        host: String,
    }

    let state = LabeledState {
        host: "example.com".to_string(),
    };

    let sections = state.debug_sections();
    assert_eq!(sections[0].entries[0].key, "Server Host");
    assert_eq!(sections[0].entries[0].value, "\"example.com\"");
}

#[test]
fn test_debug_fmt() {
    #[derive(Debug, Serialize)]
    enum Status {
        Connected,
    }

    #[derive(DebugState, Serialize)]
    struct StateWithDebug {
        #[debug(debug_fmt)]
        status: Status,
    }

    let state = StateWithDebug {
        status: Status::Connected,
    };

    let sections = state.debug_sections();
    assert_eq!(sections[0].entries[0].value, "Connected");
}

#[test]
fn test_combined_attributes() {
    #[derive(Debug, Serialize)]
    enum Level {
        High,
    }

    #[derive(DebugState, Serialize)]
    struct CombinedState {
        #[debug(section = "Info", label = "Full Name")]
        name: String,

        #[debug(section = "Info")]
        count: usize,

        #[debug(section = "Status", debug_fmt)]
        level: Level,

        #[debug(skip)]
        cache: Vec<u8>,
    }

    let state = CombinedState {
        name: "Alice".to_string(),
        count: 5,
        level: Level::High,
        cache: vec![1, 2, 3],
    };

    let sections = state.debug_sections();
    assert_eq!(sections.len(), 2);

    // Info section
    assert_eq!(sections[0].title, "Info");
    assert_eq!(sections[0].entries.len(), 2);
    assert_eq!(sections[0].entries[0].key, "Full Name");
    assert_eq!(sections[0].entries[0].value, "\"Alice\"");
    assert_eq!(sections[0].entries[1].key, "count");

    // Status section
    assert_eq!(sections[1].title, "Status");
    assert_eq!(sections[1].entries[0].key, "level");
    assert_eq!(sections[1].entries[0].value, "High");
}

#[test]
fn test_build_debug_table() {
    #[derive(DebugState, Serialize)]
    struct TableState {
        #[debug(section = "Data")]
        value: String,
    }

    let state = TableState {
        value: "test".to_string(),
    };

    let table = state.build_debug_table("My Table");
    assert_eq!(table.title, "My Table");
    assert!(!table.rows.is_empty());
}
