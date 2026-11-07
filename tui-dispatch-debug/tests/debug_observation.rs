use serde::{Serialize, Serializer};
use serde_json::json;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tui_dispatch_core::{
    Action, DispatchError, DispatchLimits, Middleware, ReducerResult, Store, StoreWithMiddleware,
};
use tui_dispatch_debug::observe::{DebugCollector, DebugStoreExt, DispatchOutcome};

#[derive(Clone, Debug, Serialize)]
enum TestAction {
    Root,
    Left,
    Leaf,
    Right,
    Cancel,
    Noop,
}
impl Action for TestAction {
    fn name(&self) -> &'static str {
        match self {
            Self::Root => "Root",
            Self::Left => "Left",
            Self::Leaf => "Leaf",
            Self::Right => "Right",
            Self::Cancel => "Cancel",
            Self::Noop => "Noop",
        }
    }
}
#[derive(Default, Serialize)]
struct State {
    count: usize,
}
fn reduce(state: &mut State, action: TestAction) -> ReducerResult<String> {
    match action {
        TestAction::Noop => ReducerResult::unchanged(),
        TestAction::Cancel => panic!("cancelled actions must not reduce"),
        _ => {
            state.count += 1;
            ReducerResult::changed().with(action.name().into())
        }
    }
}
struct Cascade;
impl Middleware<State, TestAction> for Cascade {
    fn before(&mut self, action: &TestAction, _: &State) -> bool {
        !matches!(action, TestAction::Cancel)
    }
    fn after(&mut self, action: &TestAction, _: bool, _: &State) -> Vec<TestAction> {
        match action {
            TestAction::Root => vec![TestAction::Left, TestAction::Cancel, TestAction::Right],
            TestAction::Left => vec![TestAction::Leaf],
            _ => vec![],
        }
    }
}
#[test]
fn observes_each_boundary_before_children_without_a_frame() {
    let collector = DebugCollector::new(json!({"count":0}), 16);
    let mut store = StoreWithMiddleware::new(State::default(), reduce, Cascade)
        .with_observer(collector.observer());
    store.dispatch(TestAction::Cancel);
    let result = store.dispatch(TestAction::Root);
    assert_eq!(result.effects, ["Root", "Left", "Leaf", "Right"]);
    store.dispatch(TestAction::Noop);
    let records = collector.records();
    assert_eq!(
        records.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
        ["Cancel", "Root", "Left", "Leaf", "Cancel", "Right", "Noop"]
    );
    assert_eq!(
        records
            .iter()
            .map(|r| r.state["count"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        [0, 1, 2, 3, 3, 4, 4]
    );
    assert_eq!(
        records
            .iter()
            .map(|r| r.parent_action_id)
            .collect::<Vec<_>>(),
        [None, None, Some(2), Some(3), Some(2), Some(2), None]
    );
    assert_eq!(records[0].outcome, DispatchOutcome::Cancelled);
    assert_eq!(records[4].outcome, DispatchOutcome::Cancelled);
    assert_eq!(records[6].outcome, DispatchOutcome::Applied);
    assert!(!records[6].changed);
    for (index, record) in records.iter().enumerate() {
        assert_eq!(record.sequence, index as u64 + 1);
        assert_eq!(record.action_id, record.sequence);
        assert!(record.serialization_errors.is_empty());
    }
}
#[test]
fn plain_store_records_payload_and_snapshots_are_owned() {
    #[derive(Clone, Debug, Serialize)]
    struct Add(usize);
    impl Action for Add {
        fn name(&self) -> &'static str {
            "Add"
        }
    }
    let collector = DebugCollector::new(json!({"count":0}), 2);
    let mut store = Store::new(State::default(), |s, a: Add| {
        s.count += a.0;
        ReducerResult::<tui_dispatch_core::NoEffect>::changed()
    })
    .with_observer(collector.observer());
    store.dispatch(Add(7));
    store.state_mut().count = 99;
    let records = collector.records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].action, json!(7));
    assert_eq!(records[0].state, json!({"count":7}));
    assert_eq!(collector.initial_state(), &json!({"count":0}));
    store.clear_observer();
    store.dispatch(Add(1));
    assert_eq!(collector.records(), records);
}
#[test]
fn eviction_keeps_ids_and_injection_parent_even_after_parent_eviction() {
    let collector = DebugCollector::new(json!({"count":0}), 2);
    let mut store = StoreWithMiddleware::new(State::default(), reduce, Cascade)
        .with_observer(collector.observer());
    store.dispatch(TestAction::Root);
    let records = collector.records();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].sequence, 4);
    assert_eq!(records[1].sequence, 5);
    assert_eq!(records[1].parent_action_id, Some(1));
    store.dispatch(TestAction::Noop);
    assert_eq!(collector.records()[1].sequence, 6);
}
#[test]
fn limit_rejections_preserve_partial_states_and_parentage() {
    for limits in [
        DispatchLimits {
            max_depth: 2,
            max_actions: 100,
        },
        DispatchLimits {
            max_depth: 100,
            max_actions: 2,
        },
    ] {
        let collector = DebugCollector::new(json!({"count":0}), 10);
        let mut store = StoreWithMiddleware::new(State::default(), reduce, Cascade)
            .with_dispatch_limits(limits)
            .with_observer(collector.observer());
        let error = store.try_dispatch(TestAction::Root).unwrap_err();
        assert!(matches!(
            error,
            DispatchError::DepthExceeded { .. } | DispatchError::ActionBudgetExceeded { .. }
        ));
        let records = collector.records();
        assert_eq!(records.len(), 3);
        assert_eq!(records[2].outcome, DispatchOutcome::Rejected(error));
        assert_eq!(records[2].parent_action_id, Some(2));
        assert_eq!(records[2].state, json!({"count":2}));
        assert!(!records[2].changed);
        store.dispatch(TestAction::Noop);
        assert_eq!(collector.records()[3].parent_action_id, None);
    }
}
#[test]
fn separate_observers_share_sequence_but_keep_separate_ancestry() {
    let collector = DebugCollector::new(json!({"count":0}), 32);
    let mut first = StoreWithMiddleware::new(State::default(), reduce, Cascade)
        .with_observer(collector.observer());
    let mut second = Store::new(State::default(), reduce).with_observer(collector.observer());
    first.dispatch(TestAction::Root);
    second.dispatch(TestAction::Left);
    first.dispatch(TestAction::Root);
    let records = collector.records();
    assert_eq!(records[5].parent_action_id, None);
    assert_eq!(records[6].parent_action_id, None);
    assert_eq!(records[7].parent_action_id, Some(7));
    assert!(records
        .windows(2)
        .all(|p| p[0].sequence + 1 == p[1].sequence));
}

#[derive(Debug)]
struct Probe(Arc<AtomicUsize>);
impl Clone for Probe {
    fn clone(&self) -> Self {
        self.0.fetch_add(1, Ordering::SeqCst);
        Self(self.0.clone())
    }
}
impl Action for Probe {
    fn name(&self) -> &'static str {
        "Probe"
    }
}
impl Serialize for Probe {
    fn serialize<S: Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        panic!("disabled actions must not serialize")
    }
}
struct UnserializableState;
impl Serialize for UnserializableState {
    fn serialize<S: Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        panic!("disabled state must not serialize")
    }
}
#[test]
fn disabled_and_zero_capacity_skip_serialization_and_extra_plain_store_clones() {
    for capacity in [0, 8] {
        let collector = DebugCollector::new(json!(null), capacity);
        if capacity > 0 {
            collector.set_enabled(false);
        } else {
            collector.set_enabled(true);
        }
        assert!(!collector.is_enabled());
        let clones = Arc::new(AtomicUsize::new(0));
        let mut store = Store::new(UnserializableState, |_, _: Probe| {
            ReducerResult::<tui_dispatch_core::NoEffect>::unchanged()
        })
        .with_observer(collector.observer());
        store.dispatch(Probe(clones.clone()));
        assert_eq!(clones.load(Ordering::SeqCst), 0);
        let mut store = StoreWithMiddleware::new(
            UnserializableState,
            |_, _: Probe| ReducerResult::<tui_dispatch_core::NoEffect>::unchanged(),
            tui_dispatch_core::NoopMiddleware,
        )
        .with_observer(collector.observer());
        store.dispatch(Probe(clones));
        assert!(collector.records().is_empty());
    }
}
#[test]
fn disabling_retains_history_and_reenable_keeps_sequence() {
    let collector = DebugCollector::new(json!({"count":0}), 10);
    let mut store = Store::new(State::default(), reduce).with_observer(collector.observer());
    store.dispatch(TestAction::Left);
    collector.set_enabled(false);
    store.dispatch(TestAction::Right);
    assert_eq!(collector.records().len(), 1);
    collector.set_enabled(true);
    store.dispatch(TestAction::Root);
    let records = collector.records();
    assert_eq!(records[1].sequence, 2);
    assert_eq!(records[1].state, json!({"count":3}));
    assert_eq!(records[1].parent_action_id, None);
}
#[test]
fn serialization_failures_are_explicit_and_do_not_change_reducer_result() {
    #[derive(Clone, Debug)]
    struct Bad;
    impl Action for Bad {
        fn name(&self) -> &'static str {
            "Bad"
        }
    }
    impl Serialize for Bad {
        fn serialize<S: Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("bad payload"))
        }
    }
    let collector = DebugCollector::new(json!(0), 2);
    let mut store = Store::new(0usize, |s, _: Bad| {
        *s += 1;
        ReducerResult::<tui_dispatch_core::NoEffect>::changed()
    })
    .with_observer(collector.observer());
    assert!(store.dispatch(Bad).changed);
    let record = &collector.records()[0];
    assert_eq!(record.action, json!(null));
    assert_eq!(record.state, json!(1));
    assert_eq!(record.serialization_errors, ["action: bad payload"]);
}
#[test]
fn serializers_can_read_history_without_holding_its_lock() {
    #[derive(Clone)]
    struct Inspect(DebugCollector);
    impl std::fmt::Debug for Inspect {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("Inspect")
        }
    }
    impl Action for Inspect {
        fn name(&self) -> &'static str {
            "Inspect"
        }
    }
    impl Serialize for Inspect {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            self.0.records().len().serialize(serializer)
        }
    }
    let collector = DebugCollector::new(json!(0), 2);
    let mut store = Store::new(0usize, |_, _: Inspect| {
        ReducerResult::<tui_dispatch_core::NoEffect>::unchanged()
    })
    .with_observer(collector.observer());
    store.dispatch(Inspect(collector.clone()));
    assert_eq!(collector.records()[0].action, json!(0));
}

#[tokio::test]
async fn real_runtime_collects_without_debug_adapter() {
    use ratatui::{backend::TestBackend, Terminal};
    use tui_dispatch_core::{EventOutcome, Runtime};
    let collector = DebugCollector::new(json!({"count":0}), 10);
    let store = StoreWithMiddleware::new(State::default(), reduce, Cascade)
        .with_observer(collector.observer());
    let mut runtime =
        Runtime::from_store(store).with_event_source(|_, _| tokio::spawn(std::future::pending()));
    runtime.enqueue(TestAction::Root);
    runtime.enqueue(TestAction::Noop);
    let mut terminal = Terminal::new(TestBackend::new(10, 2)).unwrap();
    let mut effects = Vec::new();
    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        runtime.run_with_effects(
            &mut terminal,
            |_, _, _, _| {},
            |_, _| EventOutcome::ignored(),
            |action| matches!(action, TestAction::Noop),
            |effect, _| effects.push(effect),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(collector.records().len(), 5);
    assert_eq!(runtime.state().count, 4);
    assert_eq!(effects, ["Root", "Left", "Leaf", "Right"]);
}
