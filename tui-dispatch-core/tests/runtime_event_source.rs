//! Injectable event sources use every production loop and share its cleanup guard.
use ratatui::{backend::CrosstermBackend, layout::Rect, Terminal, TerminalOptions, Viewport};
use std::{
    future::{poll_fn, Future},
    io::{self, Write},
    task::Poll,
    time::Duration,
};
use tui_dispatch_core::{
    testing::key, Action, DefaultBindingContext, EventBus, EventOutcome, EventRoutingState,
    HandlerResponse, Keybindings, NumericComponentId, RawEvent, ReducerResult, Runtime,
};

#[derive(Clone, Debug)]
enum TestAction {
    Increment,
    Quit,
}
impl Action for TestAction {
    fn name(&self) -> &'static str {
        "test"
    }
}
#[derive(Default)]
struct State {
    count: usize,
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
fn reduce(state: &mut State, _: TestAction) -> ReducerResult {
    state.count += 1;
    ReducerResult::changed()
}

struct Writer {
    fail: bool,
}
impl Write for Writer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.fail {
            Err(io::ErrorKind::BrokenPipe.into())
        } else {
            Ok(bytes.len())
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

// The macro varies only the production loop; all assertions are identical.
macro_rules! source_case {
    ($name:ident, $route:expr, $run:expr) => {
        #[tokio::test]
        async fn $name() {
            for exit in ["quit", "cancel", "render_error"] {
                let (probe_tx, mut probe_rx) = tokio::sync::mpsc::unbounded_channel();
                let (drop_tx, mut drop_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
                let runtime =
                    Runtime::new(State::default(), reduce).with_event_source(move |tx, token| {
                        let drop_tx = drop_tx.clone();
                        let handle = tokio::spawn(async move {
                            // Captured channel ownership proves the future is destroyed.
                            let _owned = drop_tx;
                            tx.send(RawEvent::Key(key("enter"))).unwrap();
                            std::future::pending::<()>().await;
                        });
                        probe_tx.send((token, handle.abort_handle())).unwrap();
                        handle
                    });
                let mut runtime = ($route)(runtime);
                let mut terminal = Terminal::with_options(
                    CrosstermBackend::new(Writer {
                        fail: exit == "render_error",
                    }),
                    TerminalOptions {
                        viewport: Viewport::Fixed(Rect::new(0, 0, 10, 2)),
                    },
                )
                .unwrap();
                {
                    let mut running = Box::pin(($run)(&mut runtime, &mut terminal));
                    if exit == "cancel" {
                        poll_fn(|cx| {
                            assert!(running.as_mut().poll(cx).is_pending());
                            Poll::Ready(())
                        })
                        .await;
                        // Start the source before cancelling the runtime future.
                        tokio::task::yield_now().await;
                    } else {
                        let result = tokio::time::timeout(Duration::from_secs(1), running.as_mut())
                            .await
                            .unwrap();
                        assert_eq!(result.is_err(), exit == "render_error");
                    }
                }
                let (token, handle) = probe_rx.recv().await.unwrap();
                assert!(token.is_cancelled());
                tokio::time::timeout(Duration::from_secs(1), async {
                    while !handle.is_finished() {
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .expect("source task must finish while runtime remains alive");
                // Factory still owns one sender. Dropping it after checking the task
                // finishes proves both task and factory resources are released.
                if exit == "quit" {
                    assert_eq!(runtime.state().count, 1);
                }
                drop(runtime);
                assert_eq!(drop_rx.recv().await, None);
            }
        }
    };
}

macro_rules! run_direct {
    ($runtime:expr, $terminal:expr) => {
        $runtime.run(
            $terminal,
            |_, _, _, _| {},
            |_, _| EventOutcome::actions([TestAction::Increment, TestAction::Quit]),
            |a| matches!(a, TestAction::Quit),
        )
    };
}
macro_rules! run_direct_effects {
    ($runtime:expr, $terminal:expr) => {
        $runtime.run_with_effects(
            $terminal,
            |_, _, _, _| {},
            |_, _| EventOutcome::actions([TestAction::Increment, TestAction::Quit]),
            |a| matches!(a, TestAction::Quit),
            |_, _| {},
        )
    };
}

// Named functions let Rust express the borrowed run future without requiring
// closure return lifetimes in the macro invocations.
async fn direct(
    runtime: &mut Runtime<State, TestAction>,
    terminal: &mut Terminal<CrosstermBackend<Writer>>,
) -> io::Result<()> {
    run_direct!(runtime, terminal).await
}
async fn direct_effects(
    runtime: &mut Runtime<State, TestAction>,
    terminal: &mut Terminal<CrosstermBackend<Writer>>,
) -> io::Result<()> {
    run_direct_effects!(runtime, terminal).await
}
type Routed = Runtime<
    State,
    TestAction,
    tui_dispatch_core::NoEffect,
    tui_dispatch_core::runtime::EventBusRouting<
        State,
        TestAction,
        NumericComponentId,
        DefaultBindingContext,
    >,
>;
fn route(runtime: Runtime<State, TestAction>) -> Routed {
    let mut bus = EventBus::new();
    bus.register_global(|_, _| HandlerResponse::actions([TestAction::Increment, TestAction::Quit]));
    runtime.with_event_bus(bus, Keybindings::new())
}
async fn routed(
    runtime: &mut Routed,
    terminal: &mut Terminal<CrosstermBackend<Writer>>,
) -> io::Result<()> {
    runtime
        .run(
            terminal,
            |_, _, _, _, _| {},
            |a| matches!(a, TestAction::Quit),
        )
        .await
}
async fn routed_effects(
    runtime: &mut Routed,
    terminal: &mut Terminal<CrosstermBackend<Writer>>,
) -> io::Result<()> {
    runtime
        .run_with_effects(
            terminal,
            |_, _, _, _, _| {},
            |a| matches!(a, TestAction::Quit),
            |_, _| {},
        )
        .await
}
source_case!(direct_source_lifecycle, std::convert::identity, direct);
source_case!(
    direct_effect_source_lifecycle,
    std::convert::identity,
    direct_effects
);
source_case!(routed_source_lifecycle, route, routed);
source_case!(routed_effect_source_lifecycle, route, routed_effects);
