use std::{collections::VecDeque, time::Duration};

use ratatui::{backend::TestBackend, buffer::Buffer, layout::Rect, Frame, Terminal};
use tokio_util::sync::CancellationToken;

use crate::{
    runtime::{draw_frame, Direct},
    Action, EffectContext, EventKind, EventOutcome, NoEffect, RawEvent, RenderContext, Runtime,
    RuntimeStore, Store,
};

type EventMapper<S, A> = dyn FnMut(&EventKind, &S) -> EventOutcome<A>;
type EffectHandler<A, E> = dyn FnMut(E, &mut EffectContext<A>);
type Renderer<S> = dyn FnMut(&mut Frame, Rect, &S, RenderContext);

/// Drive a direct-routing runtime without a terminal or a background event poller.
///
/// Uses the runtime's event normalization, debug interception, store/middleware
/// dispatch, effect handling, error policy, and rendering. Queued actions are
/// processed before the next injected event, making scripted input deterministic.
/// `take_actions` records dequeued root actions (including quit/cancelled actions),
/// not middleware-injected actions.
///
/// Requires `testing-time` and a current-thread Tokio runtime. Use
/// `#[tokio::test(start_paused = true)]` for virtual time. Only work spawned through
/// the runtime's task/subscription managers is tracked; externally spawned work
/// must be synchronized by the test before settling the harness.
///
/// Pending I/O, sleeping tasks, and paused completion queues count as idle.
/// A perpetually runnable task or action cycle fails after a bounded number of
/// steps instead of hanging the test. Dropping/stopping the harness cancels work.
pub struct RuntimeTestHarness<S: 'static, A: Action, E = NoEffect, St = Store<S, A, E>>
where
    St: RuntimeStore<S, A, E>,
{
    runtime: Runtime<S, A, E, Direct, St>,
    events: VecDeque<RawEvent>,
    actions: Vec<A>,
    map_event: Box<EventMapper<S, A>>,
    handle_effect: Box<EffectHandler<A, E>>,
    render: Box<Renderer<S>>,
    should_quit: Box<dyn FnMut(&A) -> bool>,
    terminal: Terminal<TestBackend>,
    stopped: bool,
}

impl<S: 'static, A: Action, E, St: RuntimeStore<S, A, E>> RuntimeTestHarness<S, A, E, St> {
    /// Own an existing runtime, preserving its store, middleware and async managers.
    /// Any configured event source is replaced by the harness's injected events.
    pub fn from_runtime(runtime: Runtime<S, A, E, Direct, St>) -> Self {
        Self {
            runtime,
            events: VecDeque::new(),
            actions: Vec::new(),
            map_event: Box::new(|_, _| EventOutcome::ignored()),
            handle_effect: Box::new(|_, _| {
                panic!("configure with_effect_handler for emitted effects")
            }),
            render: Box::new(|_, _, _, _| {}),
            should_quit: Box::new(|_| false),
            terminal: Terminal::new(TestBackend::new(80, 24)).expect("test terminal"),
            stopped: false,
        }
    }

    /// Configure the same event mapper accepted by `Runtime::run_with_effects`.
    pub fn with_event_mapper<F, R>(mut self, mut mapper: F) -> Self
    where
        F: FnMut(&EventKind, &S) -> R + 'static,
        R: Into<EventOutcome<A>>,
    {
        self.map_event = Box::new(move |event, state| mapper(event, state).into());
        self
    }

    /// Handle effects using the runtime's real effect context.
    pub fn with_effect_handler(
        mut self,
        handler: impl FnMut(E, &mut EffectContext<A>) + 'static,
    ) -> Self {
        self.handle_effect = Box::new(handler);
        self
    }

    /// Render dirty frames to the in-memory terminal.
    pub fn with_render(
        mut self,
        render: impl FnMut(&mut Frame, Rect, &S, RenderContext) + 'static,
    ) -> Self {
        self.render = Box::new(render);
        self
    }

    /// Stop before dispatching actions matched by this predicate, just like the runtime.
    pub fn with_quit_predicate(mut self, predicate: impl FnMut(&A) -> bool + 'static) -> Self {
        self.should_quit = Box::new(predicate);
        self
    }

    /// Set the in-memory terminal size and request a redraw.
    pub fn with_size(mut self, width: u16, height: u16) -> Self {
        self.terminal.backend_mut().resize(width, height);
        self.terminal
            .resize(Rect::new(0, 0, width, height))
            .expect("resize test terminal");
        self.runtime.shell.should_render = true;
        self
    }

    /// Queue a raw terminal event for the production event pipeline.
    pub fn inject(&mut self, event: RawEvent) {
        assert!(!self.stopped, "runtime harness has stopped");
        self.events.push_back(event);
    }

    /// Queue a key press using the same notation as [`super::key`].
    pub fn press(&mut self, key: &str) {
        self.inject(RawEvent::Key(super::key(key)));
    }

    /// Queue an action without bypassing middleware or effects.
    pub fn enqueue(&self, action: A) {
        assert!(!self.stopped, "runtime harness has stopped");
        self.runtime.enqueue(action);
    }

    /// Access the current state.
    pub fn state(&self) -> &S {
        self.runtime.state()
    }

    /// Access the owned runtime, e.g. to pause tasks or configure subscriptions.
    pub fn runtime_mut(&mut self) -> &mut Runtime<S, A, E, Direct, St> {
        &mut self.runtime
    }

    /// Drain root actions in processing order.
    pub fn take_actions(&mut self) -> Vec<A> {
        std::mem::take(&mut self.actions)
    }

    /// Inspect the last rendered frame.
    pub fn buffer(&self) -> &Buffer {
        self.terminal.backend().buffer()
    }

    /// Whether a quit action or dispatch error policy stopped processing.
    pub fn is_stopped(&self) -> bool {
        self.stopped
    }

    /// Count unfinished tasks; completed actions waiting in a paused queue are excluded.
    #[cfg(feature = "tasks")]
    pub fn active_task_count(&self) -> usize {
        self.runtime.tasks.len()
    }

    fn has_runnable(&self) -> bool {
        #[cfg(feature = "tasks")]
        if self.runtime.tasks.spawner.has_runnable() {
            return true;
        }
        #[cfg(feature = "subscriptions")]
        if self.runtime.subscriptions.spawner.has_runnable() {
            return true;
        }
        false
    }

    /// Settle queued events/actions and runnable managed work without advancing time.
    ///
    /// Panics on rendering failure or after 10,000 steps without becoming idle.
    pub async fn run_until_idle(&mut self) {
        assert_eq!(
            tokio::runtime::Handle::current().runtime_flavor(),
            tokio::runtime::RuntimeFlavor::CurrentThread,
            "RuntimeTestHarness requires a current-thread Tokio runtime"
        );
        for _ in 0..10_000 {
            if self.stopped {
                return;
            }
            if self.runtime.shell.should_render {
                draw_frame(
                    &mut self.runtime.shell,
                    self.runtime.store.state(),
                    &mut self.terminal,
                    &mut self.render,
                )
                .expect("render test terminal");
            }
            if let Ok(action) = self.runtime.shell.action_rx.try_recv() {
                self.actions.push(action.clone());
                if (self.should_quit)(&action) {
                    self.stop();
                    return;
                }
                self.runtime.shell.debug_log_action(&action);
                if self
                    .runtime
                    .dispatch_and_handle_effects(action, &mut self.handle_effect)
                {
                    self.stop();
                    return;
                }
                continue;
            }
            if let Some(event) = self.events.pop_front() {
                if let RawEvent::Resize(width, height) = &event {
                    self.terminal.backend_mut().resize(*width, *height);
                    self.terminal
                        .resize(Rect::new(0, 0, *width, *height))
                        .expect("resize test terminal");
                }
                self.runtime.shell.process_event(
                    event,
                    self.runtime.store.state(),
                    &mut self.map_event,
                );
                continue;
            }
            // Keeping this driver runnable prevents Tokio's paused clock from
            // auto-advancing to a sleeping task's next deadline.
            tokio::task::yield_now().await;
            if self.runtime.shell.action_rx.is_empty() && !self.has_runnable() {
                return;
            }
        }
        panic!("runtime did not become idle after 10,000 steps");
    }

    /// Settle current work, advance Tokio's clock, then settle newly runnable work.
    /// The caller must pause Tokio time first.
    pub async fn advance(&mut self, duration: Duration) {
        self.run_until_idle().await;
        tokio::time::advance(duration).await;
        self.run_until_idle().await;
    }

    fn stop(&mut self) {
        self.stopped = true;
        self.runtime.cleanup(CancellationToken::new());
    }
}

impl<S: 'static, A: Action, E, St: RuntimeStore<S, A, E>> Drop for RuntimeTestHarness<S, A, E, St> {
    fn drop(&mut self) {
        self.stop();
    }
}
