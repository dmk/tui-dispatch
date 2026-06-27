//! Contract test: every derive works with a renamed core dependency and no facade crate.

use dispatch_core::__macro_support::DebugState as _;
use dispatch_core::{
    Action as _, ActionParams as _, BindingContext as _, ComponentId as _, FeatureFlags as _,
};
use tui_dispatch_macros::{
    Action as DeriveAction, BindingContext as DeriveBindingContext,
    ComponentId as DeriveComponentId, DebugState as DeriveDebugState,
    FeatureFlags as DeriveFeatureFlags,
};

#[derive(Clone, Debug, DeriveAction)]
#[action(infer_categories)]
enum CoreAction {
    DataLoad { id: u64 },
    DataDidLoad(String),
}

#[derive(Clone, Copy, Debug, DeriveComponentId, Eq, Hash, PartialEq)]
enum CoreComponentId {
    Main,
    Status,
}

#[derive(Clone, Copy, Debug, DeriveBindingContext, Eq, Hash, PartialEq)]
enum CoreBindingContext {
    Default,
    Search,
}

#[derive(DeriveDebugState)]
struct CoreState {
    #[debug(section = "Data")]
    count: usize,
}

#[derive(DeriveFeatureFlags)]
struct CoreFeatures {
    #[flag(default = true)]
    enabled: bool,
    experimental: bool,
}

#[test]
fn all_derives_work_without_the_facade() {
    let action = CoreAction::DataLoad { id: 7 };
    assert_eq!(action.name(), "DataLoad");
    assert_eq!(action.params(), "{id: 7}");
    assert_eq!(
        <CoreAction as dispatch_core::ActionCategory>::category(&action),
        Some("data")
    );

    let loaded = CoreAction::DataDidLoad("done".into());
    assert_eq!(loaded.params(), "\"done\"");

    assert_eq!(CoreComponentId::Main.name(), "Main");
    assert_eq!(CoreComponentId::Status.name(), "Status");

    assert_eq!(CoreBindingContext::Search.name(), "search");
    assert_eq!(
        CoreBindingContext::from_name("default"),
        Some(CoreBindingContext::Default)
    );

    let sections = CoreState { count: 3 }.debug_sections();
    assert_eq!(sections[0].title, "Data");
    assert_eq!(sections[0].entries[0].value, "3");

    let mut features = CoreFeatures::default();
    assert_eq!(features.is_enabled("enabled"), Some(true));
    assert!(features.set("experimental", true));
    assert_eq!(features.is_enabled("experimental"), Some(true));
}
