//! Contract test: direct debug-crate users get the full DebugState implementation.

use tui_dispatch_debug::debug::DebugState as _;
use tui_dispatch_macros::DebugState as DeriveDebugState;

#[derive(DeriveDebugState)]
struct DirectState {
    #[debug(section = "Runtime")]
    active: bool,
}

#[test]
fn derive_integrates_with_full_debug_api_without_facade() {
    let state = DirectState { active: true };

    let sections = state.debug_sections();
    assert_eq!(sections[0].entries[0].value, "true");

    let table = state.build_debug_table("Direct");
    assert_eq!(table.title, "Direct");
    assert!(!table.rows.is_empty());
}
