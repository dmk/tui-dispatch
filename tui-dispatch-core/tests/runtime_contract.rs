//! Runtime contract tests for `DispatchRuntime` and `EffectRuntime`.
//!
//! These tests pin the public runtime contract before any internal refactor.
//! They cover the surface that downstream code already relies on:
//!
//! * helper availability before `run(...)` — `enqueue`, `action_tx`,
//!   `subscribe_actions`
//! * end-to-end `run(...)` loop drains the action queue and exits on the
//!   user-supplied quit predicate
//! * shutdown cleanup of the event poller, tasks, and subscriptions
//! * dispatch error-policy behavior through the live loop
//! * borrowing-friendly closure ergonomics for `run(...)` (compile-pass +
//!   behavioral)

use std::sync::{Arc, Mutex};
use std::time::Duration;

use ratatui::backend::TestBackend;
use ratatui::Terminal;

use tui_dispatch_core::{
    Action, DispatchError, DispatchErrorPolicy, DispatchLimits, DispatchRuntime, EffectRuntime,
    EventOutcome, Middleware, PollerConfig, ReducerResult, StoreWithMiddleware,
};

// ---------------------------------------------------------------------------
// Test fixtures
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

fn reducer(state: &mut TestState, action: TestAction) -> bool {
    match action {
        TestAction::Tick => {
            state.ticks += 1;
            true
        }
        TestAction::Quit => false,
    }
}

fn effect_reducer(state: &mut TestState, action: TestAction) -> ReducerResult<()> {
    match action {
        TestAction::Tick => {
            state.ticks += 1;
            ReducerResult::changed()
        }
        TestAction::Quit => ReducerResult::unchanged(),
    }
}

fn test_terminal() -> Terminal<TestBackend> {
    Terminal::new(TestBackend::new(40, 8)).expect("test terminal")
}

/// Use a brisk poller so the loop reacts to enqueued actions fast in tests.
fn fast_poller() -> PollerConfig {
    PollerConfig {
        poll_timeout: Duration::from_millis(1),
        loop_sleep: Duration::from_millis(1),
    }
}

fn quit_on_quit(action: &TestAction) -> bool {
    matches!(action, TestAction::Quit)
}

// ---------------------------------------------------------------------------
// Helper availability — enqueue / action_tx / subscribe_actions
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dispatch_runtime_enqueue_drains_through_run() {
    let mut runtime =
        DispatchRuntime::new(TestState::default(), reducer).with_event_poller(fast_poller());

    runtime.enqueue(TestAction::Tick);
    runtime.enqueue(TestAction::Tick);
    runtime.enqueue(TestAction::Quit);

    let mut term = test_terminal();
    runtime
        .run(
            &mut term,
            |_, _, _, _| {},
            |_, _: &TestState| EventOutcome::ignored(),
            quit_on_quit,
        )
        .await
        .expect("runtime exits cleanly");

    assert_eq!(runtime.state().ticks, 2);
}

#[tokio::test]
async fn dispatch_runtime_action_tx_clone_can_dispatch_from_other_task() {
    let mut runtime =
        DispatchRuntime::new(TestState::default(), reducer).with_event_poller(fast_poller());

    let tx = runtime.action_tx();
    tokio::spawn(async move {
        tx.send(TestAction::Tick).expect("tx alive");
        tx.send(TestAction::Quit).expect("tx alive");
    });

    let mut term = test_terminal();
    runtime
        .run(
            &mut term,
            |_, _, _, _| {},
            |_, _: &TestState| EventOutcome::ignored(),
            quit_on_quit,
        )
        .await
        .expect("runtime exits cleanly");

    assert_eq!(runtime.state().ticks, 1);
}

#[tokio::test]
async fn effect_runtime_enqueue_drains_through_run() {
    let mut runtime =
        EffectRuntime::new(TestState::default(), effect_reducer).with_event_poller(fast_poller());

    runtime.enqueue(TestAction::Tick);
    runtime.enqueue(TestAction::Quit);

    let mut term = test_terminal();
    runtime
        .run(
            &mut term,
            |_, _, _, _| {},
            |_, _: &TestState| EventOutcome::ignored(),
            quit_on_quit,
            |(), _ctx| {},
        )
        .await
        .expect("runtime exits cleanly");

    assert_eq!(runtime.state().ticks, 1);
}

#[tokio::test]
async fn effect_runtime_subscribe_actions_broadcasts_dispatched_names() {
    let mut runtime =
        EffectRuntime::new(TestState::default(), effect_reducer).with_event_poller(fast_poller());

    let mut rx = runtime.subscribe_actions();

    runtime.enqueue(TestAction::Tick);
    runtime.enqueue(TestAction::Quit);

    let mut term = test_terminal();
    runtime
        .run(
            &mut term,
            |_, _, _, _| {},
            |_, _: &TestState| EventOutcome::ignored(),
            quit_on_quit,
            |(), _ctx| {},
        )
        .await
        .expect("runtime exits cleanly");

    assert_eq!(rx.try_recv().expect("Tick broadcast"), "Tick");
    // Quit is checked before broadcast — the runtime breaks out of the loop
    // before reaching broadcast_action, so it should NOT appear here.
    assert!(rx.try_recv().is_err(), "Quit should not be broadcast");
}

// ---------------------------------------------------------------------------
// Shutdown cleanup — tasks and subscriptions
// ---------------------------------------------------------------------------

#[cfg(feature = "tasks")]
#[tokio::test]
async fn effect_runtime_cancels_tasks_on_quit() {
    let mut runtime =
        EffectRuntime::new(TestState::default(), effect_reducer).with_event_poller(fast_poller());

    runtime.tasks().spawn("long", async {
        tokio::time::sleep(Duration::from_secs(60)).await;
        TestAction::Tick
    });
    assert_eq!(runtime.tasks().len(), 1);

    runtime.enqueue(TestAction::Quit);

    let mut term = test_terminal();
    runtime
        .run(
            &mut term,
            |_, _, _, _| {},
            |_, _: &TestState| EventOutcome::ignored(),
            quit_on_quit,
            |(), _ctx| {},
        )
        .await
        .expect("runtime exits cleanly");

    assert!(
        runtime.tasks().is_empty(),
        "tasks should be cancelled on shutdown"
    );
}

#[cfg(feature = "subscriptions")]
#[tokio::test]
async fn effect_runtime_cancels_subscriptions_on_quit() {
    let mut runtime =
        EffectRuntime::new(TestState::default(), effect_reducer).with_event_poller(fast_poller());

    runtime
        .subscriptions()
        .interval("tick", Duration::from_secs(60), || TestAction::Tick);
    assert_eq!(runtime.subscriptions().len(), 1);

    runtime.enqueue(TestAction::Quit);

    let mut term = test_terminal();
    runtime
        .run(
            &mut term,
            |_, _, _, _| {},
            |_, _: &TestState| EventOutcome::ignored(),
            quit_on_quit,
            |(), _ctx| {},
        )
        .await
        .expect("runtime exits cleanly");

    assert!(
        runtime.subscriptions().is_empty(),
        "subscriptions should be cancelled on shutdown"
    );
}

#[cfg(all(feature = "tasks", feature = "subscriptions"))]
#[tokio::test]
async fn effect_runtime_cleans_up_on_dispatch_error_stop() {
    // A middleware that always re-injects, paired with depth=1, forces every
    // dispatch to fail. The Stop policy must still trigger task/subscription
    // cleanup the same way the quit-path does.
    struct LoopMiddleware;
    impl Middleware<TestState, TestAction> for LoopMiddleware {
        fn before(&mut self, _action: &TestAction, _state: &TestState) -> bool {
            true
        }
        fn after(
            &mut self,
            _action: &TestAction,
            _changed: bool,
            _state: &TestState,
        ) -> Vec<TestAction> {
            vec![TestAction::Tick]
        }
    }

    use tui_dispatch_core::EffectStoreWithMiddleware;
    let store =
        EffectStoreWithMiddleware::new(TestState::default(), effect_reducer, LoopMiddleware)
            .with_dispatch_limits(DispatchLimits {
                max_depth: 1,
                max_actions: 100,
            });

    let mut runtime = EffectRuntime::from_store(store)
        .with_event_poller(fast_poller())
        .with_dispatch_error_handler(|_| DispatchErrorPolicy::Stop);

    runtime.tasks().spawn("long", async {
        tokio::time::sleep(Duration::from_secs(60)).await;
        TestAction::Tick
    });
    runtime
        .subscriptions()
        .interval("tick", Duration::from_secs(60), || TestAction::Tick);

    runtime.enqueue(TestAction::Tick);

    let mut term = test_terminal();
    runtime
        .run(
            &mut term,
            |_, _, _, _| {},
            |_, _: &TestState| EventOutcome::ignored(),
            quit_on_quit,
            |(), _ctx| {},
        )
        .await
        .expect("runtime exits cleanly even after Stop policy");

    assert!(
        runtime.tasks().is_empty(),
        "tasks should be cancelled even when shutdown is triggered by Stop policy"
    );
    assert!(
        runtime.subscriptions().is_empty(),
        "subscriptions should be cancelled even when shutdown is triggered by Stop policy"
    );
}

// ---------------------------------------------------------------------------
// Dispatch error-policy behavior through the live `run(...)` loop
// ---------------------------------------------------------------------------

struct LoopMiddleware;

impl Middleware<TestState, TestAction> for LoopMiddleware {
    fn before(&mut self, _action: &TestAction, _state: &TestState) -> bool {
        true
    }
    fn after(
        &mut self,
        _action: &TestAction,
        _changed: bool,
        _state: &TestState,
    ) -> Vec<TestAction> {
        vec![TestAction::Tick]
    }
}

#[tokio::test]
async fn dispatch_error_handler_observes_error_and_stops_loop() {
    let store = StoreWithMiddleware::new(TestState::default(), reducer, LoopMiddleware)
        .with_dispatch_limits(DispatchLimits {
            max_depth: 1,
            max_actions: 100,
        });

    let observed: Arc<Mutex<Vec<DispatchError>>> = Arc::new(Mutex::new(Vec::new()));
    let observed_clone = observed.clone();

    let mut runtime = DispatchRuntime::from_store(store)
        .with_event_poller(fast_poller())
        .with_dispatch_error_handler(move |err| {
            observed_clone.lock().unwrap().push(err.clone());
            DispatchErrorPolicy::Stop
        });

    runtime.enqueue(TestAction::Tick);

    let mut term = test_terminal();
    runtime
        .run(
            &mut term,
            |_, _, _, _| {},
            |_, _: &TestState| EventOutcome::ignored(),
            quit_on_quit,
        )
        .await
        .expect("runtime exits cleanly even after Stop policy");

    let observed = observed.lock().unwrap();
    assert_eq!(observed.len(), 1, "handler should be called exactly once");
    assert!(
        matches!(observed[0], DispatchError::DepthExceeded { .. }),
        "expected depth-exceeded error, got {:?}",
        observed[0]
    );
}

#[tokio::test]
async fn dispatch_error_handler_continue_keeps_loop_alive_until_quit() {
    let store = StoreWithMiddleware::new(TestState::default(), reducer, LoopMiddleware)
        .with_dispatch_limits(DispatchLimits {
            max_depth: 1,
            max_actions: 100,
        });

    let observed: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
    let observed_clone = observed.clone();

    let mut runtime = DispatchRuntime::from_store(store).with_event_poller(fast_poller());

    let tx = runtime.action_tx();
    let tx_for_handler = tx.clone();
    runtime = runtime.with_dispatch_error_handler(move |_| {
        let mut n = observed_clone.lock().unwrap();
        *n += 1;
        // After both Ticks have hit the handler, signal Quit. Removes the
        // timing-window assumption of a fixed pre-Quit sleep.
        if *n >= 2 {
            let _ = tx_for_handler.send(TestAction::Quit);
        }
        DispatchErrorPolicy::Continue
    });

    runtime.enqueue(TestAction::Tick);
    runtime.enqueue(TestAction::Tick);

    let mut term = test_terminal();
    runtime
        .run(
            &mut term,
            |_, _, _, _| {},
            |_, _: &TestState| EventOutcome::ignored(),
            quit_on_quit,
        )
        .await
        .expect("runtime exits cleanly");

    assert!(
        *observed.lock().unwrap() >= 2,
        "Continue policy must keep the loop alive long enough to absorb both Tick errors"
    );
}

// ---------------------------------------------------------------------------
// Closure ergonomics — `run(...)` accepts borrowing-friendly closures.
//
// The compile-pass aspect is the point: if these closures ever stop being
// accepted (e.g. if the runtime grows a `'static` bound or a Send bound on
// captures), this file fails to build.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn run_accepts_closures_that_borrow_local_state() {
    let mut render_count: u32 = 0;
    let label = String::from("hello");
    let mut event_count: u32 = 0;

    let mut runtime =
        DispatchRuntime::new(TestState::default(), reducer).with_event_poller(fast_poller());
    runtime.enqueue(TestAction::Tick);
    runtime.enqueue(TestAction::Quit);

    let mut term = test_terminal();
    runtime
        .run(
            &mut term,
            |_frame, _area, _state, _ctx| {
                render_count += 1; // borrows &mut local
                let _ = label.len(); // borrows &local
            },
            |_event, _state: &TestState| {
                event_count += 1;
                EventOutcome::ignored()
            },
            quit_on_quit,
        )
        .await
        .expect("runtime exits cleanly");

    assert!(render_count >= 1, "render closure must run at least once");
    assert_eq!(label, "hello");
    // event_count may be zero if no real events fire on a TestBackend — we
    // only require the closure to have been accepted by the type system.
    let _ = event_count;
}

#[tokio::test]
async fn effect_run_accepts_borrowing_closures_for_render_event_and_effect() {
    let mut render_count: u32 = 0;
    let mut effect_count: u32 = 0;
    let owned = vec![1u8, 2, 3];

    let mut runtime =
        EffectRuntime::new(TestState::default(), effect_reducer).with_event_poller(fast_poller());

    runtime.enqueue(TestAction::Tick);
    runtime.enqueue(TestAction::Quit);

    let mut term = test_terminal();
    runtime
        .run(
            &mut term,
            |_frame, _area, _state, _ctx| {
                render_count += 1;
                let _ = owned.len();
            },
            |_event, _state: &TestState| EventOutcome::ignored(),
            quit_on_quit,
            |(), _ctx| {
                effect_count += 1;
            },
        )
        .await
        .expect("runtime exits cleanly");

    assert!(render_count >= 1);
    assert_eq!(owned, vec![1, 2, 3]);
    let _ = effect_count;
}

// ---------------------------------------------------------------------------
// Compile-pass guards for runtime helper signatures.
//
// These are intentionally non-`async` and never run — the point is that the
// trait/method signatures keep accepting these patterns.
// ---------------------------------------------------------------------------

#[allow(dead_code, unused_variables)]
fn _compile_pass_helper_signatures() {
    let runtime = DispatchRuntime::new(TestState::default(), reducer);

    // enqueue takes &self
    let _: () = {
        let r = &runtime;
        r.enqueue(TestAction::Tick);
    };

    // action_tx returns a Send-able UnboundedSender suitable for spawning
    let tx = runtime.action_tx();
    let _spawnable: tokio::task::JoinHandle<()> = tokio::spawn(async move {
        let _ = tx.send(TestAction::Tick);
    });

    let effect_runtime = EffectRuntime::new(TestState::default(), effect_reducer);

    // subscribe_actions takes &self and returns a broadcast::Receiver<String>
    let _rx: tokio::sync::broadcast::Receiver<String> = effect_runtime.subscribe_actions();

    // The configuration helpers are chainable.
    let _runtime = DispatchRuntime::new(TestState::default(), reducer)
        .with_event_poller(PollerConfig::default())
        .with_dispatch_error_handler(|_| DispatchErrorPolicy::Stop);

    let _effect = EffectRuntime::new(TestState::default(), effect_reducer)
        .with_event_poller(PollerConfig::default())
        .with_dispatch_error_handler(|_| DispatchErrorPolicy::Stop);
}
