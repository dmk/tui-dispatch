//! Standalone debugger; the app keeps ownership of its terminal and execution.

mod client {
    use std::{
        collections::VecDeque,
        io,
        path::PathBuf,
        sync::Arc,
        time::{Duration, Instant},
    };

    use clap::Parser;
    use crossterm::{
        event::{self, Event},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    };
    use ratatui::{backend::CrosstermBackend, Terminal};
    use tokio::sync::watch;
    use tui_dispatch_core::{process_raw_event, RawEvent};
    use tui_dispatch_debug::{
        control::{ComponentTarget, DebugCommand},
        debug::{ActionLogDisplayEntry, DebugInspector},
        recording::RecordingReader,
        records::{DebugSnapshot, SnapshotOutcome, SnapshotRecord},
    };

    #[cfg(unix)]
    use tui_dispatch_debug::transport::DebugClient;

    #[derive(Parser)]
    #[command(about = "Inspect a running tui-dispatch app or a recording")]
    struct Args {
        /// Unix socket path exposed by the app
        #[arg(required_unless_present = "recording", conflicts_with = "recording")]
        socket: Option<PathBuf>,
        /// Open a .tuidbg recording instead of a socket
        #[arg(long)]
        recording: Option<PathBuf>,
        /// Follow the recording until its writer finishes (or quit with q)
        #[arg(long, requires = "recording", conflicts_with = "once")]
        follow: bool,
        /// Maximum recording records retained by the inspector
        #[arg(long, default_value_t = 1024, value_parser = clap::value_parser!(u32).range(1..), requires = "recording")]
        history_limit: u32,
        /// Print one JSON snapshot and exit without opening a terminal UI
        #[arg(long)]
        once: bool,
    }

    struct TerminalGuard;
    impl Drop for TerminalGuard {
        fn drop(&mut self) {
            let _ = disable_raw_mode();
            let _ = execute!(io::stdout(), LeaveAlternateScreen, crossterm::cursor::Show);
        }
    }

    fn action_entries(snapshot: &DebugSnapshot) -> serde_json::Result<Vec<ActionLogDisplayEntry>> {
        snapshot
            .history
            .iter()
            .rev()
            .map(|record| {
                let outcome = match record.outcome {
                    SnapshotOutcome::Applied => "applied",
                    SnapshotOutcome::Cancelled => "cancelled",
                    _ => "rejected",
                };
                Ok(ActionLogDisplayEntry {
                    sequence: record.sequence,
                    name: record.name.clone(),
                    params: format!("[{outcome}] {}", record.action),
                    params_detail: serde_json::to_string_pretty(&serde_json::json!({
                        "action": record.action,
                        "outcome": record.outcome,
                        "changed": record.changed,
                        "parent_action_id": record.parent_action_id,
                        "serialization_errors": record.serialization_errors,
                    }))?,
                    // The current wire format has no dispatch timestamps.
                    elapsed: "—".into(),
                })
            })
            .collect()
    }

    type Update = Option<Result<(Arc<DebugSnapshot>, String), String>>;

    #[derive(Clone, Default)]
    struct ControlIntent {
        pause_parity: bool,
        highlight: Option<ComponentTarget>,
    }

    #[cfg(unix)]
    async fn live_connection(
        client: &mut DebugClient,
        sender: &watch::Sender<Update>,
        controls: &mut watch::Receiver<ControlIntent>,
    ) -> io::Result<()> {
        let mut previous = controls.borrow_and_update().clone();
        let mut available = false;
        let mut poll = tokio::time::interval(Duration::from_millis(250));
        poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            let mut command_error = None;
            tokio::select! {
                _ = poll.tick() => {},
                changed = controls.changed() => {
                    if changed.is_err() { return Ok(()); }
                    let current = controls.borrow_and_update().clone();
                    if available {
                        let mut requests = Vec::new();
                        if current.pause_parity != previous.pause_parity { requests.push(DebugCommand::TogglePause); }
                        if current.highlight != previous.highlight { requests.push(DebugCommand::Highlight(current.highlight.clone())); }
                        for request in requests {
                            if let Err(error) = client.command(request).await {
                                if error.kind() == io::ErrorKind::Unsupported { command_error = Some(error.to_string()); }
                                else { return Err(error); }
                            }
                        }
                    }
                    previous = current;
                }
            }
            // Never cancel an in-flight socket request to process UI input: the
            // watch slot coalesces new selection/toggle intentions until it ends.
            let snapshot = client.snapshot().await?;
            let was_available = available;
            available = snapshot.control.is_some();
            if available && !was_available {
                previous = controls.borrow_and_update().clone();
                if previous.highlight.is_some() {
                    let _ = client
                        .command(DebugCommand::Highlight(previous.highlight.clone()))
                        .await;
                }
            }
            let status = command_error
                .map(|error| format!("Control: {error}"))
                .unwrap_or_else(|| "Connected".into());
            if sender.send(Some(Ok((Arc::new(snapshot), status)))).is_err() {
                return Ok(());
            }
        }
    }

    async fn socket_snapshot(path: &std::path::Path) -> io::Result<DebugSnapshot> {
        #[cfg(unix)]
        {
            DebugClient::connect(path).await?.snapshot().await
        }
        #[cfg(not(unix))]
        {
            let _ = path;
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "socket attachment requires Unix",
            ))
        }
    }

    fn snapshot(history: &VecDeque<SnapshotRecord>) -> DebugSnapshot {
        DebugSnapshot {
            state: history.back().map(|r| r.state.clone()).unwrap_or_default(),
            history: history.iter().cloned().collect(),
            components: None,
            control: None,
        }
    }

    fn retain(history: &mut VecDeque<SnapshotRecord>, record: SnapshotRecord, limit: usize) {
        if history.len() == limit {
            history.pop_front();
        }
        history.push_back(record);
    }

    fn read_recording(path: &std::path::Path, limit: usize) -> io::Result<(DebugSnapshot, bool)> {
        let mut reader = RecordingReader::open(path)?;
        let mut history = VecDeque::new();
        loop {
            match reader.read_next() {
                Ok(Some(record)) => retain(&mut history, record, limit),
                Ok(None) => return Ok((snapshot(&history), false)),
                Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                    return Ok((snapshot(&history), true))
                }
                Err(error) => return Err(error),
            }
        }
    }

    async fn follow_recording(
        path: &std::path::Path,
        limit: usize,
        sender: &watch::Sender<Update>,
    ) -> io::Result<()> {
        let mut reader = RecordingReader::follow(path)?;
        let mut history = VecDeque::new();
        let mut dirty = true;
        let mut published = Instant::now();
        loop {
            let finished =
                match tokio::time::timeout(Duration::from_millis(250), reader.next()).await {
                    Ok(Ok(Some(record))) => {
                        retain(&mut history, record, limit);
                        dirty = true;
                        false
                    }
                    Ok(Ok(None)) => true,
                    Ok(Err(error)) => return Err(error),
                    Err(_) => false,
                };
            if finished || (dirty && published.elapsed() >= Duration::from_millis(250)) {
                let status = if finished {
                    "Recording · finished"
                } else {
                    "Recording · following"
                };
                if sender
                    .send(Some(Ok((Arc::new(snapshot(&history)), status.into()))))
                    .is_err()
                {
                    return Ok(());
                }
                published = Instant::now();
                dirty = false;
            }
            if finished {
                return Ok(());
            }
        }
    }

    pub async fn run() -> io::Result<()> {
        let args = Args::parse();
        if args.once {
            let snapshot = if let Some(path) = &args.recording {
                let (snapshot, incomplete) = read_recording(path, args.history_limit as usize)?;
                if incomplete {
                    eprintln!("Recording has an incomplete tail; showing complete records");
                }
                snapshot
            } else {
                socket_snapshot(args.socket.as_ref().unwrap()).await?
            };
            serde_json::to_writer_pretty(io::stdout().lock(), &snapshot)?;
            println!();
            return Ok(());
        }

        enable_raw_mode()?;
        let _guard = TerminalGuard;
        execute!(io::stdout(), EnterAlternateScreen)?;
        let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
        let (sender, mut receiver) = watch::channel(None);
        let (control_sender, control_receiver) = watch::channel(ControlIntent::default());
        // A watch slot retains only the latest snapshot. Slow rendering never
        // creates a backlog, and network waits cannot prevent keyboard handling.
        let reader = tokio::spawn(async move {
            #[cfg(unix)]
            let mut control_receiver = control_receiver;
            #[cfg(not(unix))]
            let _ = control_receiver;
            if let Some(path) = args.recording {
                let result = if args.follow {
                    follow_recording(&path, args.history_limit as usize, &sender).await
                } else {
                    read_recording(&path, args.history_limit as usize).map(
                        |(snapshot, incomplete)| {
                            let status = if incomplete {
                                "Recording · incomplete tail"
                            } else {
                                "Recording · finished"
                            };
                            let _ = sender.send(Some(Ok((Arc::new(snapshot), status.into()))));
                        },
                    )
                };
                if let Err(error) = result {
                    let _ = sender.send(Some(Err(error.to_string())));
                }
                return;
            }
            #[cfg(unix)]
            loop {
                match DebugClient::connect(args.socket.as_ref().unwrap()).await {
                    Ok(mut client) => {
                        if let Err(error) =
                            live_connection(&mut client, &sender, &mut control_receiver).await
                        {
                            let _ = sender.send(Some(Err(error.to_string())));
                        } else {
                            return;
                        }
                    }
                    Err(error) => {
                        let _ = sender.send(Some(Err(error.to_string())));
                    }
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            #[cfg(not(unix))]
            let _ = sender.send(Some(Err("socket attachment requires Unix".into())));
        });
        // Aborting on every exit path also covers draw/input errors.
        struct ReaderGuard(tokio::task::JoinHandle<()>);
        impl Drop for ReaderGuard {
            fn drop(&mut self) {
                self.0.abort();
            }
        }
        let _reader = ReaderGuard(reader);
        let mut inspector = DebugInspector::new().fullscreen();
        let mut status = "Connecting…".to_owned();
        let mut tick = tokio::time::interval(Duration::from_millis(50));
        let mut first_draw = true;
        loop {
            tick.tick().await;
            let mut redraw = first_draw;
            if receiver.has_changed().unwrap_or(false) {
                match receiver.borrow_and_update().as_ref() {
                    Some(Ok((next, next_status))) => {
                        status = match &next.control {
                            Some(control) if control.paused != control.pause_requested => format!(
                                "{next_status} · {}",
                                if control.pause_requested {
                                    "pausing…"
                                } else {
                                    "resuming…"
                                }
                            ),
                            Some(control) => format!(
                                "{next_status} · {}",
                                if control.paused { "PAUSED" } else { "running" }
                            ),
                            None => format!("{next_status} · read only"),
                        };
                        inspector.update(next.state.clone(), action_entries(next)?);
                        inspector.update_components(next.components.clone());
                        inspector.update_control_status(next.control.clone());
                    }
                    Some(Err(error)) => {
                        status = format!("Source error: {error}");
                        inspector.update_control_status(None);
                    }
                    None => {}
                }
                redraw = true;
            }
            while event::poll(Duration::ZERO)? {
                let event = match event::read()? {
                    Event::Key(key) => RawEvent::Key(key),
                    Event::Mouse(mouse) => RawEvent::Mouse(mouse),
                    Event::Resize(width, height) => RawEvent::Resize(width, height),
                    _ => continue,
                };
                if inspector.handle_event(&process_raw_event(event)) {
                    return Ok(());
                }
                redraw = true;
            }
            for command in inspector.take_commands() {
                control_sender.send_modify(|intent| match command {
                    DebugCommand::TogglePause => intent.pause_parity = !intent.pause_parity,
                    DebugCommand::Highlight(ref target) => intent.highlight = target.clone(),
                    DebugCommand::SetPaused(_) => {} // The inspector emits toggles only.
                });
            }
            if !redraw {
                continue;
            }
            first_draw = false;
            terminal.draw(|frame| inspector.render(frame, &status))?;
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use tui_dispatch_debug::{observe::DebugRecord, recording::RecordingWriter};

        #[tokio::test]
        async fn following_publishes_bounded_history_and_finishes_without_a_terminal() {
            tokio::time::timeout(Duration::from_secs(3), async {
                let path =
                    std::env::temp_dir().join(format!("td-follow-cli-{}", std::process::id()));
                struct Cleanup(PathBuf);
                impl Drop for Cleanup {
                    fn drop(&mut self) {
                        let _ = std::fs::remove_file(&self.0);
                    }
                }
                let mut writer = RecordingWriter::create(&path).unwrap();
                let _cleanup = Cleanup(path.clone());
                let (sender, mut receiver) = watch::channel(None);
                let worker = tokio::spawn(async move { follow_recording(&path, 2, &sender).await });
                for sequence in 1..=5 {
                    writer
                        .append(&DebugRecord::dispatch(
                            sequence,
                            "Add",
                            serde_json::json!(sequence),
                            serde_json::json!(sequence),
                            true,
                        ))
                        .unwrap();
                }
                writer.flush().unwrap();
                receiver.changed().await.unwrap();
                {
                    let update = receiver.borrow_and_update();
                    let (snapshot, status) = update.as_ref().unwrap().as_ref().unwrap();
                    assert_eq!(snapshot.state, serde_json::json!(5));
                    assert_eq!(
                        snapshot
                            .history
                            .iter()
                            .map(|r| r.sequence)
                            .collect::<Vec<_>>(),
                        [4, 5]
                    );
                    assert!(status.contains("following"));
                }
                writer.finish().unwrap();
                receiver.changed().await.unwrap();
                let update = receiver.borrow_and_update();
                let (snapshot, status) = update.as_ref().unwrap().as_ref().unwrap();
                assert_eq!(snapshot.history.len(), 2);
                assert!(status.contains("finished"));
                worker.await.unwrap().unwrap();
            })
            .await
            .unwrap();
        }
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    client::run().await
}
