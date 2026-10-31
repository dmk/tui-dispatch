//! Regression coverage for roadmap item 03.
#![cfg(feature = "testing-time")]

#[cfg(any(feature = "tasks", feature = "subscriptions"))]
use std::time::Duration;
use std::{cell::RefCell, rc::Rc};
use tui_dispatch_core::{
    testing::{buffer_to_string_plain, RuntimeTestHarness},
    Action, EventKind, EventOutcome, Middleware, RawEvent, ReducerResult, Runtime,
    StoreWithMiddleware,
};

#[derive(Clone, Debug, PartialEq, Eq)]
enum TestAction {
    #[allow(dead_code)] // Constructed by task-enabled tests.
    Fetch,
    Loaded(i32),
    Quit,
}
impl Action for TestAction {
    fn name(&self) -> &'static str {
        match self {
            Self::Fetch => "Fetch",
            Self::Loaded(_) => "Loaded",
            Self::Quit => "Quit",
        }
    }
}
#[derive(Debug)]
enum Effect {
    Fetch,
}
fn reduce(state: &mut i32, action: TestAction) -> ReducerResult<Effect> {
    match action {
        TestAction::Fetch => ReducerResult::effect(Effect::Fetch),
        TestAction::Loaded(value) => {
            *state = value;
            ReducerResult::changed()
        }
        TestAction::Quit => ReducerResult::unchanged(),
    }
}

#[cfg(feature = "tasks")]
#[tokio::test(start_paused = true)]
async fn injected_key_runs_delayed_effect_at_exact_deadline() {
    let mut app = RuntimeTestHarness::from_runtime(Runtime::new(0, reduce))
        .with_event_mapper(|event, _| match event {
            EventKind::Key(_) => EventOutcome::action(TestAction::Fetch),
            _ => EventOutcome::ignored(),
        })
        .with_effect_handler(|Effect::Fetch, ctx| {
            ctx.tasks()
                .debounce("fetch", Duration::from_millis(200), async {
                    TestAction::Loaded(42)
                });
        });
    let start = tokio::time::Instant::now();
    app.press("enter");
    app.run_until_idle().await;
    assert_eq!(tokio::time::Instant::now(), start);
    assert_eq!(*app.state(), 0);
    app.advance(Duration::from_millis(199)).await;
    assert_eq!(*app.state(), 0);
    app.advance(Duration::from_millis(1)).await;
    assert_eq!(*app.state(), 42);
    assert_eq!(
        app.take_actions(),
        [TestAction::Fetch, TestAction::Loaded(42)]
    );
    assert_eq!(app.active_task_count(), 0);
}

#[cfg(feature = "tasks")]
#[tokio::test(start_paused = true)]
async fn settles_self_waking_tasks_beyond_a_fixed_yield_count() {
    let mut app = RuntimeTestHarness::from_runtime(Runtime::new(0, reduce)).with_effect_handler(
        |Effect::Fetch, ctx| {
            ctx.tasks().spawn("fetch", async {
                for _ in 0..200 {
                    tokio::task::yield_now().await;
                }
                TestAction::Loaded(42)
            });
        },
    );
    app.enqueue(TestAction::Fetch);
    app.run_until_idle().await;
    assert_eq!(*app.state(), 42);
    assert_eq!(app.active_task_count(), 0);
}

#[cfg(feature = "tasks")]
#[tokio::test(start_paused = true)]
async fn pending_io_and_paused_completions_are_idle_and_drop_cancels_work() {
    let mut runtime = Runtime::new(0, reduce);
    let (mut tx, rx) = tokio::sync::oneshot::channel();
    // Work registered before harness construction is tracked too.
    runtime
        .tasks()
        .spawn("io", async move { rx.await.unwrap() });
    let pause = runtime.tasks().pause_handle();
    pause.pause();
    runtime
        .tasks()
        .spawn("ready", async { TestAction::Loaded(42) });
    let mut app = RuntimeTestHarness::from_runtime(runtime);
    let start = tokio::time::Instant::now();
    app.run_until_idle().await;
    assert_eq!(tokio::time::Instant::now(), start);
    assert_eq!(*app.state(), 0);
    assert_eq!(app.active_task_count(), 1);
    for action in pause.resume() {
        app.enqueue(action);
    }
    app.run_until_idle().await;
    assert_eq!(*app.state(), 42);
    drop(app);
    tx.closed().await;
}

#[cfg(feature = "tasks")]
#[tokio::test(start_paused = true)]
async fn debounce_replacement_cancels_obsolete_completion() {
    let mut app = RuntimeTestHarness::from_runtime(Runtime::new(0, reduce));
    app.runtime_mut()
        .tasks()
        .debounce("fetch", Duration::from_millis(200), async {
            TestAction::Loaded(1)
        });
    app.advance(Duration::from_millis(100)).await;
    app.runtime_mut()
        .tasks()
        .debounce("fetch", Duration::from_millis(200), async {
            TestAction::Loaded(2)
        });
    app.advance(Duration::from_millis(100)).await;
    assert_eq!(*app.state(), 0);
    app.advance(Duration::from_millis(100)).await;
    assert_eq!(app.take_actions(), [TestAction::Loaded(2)]);
}

#[cfg(feature = "subscriptions")]
#[tokio::test(start_paused = true)]
async fn sleeping_and_paused_subscriptions_do_not_prevent_idle() {
    let mut app = RuntimeTestHarness::from_runtime(Runtime::new(0, reduce));
    app.runtime_mut()
        .subscriptions()
        .interval("tick", Duration::from_millis(10), || TestAction::Loaded(7));
    app.run_until_idle().await;
    assert!(app.take_actions().is_empty());
    app.advance(Duration::from_millis(10)).await;
    assert_eq!(app.take_actions(), [TestAction::Loaded(7)]);
    app.runtime_mut().subscriptions().pause();
    app.advance(Duration::from_millis(10)).await;
    assert!(app.take_actions().is_empty());
}

#[tokio::test(start_paused = true)]
async fn renders_changes_and_resize_through_runtime_pipeline() {
    let frames = Rc::new(RefCell::new(Vec::new()));
    let capture = frames.clone();
    let mut app = RuntimeTestHarness::from_runtime(Runtime::new(0, reduce))
        .with_size(10, 2)
        .with_event_mapper(|event, state| match event {
            EventKind::Key(_) => EventOutcome::action(TestAction::Loaded(state + 1)),
            EventKind::Resize(..) => EventOutcome::needs_render(),
            _ => EventOutcome::ignored(),
        })
        .with_render(move |frame, area, state, _| {
            capture.borrow_mut().push((area.width, *state));
            frame.render_widget(ratatui::widgets::Paragraph::new(state.to_string()), area);
        });
    app.press("enter");
    app.press("enter");
    app.inject(RawEvent::Resize(12, 3));
    app.run_until_idle().await;
    assert_eq!(*frames.borrow(), [(10, 0), (10, 1), (10, 2), (12, 2)]);
    assert_eq!(app.buffer().area.width, 12);
    assert!(buffer_to_string_plain(app.buffer()).starts_with('2'));
}

struct Inject;
impl Middleware<i32, TestAction> for Inject {
    fn before(&mut self, action: &TestAction, _: &i32) -> bool {
        !matches!(action, TestAction::Loaded(9))
    }
    fn after(&mut self, action: &TestAction, _: bool, _: &i32) -> Vec<TestAction> {
        if matches!(action, TestAction::Loaded(1)) {
            vec![TestAction::Loaded(2)]
        } else {
            vec![]
        }
    }
}

#[tokio::test(start_paused = true)]
async fn preserves_middleware_and_quit_semantics() {
    let store = StoreWithMiddleware::new(0, reduce, Inject);
    let mut app = RuntimeTestHarness::from_runtime(Runtime::from_store(store))
        .with_quit_predicate(|a| matches!(a, TestAction::Quit));
    app.enqueue(TestAction::Loaded(1));
    app.enqueue(TestAction::Loaded(9));
    app.enqueue(TestAction::Quit);
    app.enqueue(TestAction::Loaded(8));
    app.run_until_idle().await;
    assert_eq!(*app.state(), 2);
    assert!(app.is_stopped());
    assert_eq!(
        app.take_actions(),
        [
            TestAction::Loaded(1),
            TestAction::Loaded(9),
            TestAction::Quit
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn honors_dispatch_error_policy_from_the_owned_runtime() {
    use tui_dispatch_core::{DispatchErrorPolicy, DispatchLimits};
    for policy in [
        DispatchErrorPolicy::Continue,
        DispatchErrorPolicy::Render,
        DispatchErrorPolicy::Stop,
    ] {
        let errors = Rc::new(RefCell::new(0));
        let capture = errors.clone();
        let store =
            StoreWithMiddleware::new(0, reduce, Inject).with_dispatch_limits(DispatchLimits {
                max_depth: 10,
                max_actions: 1,
            });
        let runtime = Runtime::from_store(store).with_dispatch_error_handler(move |_| {
            *capture.borrow_mut() += 1;
            policy
        });
        let mut app = RuntimeTestHarness::from_runtime(runtime);
        app.enqueue(TestAction::Loaded(1));
        app.enqueue(TestAction::Loaded(3));
        app.run_until_idle().await;
        assert_eq!(*errors.borrow(), 1);
        let stopped = policy == DispatchErrorPolicy::Stop;
        assert_eq!(app.is_stopped(), stopped);
        assert_eq!(*app.state(), if stopped { 1 } else { 3 });
    }
}
