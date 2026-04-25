//! Bus-routing contract tests for the unified `Runtime`.
//!
//! Covers accessor availability before `run`, the `run_with_hooks`
//! post-render hook, and feature-gated accessors after `.with_event_bus(...)`.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use ratatui::backend::TestBackend;
use ratatui::Terminal;

use tui_dispatch_core::{
    Action, BindingContext, EventBus, EventRoutingState, Keybindings, NumericComponentId,
    PollerConfig, ReducerResult, Runtime,
};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
enum TestAction {
    Tick,
    Quit,
}

impl Action for TestAction {
    fn name(&self) -> &'static str {
        match self {
            TestAction::Tick => "Tick",
            TestAction::Quit => "Quit",
        }
    }
}

#[derive(Default)]
struct TestState {
    ticks: u32,
}

impl EventRoutingState<NumericComponentId, TestCtx> for TestState {
    fn focused(&self) -> Option<NumericComponentId> {
        None
    }

    fn modal(&self) -> Option<NumericComponentId> {
        None
    }

    fn binding_context(&self, _id: NumericComponentId) -> TestCtx {
        TestCtx
    }

    fn default_context(&self) -> TestCtx {
        TestCtx
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
struct TestCtx;

impl BindingContext for TestCtx {
    fn name(&self) -> &'static str {
        "default"
    }

    fn from_name(name: &str) -> Option<Self> {
        (name == "default").then_some(Self)
    }

    fn all() -> &'static [Self] {
        &[Self]
    }
}

fn reducer(state: &mut TestState, action: TestAction) -> ReducerResult {
    match action {
        TestAction::Tick => {
            state.ticks += 1;
            ReducerResult::changed()
        }
        TestAction::Quit => ReducerResult::unchanged(),
    }
}

fn effect_reducer(state: &mut TestState, action: TestAction) -> ReducerResult<TestEffect> {
    match action {
        TestAction::Tick => {
            state.ticks += 1;
            ReducerResult::changed_with(TestEffect::EchoTick)
        }
        TestAction::Quit => ReducerResult::unchanged(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestEffect {
    EchoTick,
}

fn fast_poller() -> PollerConfig {
    PollerConfig {
        poll_timeout: Duration::from_millis(1),
        loop_sleep: Duration::from_millis(1),
    }
}

fn test_terminal() -> Terminal<TestBackend> {
    Terminal::new(TestBackend::new(40, 8)).expect("test terminal")
}

fn quit_on_quit(action: &TestAction) -> bool {
    matches!(action, TestAction::Quit)
}

fn make_bus() -> (
    EventBus<TestState, TestAction, NumericComponentId, TestCtx>,
    Keybindings<TestCtx>,
) {
    (EventBus::new(), Keybindings::new())
}

// ---------------------------------------------------------------------------
// Runtime + bus — accessors and basic loop drive
// ---------------------------------------------------------------------------

#[tokio::test]
async fn bus_runtime_accessors_available_before_run() {
    let (bus, keybindings) = make_bus();
    let runtime = Runtime::new(TestState::default(), reducer).with_event_bus(bus, keybindings);

    // Accessors should be callable without running the loop.
    assert_eq!(runtime.state().ticks, 0);
    let _ = runtime.bus();
    let _ = runtime.keybindings();
    let _tx = runtime.action_tx();
    runtime.enqueue(TestAction::Quit);
}

#[tokio::test]
async fn bus_runtime_mut_accessors_available_before_run() {
    let (bus, keybindings) = make_bus();
    let mut runtime = Runtime::new(TestState::default(), reducer).with_event_bus(bus, keybindings);

    // Mutable accessors are how callers register components/keybindings.
    let _: &mut EventBus<_, _, _, _> = runtime.bus_mut();
    let _: &mut Keybindings<_> = runtime.keybindings_mut();
}

#[tokio::test]
async fn bus_runtime_run_drains_queued_actions() {
    let (bus, keybindings) = make_bus();
    let mut runtime = Runtime::new(TestState::default(), reducer)
        .with_event_poller(fast_poller())
        .with_event_bus(bus, keybindings);

    runtime.enqueue(TestAction::Tick);
    runtime.enqueue(TestAction::Tick);
    runtime.enqueue(TestAction::Quit);

    let mut term = test_terminal();
    runtime
        .run(&mut term, |_, _, _, _, _| {}, quit_on_quit)
        .await
        .expect("runtime exits cleanly");

    assert_eq!(runtime.state().ticks, 2);
}

#[tokio::test]
async fn bus_runtime_run_with_hooks_fires_after_render_callback() {
    let (bus, keybindings) = make_bus();
    let mut runtime = Runtime::new(TestState::default(), reducer)
        .with_event_poller(fast_poller())
        .with_event_bus(bus, keybindings);

    runtime.enqueue(TestAction::Tick);
    runtime.enqueue(TestAction::Quit);

    let calls: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));
    let calls_clone = calls.clone();

    let mut term = test_terminal();
    runtime
        .run_with_hooks(
            &mut term,
            |_, _, _, _, _| {},
            quit_on_quit,
            move |_bus: &mut EventBus<TestState, TestAction, NumericComponentId, TestCtx>,
                  _state: &TestState| {
                *calls_clone.lock().unwrap() += 1;
            },
        )
        .await
        .expect("runtime exits cleanly");

    assert!(
        *calls.lock().unwrap() >= 1,
        "after_render hook must fire at least once per rendered frame"
    );
}

// Compile-pass: closures capturing local &mut state across `run(...)`.
#[tokio::test]
async fn bus_runtime_run_accepts_borrowing_render_closure() {
    let (bus, keybindings) = make_bus();
    let mut runtime = Runtime::new(TestState::default(), reducer)
        .with_event_poller(fast_poller())
        .with_event_bus(bus, keybindings);

    let mut render_count: u32 = 0;
    let owned = vec![1u8, 2, 3];

    runtime.enqueue(TestAction::Tick);
    runtime.enqueue(TestAction::Quit);

    let mut term = test_terminal();
    runtime
        .run(
            &mut term,
            |_frame, _area, _state, _ctx, _event_ctx| {
                render_count += 1;
                let _ = owned.len();
            },
            quit_on_quit,
        )
        .await
        .expect("runtime exits cleanly");

    assert!(render_count >= 1);
    assert_eq!(owned, vec![1, 2, 3]);
}

// ---------------------------------------------------------------------------
// Runtime + bus + effects
// ---------------------------------------------------------------------------

#[tokio::test]
async fn bus_runtime_run_with_effects_dispatches_effects() {
    let (bus, keybindings) = make_bus();
    let mut runtime = Runtime::new(TestState::default(), effect_reducer)
        .with_event_poller(fast_poller())
        .with_event_bus(bus, keybindings);

    runtime.enqueue(TestAction::Tick);
    runtime.enqueue(TestAction::Quit);

    let effects = Arc::new(Mutex::new(Vec::new()));
    let effects_clone = effects.clone();

    let mut term = test_terminal();
    runtime
        .run_with_effects(
            &mut term,
            |_, _, _, _, _| {},
            quit_on_quit,
            move |effect, _ctx| {
                effects_clone.lock().unwrap().push(effect);
            },
        )
        .await
        .expect("runtime exits cleanly");

    assert_eq!(runtime.state().ticks, 1);
    assert_eq!(&*effects.lock().unwrap(), &[TestEffect::EchoTick]);
}

#[tokio::test]
async fn bus_runtime_subscribe_actions_survives_wrapping() {
    let (bus, keybindings) = make_bus();
    let mut runtime = Runtime::new(TestState::default(), effect_reducer)
        .with_event_poller(fast_poller())
        .with_event_bus(bus, keybindings);

    let mut rx = runtime.subscribe_actions();

    runtime.enqueue(TestAction::Tick);
    runtime.enqueue(TestAction::Quit);

    let mut term = test_terminal();
    runtime
        .run_with_effects(
            &mut term,
            |_, _, _, _, _| {},
            quit_on_quit,
            |_effect, _ctx| {},
        )
        .await
        .expect("runtime exits cleanly");

    assert_eq!(rx.try_recv().expect("Tick broadcast"), "Tick");
}

#[tokio::test]
async fn bus_runtime_run_with_effect_hooks_fires_after_render_callback() {
    let (bus, keybindings) = make_bus();
    let mut runtime = Runtime::new(TestState::default(), effect_reducer)
        .with_event_poller(fast_poller())
        .with_event_bus(bus, keybindings);

    runtime.enqueue(TestAction::Tick);
    runtime.enqueue(TestAction::Quit);

    let calls: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));
    let calls_clone = calls.clone();

    let mut term = test_terminal();
    runtime
        .run_with_effect_hooks(
            &mut term,
            |_, _, _, _, _| {},
            quit_on_quit,
            |_effect, _ctx| {},
            move |_bus: &mut EventBus<TestState, TestAction, NumericComponentId, TestCtx>,
                  _state: &TestState| {
                *calls_clone.lock().unwrap() += 1;
            },
        )
        .await
        .expect("runtime exits cleanly");

    assert!(*calls.lock().unwrap() >= 1);
}

#[cfg(feature = "tasks")]
#[tokio::test]
async fn bus_runtime_tasks_accessor_forwards() {
    let (bus, keybindings) = make_bus();
    let mut runtime = Runtime::new(TestState::default(), effect_reducer)
        .with_event_poller(fast_poller())
        .with_event_bus(bus, keybindings);

    runtime.tasks().spawn("long", async {
        tokio::time::sleep(Duration::from_secs(60)).await;
        TestAction::Tick
    });
    assert_eq!(runtime.tasks().len(), 1);

    runtime.enqueue(TestAction::Quit);

    let mut term = test_terminal();
    runtime
        .run_with_effects(
            &mut term,
            |_, _, _, _, _| {},
            quit_on_quit,
            |_effect, _ctx| {},
        )
        .await
        .expect("runtime exits cleanly");

    assert!(
        runtime.tasks().is_empty(),
        "tasks must be cancelled on shutdown through the bus wrapper"
    );
}

#[cfg(feature = "subscriptions")]
#[tokio::test]
async fn bus_runtime_subscriptions_accessor_forwards() {
    let (bus, keybindings) = make_bus();
    let mut runtime = Runtime::new(TestState::default(), effect_reducer)
        .with_event_poller(fast_poller())
        .with_event_bus(bus, keybindings);

    runtime
        .subscriptions()
        .interval("tick", Duration::from_secs(60), || TestAction::Tick);
    assert_eq!(runtime.subscriptions().len(), 1);

    runtime.enqueue(TestAction::Quit);

    let mut term = test_terminal();
    runtime
        .run_with_effects(
            &mut term,
            |_, _, _, _, _| {},
            quit_on_quit,
            |_effect, _ctx| {},
        )
        .await
        .expect("runtime exits cleanly");

    assert!(
        runtime.subscriptions().is_empty(),
        "subscriptions must be cancelled on shutdown through the bus wrapper"
    );
}

// ---------------------------------------------------------------------------
// Compile-pass guards for the bus-wrapper helper signatures.
// ---------------------------------------------------------------------------

#[allow(dead_code, unused_variables)]
fn _compile_pass_bus_signatures() {
    let (bus, kb) = make_bus();
    let runtime = Runtime::new(TestState::default(), reducer).with_event_bus(bus, kb);

    // action_tx is clone-able and Send
    let tx = runtime.action_tx();
    let _spawn: tokio::task::JoinHandle<()> = tokio::spawn(async move {
        let _ = tx.send(TestAction::Tick);
    });

    let (bus, kb) = make_bus();
    let _effect = Runtime::new(TestState::default(), effect_reducer).with_event_bus(bus, kb);
}
