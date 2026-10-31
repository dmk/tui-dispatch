//! Runtime lifecycle regressions promoted from roadmap item 01.
//! Covers actual work destruction in all four loop paths and pause boundaries.
#![cfg(all(feature = "tasks", feature = "subscriptions"))]

use ratatui::{
    backend::{CrosstermBackend, TestBackend},
    layout::Rect,
    Terminal, TerminalOptions, Viewport,
};
use std::{
    future::{poll_fn, Future},
    io::{self, Write},
    task::Poll,
    time::Duration,
};
use tui_dispatch_core::{
    DefaultBindingContext, EventBus, EventOutcome, EventRoutingState, Keybindings,
    NumericComponentId, PollerConfig, ReducerResult, Runtime, TaskManager,
};

#[derive(Clone, Debug, PartialEq, Eq)]
enum TestAction {
    Loaded(i32),
}
impl tui_dispatch_core::Action for TestAction {
    fn name(&self) -> &'static str {
        "Loaded"
    }
}
#[derive(Default)]
struct State {
    count: i32,
}
fn reduce(state: &mut State, action: TestAction) -> tui_dispatch_core::ReducerResult {
    let TestAction::Loaded(count) = action;
    state.count = count;
    tui_dispatch_core::ReducerResult::changed()
}

struct CancellationProbes {
    task: tokio::sync::oneshot::Sender<TestAction>,
    subscription: tokio::sync::mpsc::UnboundedSender<TestAction>,
}

impl CancellationProbes {
    async fn assert_cancelled(mut self) {
        tokio::time::timeout(Duration::from_secs(1), async {
            self.task.closed().await;
            self.subscription.closed().await;
        })
        .await
        .expect("cleanup must drop owned futures/streams, not merely clear registrations");
    }
}

fn runtime_with_pending_work<E>(
    reducer: fn(&mut State, TestAction) -> ReducerResult<E>,
) -> (Runtime<State, TestAction, E>, CancellationProbes) {
    let mut runtime = Runtime::new(State::default(), reducer).with_event_poller(PollerConfig {
        // These tests finish in the first poll/draw, before terminal input is read.
        poll_timeout: Duration::ZERO,
        loop_sleep: Duration::from_secs(3600),
    });
    let (task, task_rx) = tokio::sync::oneshot::channel();
    let (subscription, subscription_rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .tasks()
        .spawn("pending", async move { task_rx.await.unwrap() });
    runtime.subscriptions().stream(
        "pending",
        tokio_stream::wrappers::UnboundedReceiverStream::new(subscription_rx),
    );
    (runtime, CancellationProbes { task, subscription })
}

impl EventRoutingState<NumericComponentId, DefaultBindingContext> for State {
    fn focused(&self) -> Option<NumericComponentId> {
        None
    }
    fn modal(&self) -> Option<NumericComponentId> {
        None
    }
    fn binding_context(&self, _: NumericComponentId) -> DefaultBindingContext {
        DefaultBindingContext
    }
    fn default_context(&self) -> DefaultBindingContext {
        DefaultBindingContext
    }
}

fn effect_reduce(state: &mut State, action: TestAction) -> ReducerResult<()> {
    if reduce(state, action).changed {
        ReducerResult::changed()
    } else {
        ReducerResult::unchanged()
    }
}

struct BrokenWriter;
impl Write for BrokenWriter {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "test backend disconnected",
        ))
    }
    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "test backend disconnected",
        ))
    }
}

// Repeat the same contract across the four independently implemented loop paths.
macro_rules! lifecycle_tests {
    ($cancel:ident, $error:ident, $reducer:path,
     |$runtime:ident| $route:expr, |$terminal:ident| $run:expr) => {
        #[tokio::test]
        async fn $cancel() {
            let ($runtime, probes) = runtime_with_pending_work($reducer);
            let mut $runtime = $route;
            let mut $terminal = Terminal::new(TestBackend::new(20, 5)).unwrap();
            {
                let mut running = Box::pin($run);
                poll_fn(|cx| {
                    assert!(running.as_mut().poll(cx).is_pending());
                    Poll::Ready(())
                })
                .await;
            }
            assert_eq!(
                ($runtime.tasks().len(), $runtime.subscriptions().len()),
                (0, 0),
                "cancelling run must clean up while Runtime stays alive"
            );
            probes.assert_cancelled().await;
        }

        #[tokio::test]
        async fn $error() {
            let ($runtime, probes) = runtime_with_pending_work($reducer);
            let mut $runtime = $route;
            let mut $terminal = Terminal::with_options(
                CrosstermBackend::new(BrokenWriter),
                TerminalOptions {
                    viewport: Viewport::Fixed(Rect::new(0, 0, 20, 5)),
                },
            )
            .unwrap();
            let result = $run.await;
            assert!(result.is_err(), "fixture must actually fail drawing");
            assert_eq!(
                ($runtime.tasks().len(), $runtime.subscriptions().len()),
                (0, 0),
                "backend errors must take the same cleanup path as normal shutdown"
            );
            probes.assert_cancelled().await;
        }
    };
}

lifecycle_tests!(
    direct_drop_cancels_work,
    direct_render_error_cancels_work,
    reduce,
    |runtime| runtime,
    |terminal| runtime.run(
        &mut terminal,
        |_, _, _, _| {},
        |_, _| EventOutcome::ignored(),
        |_| false
    )
);
lifecycle_tests!(
    direct_effect_drop_cancels_work,
    direct_effect_render_error_cancels_work,
    effect_reduce,
    |runtime| runtime,
    |terminal| runtime.run_with_effects(
        &mut terminal,
        |_, _, _, _| {},
        |_, _| EventOutcome::ignored(),
        |_| false,
        |(), _| {}
    )
);
lifecycle_tests!(
    routed_drop_cancels_work,
    routed_render_error_cancels_work,
    reduce,
    |runtime| runtime.with_event_bus(
        EventBus::<_, _, NumericComponentId, DefaultBindingContext>::new(),
        Keybindings::new()
    ),
    |terminal| runtime.run(&mut terminal, |_, _, _, _, _| {}, |_| false)
);
lifecycle_tests!(
    routed_effect_drop_cancels_work,
    routed_effect_render_error_cancels_work,
    effect_reduce,
    |runtime| runtime.with_event_bus(
        EventBus::<_, _, NumericComponentId, DefaultBindingContext>::new(),
        Keybindings::new()
    ),
    |terminal| runtime.run_with_effects(&mut terminal, |_, _, _, _, _| {}, |_| false, |(), _| {})
);

#[tokio::test]
async fn completed_actions_cross_pause_boundaries_exactly_once() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut tasks = TaskManager::new(tx);
    let pause = tasks.pause_handle();
    pause.pause();
    tasks.spawn("first", async { TestAction::Loaded(1) });
    tokio::time::timeout(Duration::from_secs(1), async {
        while !tasks.is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(rx.try_recv().is_err());
    assert_eq!(pause.resume(), vec![TestAction::Loaded(1)]);
    tasks.spawn("second", async { TestAction::Loaded(2) });
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), rx.recv())
            .await
            .unwrap(),
        Some(TestAction::Loaded(2))
    );
    assert!(pause.resume().is_empty());
    assert!(rx.try_recv().is_err());
    // This sequential boundary test is not a deterministic reproduction of the
    // atomic-flag/queue race; a scheduler-controlled unit test belongs with its fix.
}
