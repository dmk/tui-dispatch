//! Standalone read-only debugger; the app keeps ownership of its terminal.

#[cfg(unix)]
mod unix {
    use std::{io, path::PathBuf, sync::Arc, time::Duration};

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
        debug::{ActionLogDisplayEntry, DebugInspector},
        transport::{DebugClient, DebugSnapshot, SnapshotOutcome},
    };

    #[derive(Parser)]
    #[command(about = "Attach a read-only debugger to a running tui-dispatch app")]
    struct Args {
        /// Unix socket path exposed by the app
        socket: PathBuf,
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

    pub async fn run() -> io::Result<()> {
        let args = Args::parse();
        if args.once {
            let snapshot = DebugClient::connect(args.socket).await?.snapshot().await?;
            serde_json::to_writer_pretty(io::stdout().lock(), &snapshot)?;
            println!();
            return Ok(());
        }

        enable_raw_mode()?;
        let _guard = TerminalGuard;
        execute!(io::stdout(), EnterAlternateScreen)?;
        let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
        let (sender, mut receiver) = watch::channel(None);
        // A watch slot retains only the latest snapshot. Slow rendering never
        // creates a backlog, and network waits cannot prevent keyboard handling.
        let reader = tokio::spawn(async move {
            loop {
                match DebugClient::connect(&args.socket).await {
                    Ok(mut client) => loop {
                        match client.snapshot().await {
                            Ok(snapshot) => {
                                if sender.send(Some(Ok(Arc::new(snapshot)))).is_err() {
                                    return;
                                }
                            }
                            Err(error) => {
                                let _ = sender.send(Some(Err(error.to_string())));
                                break;
                            }
                        }
                        tokio::time::sleep(Duration::from_millis(250)).await;
                    },
                    Err(error) => {
                        let _ = sender.send(Some(Err(error.to_string())));
                    }
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });
        // Aborting on every exit path also covers draw/input errors.
        struct ReaderGuard(tokio::task::JoinHandle<()>);
        impl Drop for ReaderGuard {
            fn drop(&mut self) {
                self.0.abort();
            }
        }
        let _reader = ReaderGuard(reader);
        let mut inspector = DebugInspector::new();
        let mut status = "Connecting…".to_owned();
        let mut tick = tokio::time::interval(Duration::from_millis(50));
        let mut first_draw = true;
        loop {
            tick.tick().await;
            let mut redraw = first_draw;
            if receiver.has_changed().unwrap_or(false) {
                match receiver.borrow_and_update().as_ref() {
                    Some(Ok(next)) => {
                        status = "Connected · read only".into();
                        inspector.update(next.state.clone(), action_entries(next)?);
                    }
                    Some(Err(error)) => status = format!("Disconnected: {error} · retrying"),
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
            if !redraw {
                continue;
            }
            first_draw = false;
            terminal.draw(|frame| inspector.render(frame, &status))?;
        }
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    #[cfg(unix)]
    {
        unix::run().await
    }
    #[cfg(not(unix))]
    {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "debugger attachment requires Unix",
        ))
    }
}
