#![cfg(unix)]
use ratatui::{backend::TestBackend, layout::Rect, widgets::Paragraph, Frame, Terminal};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::mpsc;
use tui_dispatch_core::{
    runtime::DebugAdapter, DefaultBindingContext, EventBus, EventKind, EventOutcome,
    EventRoutingState, HandlerResponse, Keybindings, NumericComponentId, RawEvent, ReducerResult,
    RenderContext, Runtime, Store,
};
use tui_dispatch_debug::{
    control::{ComponentTarget, DebugCommand},
    debug::{ComponentSnapshot, DebugLayer, DebugSection, DebugState},
    observe::{DebugCollector, DebugStoreExt},
    transport::{DebugClient, DebugServer},
};

#[derive(Clone, Debug, Serialize, tui_dispatch_macros::Action)]
enum Action {
    Add(usize),
    Quit,
}
#[derive(Default, Serialize)]
struct State {
    count: usize,
}
impl DebugState for State {
    fn debug_sections(&self) -> Vec<DebugSection> {
        vec![DebugSection::new("State").entry("count", self.count.to_string())]
    }
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
fn reduce(state: &mut State, action: Action) -> ReducerResult {
    if let Action::Add(amount) = action {
        state.count += amount;
    }
    ReducerResult::changed()
}

fn viewport(event: &EventKind, _: &State) -> EventOutcome<Action> {
    if matches!(event, EventKind::Resize(_, _)) {
        EventOutcome::actions([Action::Add(3)])
    } else {
        EventOutcome::ignored()
    }
}
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = PathBuf::from(format!(
            "/tmp/td-control-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn socket(&self) -> PathBuf {
        self.0.join("app.sock")
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// Observe the completed production adapter render, including its app-side border.
struct Probe {
    layer: DebugLayer<Action>,
    frames: mpsc::UnboundedSender<(usize, bool)>,
}
impl DebugAdapter<State, Action> for Probe {
    fn render(
        &mut self,
        frame: &mut Frame,
        state: &State,
        ctx: RenderContext,
        render: &mut dyn FnMut(&mut Frame, Rect, &State, RenderContext),
    ) {
        DebugAdapter::render(&mut self.layer, frame, state, ctx, render);
        let highlighted = frame.buffer_mut()[(3, 2)].symbol() == "┌";
        let _ = self.frames.send((state.count, highlighted));
    }
    fn handle_event(
        &mut self,
        event: &EventKind,
        state: &State,
        tx: &mpsc::UnboundedSender<Action>,
    ) -> Option<bool> {
        DebugAdapter::handle_event(&mut self.layer, event, state, tx)
    }
    fn log_action(&mut self, action: &Action) {
        self.layer.log_action(action);
    }
    fn is_enabled(&self) -> bool {
        DebugAdapter::<State, Action>::is_enabled(&self.layer)
    }
    fn control_notify(&self) -> Option<Arc<tokio::sync::Notify>> {
        DebugAdapter::<State, Action>::control_notify(&self.layer)
    }
    fn process_controls(&mut self, tx: &mpsc::UnboundedSender<Action>) -> bool {
        DebugAdapter::<State, Action>::process_controls(&mut self.layer, tx)
    }
    fn processing_paused(&self) -> bool {
        DebugAdapter::<State, Action>::processing_paused(&self.layer)
    }
    #[cfg(feature = "tasks")]
    fn with_task_manager(mut self, tasks: &tui_dispatch_core::TaskManager<Action>) -> Self {
        self.layer = self.layer.with_task_manager(tasks);
        self
    }
    #[cfg(feature = "subscriptions")]
    fn with_subscriptions(mut self, subs: &tui_dispatch_core::Subscriptions<Action>) -> Self {
        self.layer = self.layer.with_subscriptions(subs);
        self
    }
}

async fn confirmed_pause(client: &mut DebugClient, paused: bool) {
    loop {
        if client.snapshot().await.unwrap().control.unwrap().paused == paused {
            return;
        }
        tokio::task::yield_now().await;
    }
}
async fn frame_with(frames: &mut mpsc::UnboundedReceiver<(usize, bool)>, highlighted: bool) {
    loop {
        if let Some((count, border)) = frames.recv().await {
            if border == highlighted {
                assert_eq!(count, 0);
                return;
            }
        }
    }
}

#[tokio::test]
async fn pause_queue_resume_and_app_highlight_work_in_all_four_runtime_loops() {
    tokio::time::timeout(Duration::from_secs(8), async {
        for mode in 0..4 {
            let directory = Directory::new();
            let collector = DebugCollector::new(serde_json::json!({"count":0}), 16);
            collector.set_components(vec![ComponentSnapshot {
                raw_id: 7,
                type_name: "Player".into(),
                type_name_full: "game::Player".into(),
                bound_id: Some("player".into()),
                last_area: Some(Rect::new(3, 2, 8, 4)),
                debug_entries: vec![],
            }]);
            let (frames_tx, mut frames) = mpsc::unbounded_channel();
            let (source_tx, source_rx) = tokio::sync::oneshot::channel();
            let mut source_tx = Some(source_tx);
            let store = Store::new(State::default(), reduce).with_observer(collector.observer());
            let mut runtime = Runtime::from_store(store)
                .with_debug(Probe {
                    layer: DebugLayer::simple()
                        .active(false)
                        .with_remote_debug(collector.clone()),
                    frames: frames_tx,
                })
                .with_event_source(move |sender, _| {
                    source_tx.take().unwrap().send(sender).unwrap();
                    tokio::spawn(std::future::pending())
                });
            #[cfg(feature = "tasks")]
            let (complete_tx, complete_rx) = tokio::sync::oneshot::channel();
            #[cfg(feature = "tasks")]
            let (finished_tx, finished_rx) = tokio::sync::oneshot::channel();
            #[cfg(feature = "tasks")]
            runtime.tasks().spawn("completion", async move {
                complete_rx.await.unwrap();
                let _ = finished_tx.send(());
                Action::Add(10)
            });
            let tx = runtime.action_tx();
            let server = DebugServer::bind(directory.socket(), collector.clone())
                .await
                .unwrap();
            let inspect = async {
                let mut client = DebugClient::connect(directory.socket()).await.unwrap();
                client.command(DebugCommand::SetPaused(true)).await.unwrap();
                confirmed_pause(&mut client, true).await;
                while frames.try_recv().is_ok() {}
                tx.send(Action::Add(2)).unwrap();
                source_rx
                    .await
                    .unwrap()
                    .send(RawEvent::Resize(40, 12))
                    .unwrap();
                frame_with(&mut frames, false).await; // Resize redraws, but its viewport action stays queued.
                #[cfg(feature = "tasks")]
                {
                    complete_tx.send(()).unwrap();
                    finished_rx.await.unwrap();
                }
                for _ in 0..16 {
                    tokio::task::yield_now().await;
                }
                assert!(
                    collector.records().is_empty(),
                    "actions must remain queued while paused (mode {mode})"
                );
                let target = ComponentTarget {
                    raw_id: 7,
                    bound_id: Some("player".into()),
                };
                client
                    .command(DebugCommand::Highlight(Some(target.clone())))
                    .await
                    .unwrap();
                frame_with(&mut frames, true).await; // A socket request redraws even with no app actions.
                client.command(DebugCommand::Highlight(None)).await.unwrap();
                frame_with(&mut frames, false).await;
                client
                    .command(DebugCommand::Highlight(Some(target)))
                    .await
                    .unwrap();
                frame_with(&mut frames, true).await;
                let spectator = DebugClient::connect(directory.socket()).await.unwrap();
                drop(spectator);
                assert!(client.snapshot().await.unwrap().control.unwrap().paused);
                let mut observer = DebugClient::connect(directory.socket()).await.unwrap();
                drop(client); // Release both pause and highlight; spectator disconnect did neither.
                confirmed_pause(&mut observer, false).await;
                let expected = if cfg!(feature = "tasks") { 15 } else { 5 };
                loop {
                    let snapshot = observer.snapshot().await.unwrap();
                    if snapshot.state["count"] == expected {
                        assert!(snapshot.control.unwrap().highlight.is_none());
                        break;
                    }
                    tokio::task::yield_now().await;
                }
                assert_eq!(
                    collector.records().len(),
                    if cfg!(feature = "tasks") { 3 } else { 2 }
                );
                tx.send(Action::Quit).unwrap();
            };
            let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
            let render = |frame: &mut Frame, area, _: &State, _| {
                frame.render_widget(Paragraph::new("GAME"), area)
            };
            let result = match mode {
                0 => {
                    tokio::join!(
                        runtime.run(&mut terminal, render, viewport, |action| matches!(
                            action,
                            Action::Quit
                        )),
                        inspect
                    )
                    .0
                }
                1 => {
                    tokio::join!(
                        runtime.run_with_effects(
                            &mut terminal,
                            render,
                            viewport,
                            |action| matches!(action, Action::Quit),
                            |effect, _| match effect {}
                        ),
                        inspect
                    )
                    .0
                }
                _ => {
                    let mut bus =
                        EventBus::<State, Action, NumericComponentId, DefaultBindingContext>::new();
                    bus.register_global(|event, _| {
                        if matches!(event.kind, EventKind::Resize(_, _)) {
                            HandlerResponse::actions([Action::Add(3)])
                        } else {
                            HandlerResponse::ignored()
                        }
                    });
                    let mut runtime = runtime.with_event_bus(bus, Keybindings::new());
                    if mode == 2 {
                        tokio::join!(
                            runtime.run(
                                &mut terminal,
                                |frame, area, state, ctx, _| render(frame, area, state, ctx),
                                |action| matches!(action, Action::Quit)
                            ),
                            inspect
                        )
                        .0
                    } else {
                        tokio::join!(
                            runtime.run_with_effects(
                                &mut terminal,
                                |frame, area, state, ctx, _| render(frame, area, state, ctx),
                                |action| matches!(action, Action::Quit),
                                |effect, _| match effect {}
                            ),
                            inspect
                        )
                        .0
                    }
                }
            };
            result.unwrap();
            assert_ne!(terminal.backend().buffer()[(3, 2)].symbol(), "┌");
            server.shutdown().await.unwrap();
        }
    })
    .await
    .expect("remote controls must wake paused/idle runtimes");
}

#[tokio::test]
async fn read_only_collectors_reject_commands_without_breaking_snapshots() {
    let directory = Directory::new();
    let collector = DebugCollector::new(serde_json::json!(0), 8);
    let server = DebugServer::bind(directory.socket(), collector)
        .await
        .unwrap();
    let mut client = DebugClient::connect(directory.socket()).await.unwrap();
    assert_eq!(
        client
            .command(DebugCommand::TogglePause)
            .await
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::Unsupported
    );
    assert_eq!(client.snapshot().await.unwrap().state, serde_json::json!(0));
    server.shutdown().await.unwrap();
}
