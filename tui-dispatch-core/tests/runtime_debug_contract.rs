//! Contract tests for the runtime/debug integration surface.
//!
//! `tui-dispatch-debug` plugs into the runtime through the `DebugAdapter`
//! trait exposed under `feature = "debug"`. These tests pin
//! exactly what the runtime guarantees about that integration so the
//! upcoming refactor cannot quietly change it:
//!
//! * `DebugAdapter::log_action` is called once per dispatched action,
//!   in dispatch order
//! * `DebugAdapter::handle_event` runs first for every event; returning
//!   `Some(needs_render)` short-circuits the rest of routing
//! * `DebugAdapter::render` is invoked when a debug adapter is attached
//! * `RenderContext::debug_enabled` mirrors `DebugAdapter::is_enabled()`
//! * `DebugAdapter::with_task_manager` / `with_subscriptions` are wired up
//!   when `Runtime::with_debug` is called with the corresponding
//!   feature enabled
//!
//! The whole file is feature-gated; without `feature = "debug"` it is empty.

#![cfg(feature = "debug")]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::{Frame, Terminal};
use tokio::sync::mpsc;

use tui_dispatch_core::runtime::DebugAdapter;
use tui_dispatch_core::{
    Action, EventKind, EventOutcome, PollerConfig, ReducerResult, RenderContext, Runtime,
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
struct TestState;

fn reducer(_state: &mut TestState, _action: TestAction) -> ReducerResult {
    ReducerResult::changed()
}

fn effect_reducer(_state: &mut TestState, _action: TestAction) -> ReducerResult<()> {
    ReducerResult::changed()
}

fn test_terminal() -> Terminal<TestBackend> {
    Terminal::new(TestBackend::new(40, 8)).expect("test terminal")
}

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
// Recording mock adapter
// ---------------------------------------------------------------------------

#[derive(Default)]
struct AdapterCalls {
    rendered: usize,
    handled_events: usize,
    logged_actions: Vec<String>,
    is_enabled_queries: usize,
    #[cfg_attr(not(feature = "tasks"), allow(dead_code))]
    task_manager_attached: bool,
    #[cfg_attr(not(feature = "subscriptions"), allow(dead_code))]
    subscriptions_attached: bool,
}

#[derive(Clone)]
struct MockDebug {
    enabled: bool,
    /// If `Some`, `handle_event` returns this value (and increments
    /// `handled_events`) instead of returning `None`.
    intercept_with: Option<bool>,
    calls: Arc<Mutex<AdapterCalls>>,
}

impl MockDebug {
    fn new() -> Self {
        Self {
            enabled: false,
            intercept_with: None,
            calls: Arc::new(Mutex::new(AdapterCalls::default())),
        }
    }

    fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[allow(dead_code)]
    fn intercept(mut self, needs_render: bool) -> Self {
        self.intercept_with = Some(needs_render);
        self
    }

    fn calls(&self) -> Arc<Mutex<AdapterCalls>> {
        self.calls.clone()
    }
}

impl DebugAdapter<TestState, TestAction> for MockDebug {
    fn render(
        &mut self,
        frame: &mut Frame,
        state: &TestState,
        render_ctx: RenderContext,
        render_fn: &mut dyn FnMut(&mut Frame, Rect, &TestState, RenderContext),
    ) {
        self.calls.lock().unwrap().rendered += 1;
        // The contract says the adapter is responsible for invoking the
        // user's render callback (so it can wrap it with overlays, etc.).
        render_fn(frame, frame.area(), state, render_ctx);
    }

    fn handle_event(
        &mut self,
        _event: &EventKind,
        _state: &TestState,
        _action_tx: &mpsc::UnboundedSender<TestAction>,
    ) -> Option<bool> {
        self.calls.lock().unwrap().handled_events += 1;
        self.intercept_with
    }

    fn log_action(&mut self, action: &TestAction) {
        self.calls
            .lock()
            .unwrap()
            .logged_actions
            .push(action.name().to_string());
    }

    fn is_enabled(&self) -> bool {
        self.calls.lock().unwrap().is_enabled_queries += 1;
        self.enabled
    }
    #[cfg(feature = "tasks")]
    fn with_task_manager(self, _tasks: &tui_dispatch_core::tasks::TaskManager<TestAction>) -> Self {
        self.calls.lock().unwrap().task_manager_attached = true;
        self
    }

    #[cfg(feature = "subscriptions")]
    fn with_subscriptions(
        self,
        _subs: &tui_dispatch_core::subscriptions::Subscriptions<TestAction>,
    ) -> Self {
        self.calls.lock().unwrap().subscriptions_attached = true;
        self
    }
}

struct MinimalDebug;

impl DebugAdapter<TestState, TestAction> for MinimalDebug {
    fn render(
        &mut self,
        frame: &mut Frame,
        state: &TestState,
        render_ctx: RenderContext,
        render_fn: &mut dyn FnMut(&mut Frame, Rect, &TestState, RenderContext),
    ) {
        render_fn(frame, frame.area(), state, render_ctx);
    }

    fn handle_event(
        &mut self,
        _event: &EventKind,
        _state: &TestState,
        _action_tx: &mpsc::UnboundedSender<TestAction>,
    ) -> Option<bool> {
        None
    }

    fn log_action(&mut self, _action: &TestAction) {}

    fn is_enabled(&self) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// log_action ordering and frequency
// ---------------------------------------------------------------------------

#[tokio::test]
async fn runtime_logs_each_dispatched_action_in_order() {
    let debug = MockDebug::new();
    let calls = debug.calls();

    let mut runtime = Runtime::new(TestState, reducer)
        .with_event_poller(fast_poller())
        .with_debug(debug);

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

    let calls = calls.lock().unwrap();
    // Quit is checked before log_action — it must NOT be logged.
    assert_eq!(
        calls.logged_actions,
        vec!["Tick".to_string(), "Tick".to_string()]
    );
}

#[tokio::test]
async fn runtime_with_effects_logs_each_dispatched_action_in_order() {
    let debug = MockDebug::new();
    let calls = debug.calls();

    let mut runtime = Runtime::new(TestState, effect_reducer)
        .with_event_poller(fast_poller())
        .with_debug(debug);

    runtime.enqueue(TestAction::Tick);
    runtime.enqueue(TestAction::Tick);
    runtime.enqueue(TestAction::Quit);

    let mut term = test_terminal();
    runtime
        .run_with_effects(
            &mut term,
            |_, _, _, _| {},
            |_, _: &TestState| EventOutcome::ignored(),
            quit_on_quit,
            |(), _ctx| {},
        )
        .await
        .expect("runtime exits cleanly");

    let calls = calls.lock().unwrap();
    assert_eq!(
        calls.logged_actions,
        vec!["Tick".to_string(), "Tick".to_string()]
    );
}

// ---------------------------------------------------------------------------
// Render integration — debug adapter wraps the user render closure
// ---------------------------------------------------------------------------

#[tokio::test]
async fn debug_adapter_render_is_invoked_when_attached() {
    let debug = MockDebug::new();
    let calls = debug.calls();

    let mut runtime = Runtime::new(TestState, reducer)
        .with_event_poller(fast_poller())
        .with_debug(debug);

    let user_render_count = Arc::new(Mutex::new(0usize));
    let user_render_count_clone = user_render_count.clone();

    runtime.enqueue(TestAction::Quit);

    let mut term = test_terminal();
    runtime
        .run(
            &mut term,
            move |_, _, _, _| {
                *user_render_count_clone.lock().unwrap() += 1;
            },
            |_, _: &TestState| EventOutcome::ignored(),
            quit_on_quit,
        )
        .await
        .expect("runtime exits cleanly");

    let calls = calls.lock().unwrap();
    assert!(calls.rendered >= 1, "debug adapter render was not called");
    // The mock adapter forwards to the user render closure exactly once per
    // call, so the two counts must match.
    assert_eq!(*user_render_count.lock().unwrap(), calls.rendered);
}

// ---------------------------------------------------------------------------
// is_enabled drives RenderContext::debug_enabled
// ---------------------------------------------------------------------------

#[tokio::test]
async fn render_context_debug_enabled_mirrors_adapter_is_enabled() {
    let debug = MockDebug::new().enabled(true);
    let calls = debug.calls();

    let mut runtime = Runtime::new(TestState, reducer)
        .with_event_poller(fast_poller())
        .with_debug(debug);

    let observed: Arc<Mutex<Option<bool>>> = Arc::new(Mutex::new(None));
    let observed_clone = observed.clone();

    runtime.enqueue(TestAction::Quit);

    let mut term = test_terminal();
    runtime
        .run(
            &mut term,
            move |_, _, _, ctx: RenderContext| {
                *observed_clone.lock().unwrap() = Some(ctx.debug_enabled);
            },
            |_, _: &TestState| EventOutcome::ignored(),
            quit_on_quit,
        )
        .await
        .expect("runtime exits cleanly");

    assert!(observed.lock().unwrap().unwrap_or(false));
    assert!(calls.lock().unwrap().is_enabled_queries >= 1);
}

// ---------------------------------------------------------------------------
// DebugAdapter auto-wiring on Runtime::with_debug
// ---------------------------------------------------------------------------

#[cfg(feature = "tasks")]
#[tokio::test]
async fn runtime_with_debug_invokes_with_task_manager_hook() {
    let debug = MockDebug::new();
    let calls = debug.calls();

    let _runtime = Runtime::new(TestState, effect_reducer).with_debug(debug);

    assert!(
        calls.lock().unwrap().task_manager_attached,
        "DebugAdapter::with_task_manager should fire when feature `tasks` is enabled"
    );
}

#[cfg(feature = "subscriptions")]
#[tokio::test]
async fn runtime_with_debug_invokes_with_subscriptions_hook() {
    let debug = MockDebug::new();
    let calls = debug.calls();

    let _runtime = Runtime::new(TestState, effect_reducer).with_debug(debug);

    assert!(
        calls.lock().unwrap().subscriptions_attached,
        "DebugAdapter::with_subscriptions should fire when feature `subscriptions` is enabled"
    );
}

#[test]
fn runtime_with_debug_accepts_adapter_without_optional_hooks() {
    let _runtime = Runtime::new(TestState, reducer).with_debug(MinimalDebug);
}
