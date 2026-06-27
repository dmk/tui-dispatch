//! Downstream-facing construction and matching contract for extensible public types.

use tui_dispatch_core::{
    DispatchError, DispatchErrorPolicy, EventKind, NumericComponentId, ReducerResult,
    RenderContext, RouteTarget,
};

#[test]
fn extensible_public_types_remain_constructible() {
    let render_context = RenderContext::new(true);
    assert!(render_context.debug_enabled);
    assert!(!render_context.is_focused());
    assert!(!RenderContext::default().debug_enabled);

    let event = EventKind::Tick;
    assert!(matches!(event, EventKind::Tick));

    let target = RouteTarget::<NumericComponentId>::Global;
    assert!(matches!(target, RouteTarget::Global));

    let error = DispatchError::DepthExceeded {
        max_depth: 16,
        action: "Loop",
    };
    assert!(matches!(error, DispatchError::DepthExceeded { .. }));

    assert_eq!(DispatchErrorPolicy::default(), DispatchErrorPolicy::Stop);

    let result = ReducerResult::changed().with("save");
    assert!(result.changed);
    assert_eq!(result.effects, ["save"]);

    let result = ReducerResult::effects(vec!["save", "notify"]).mark_changed();
    assert!(result.changed);
    assert_eq!(result.effects, ["save", "notify"]);
}

#[test]
#[allow(deprecated)]
fn deprecated_reducer_result_helpers_remain_compatible() {
    let single = ReducerResult::changed_with("save");
    assert!(single.changed);
    assert_eq!(single.effects, ["save"]);

    let many = ReducerResult::changed_with_many(vec!["save", "notify"]);
    assert!(many.changed);
    assert_eq!(many.effects, ["save", "notify"]);
}
