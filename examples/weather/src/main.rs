//! Weather TUI - tui-dispatch example
//!
//! This example demonstrates the full tui-dispatch pattern:
//! 1. Event (keyboard) -> Component.handle_event() -> Actions
//! 2. Actions dispatched to Store
//! 3. Reducer updates state
//! 4. If state changed, re-render
//!
//! FRAMEWORK PATTERN: The Main Loop
//! - spawn_event_poller for terminal events
//! - Action channel for async results
//! - Store with middleware for state management
//! - Debug layer for inspection (F12)
//! - Render on state change
//!
//! # Features
//!
//! - **Debug mode** (F12): Freeze UI, inspect state, view action log
//! - **Auto-refresh**: Weather updates automatically every 5 minutes
//! - **Action logging**: All actions tracked with timestamps
//!
//! # Usage
//!
//! ```sh
//! # Run with default city (Kyiv)
//! cargo run -p weather-example
//!
//! # Run with custom city
//! cargo run -p weather-example -- --city London
//! ```

mod action;
mod api;
mod components;
mod reducer;
mod sprites;
mod state;

use std::io;
use std::time::Duration;

use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tui_dispatch::debug::{
    ActionLoggerMiddleware, DebugAction, DebugLayer, DebugSideEffect, SimpleDebugContext,
};
use tui_dispatch::{
    EventKind, RawEvent, StoreWithMiddleware, process_raw_event, spawn_event_poller,
};

use crate::action::Action;
use crate::api::GeocodingError;
use crate::components::{WeatherDisplay, WeatherDisplayProps};
use crate::reducer::reducer;
use crate::state::{AppState, Location};

/// Weather TUI - tui-dispatch framework example
#[derive(Parser, Debug)]
#[command(name = "weather")]
#[command(about = "A weather TUI demonstrating tui-dispatch patterns")]
struct Args {
    /// City name to look up (uses Open-Meteo geocoding)
    #[arg(long, short, default_value = "Kyiv")]
    city: String,

    #[arg(long, short, default_value = "30")]
    refresh_interval: u64,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let args = Args::parse();

    // Geocode city before entering TUI mode
    let location = match api::geocode_city(&args.city).await {
        Ok(loc) => loc,
        Err(e) => {
            match e {
                GeocodingError::NotFound(city) => {
                    eprintln!(
                        "Error: City '{}' not found. Please check the spelling.",
                        city
                    );
                    eprintln!("Examples: 'London', 'Tokyo', 'New York'");
                }
                GeocodingError::Request(e) => {
                    eprintln!("Error: Could not connect to geocoding service.");
                    eprintln!("Details: {}", e);
                }
            }
            std::process::exit(1);
        }
    };

    // ===== Terminal setup =====
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run the app and capture result
    let result = run_app(&mut terminal, location, args.refresh_interval).await;

    // ===== Cleanup =====
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    location: Location,
    refresh_interval: u64,
) -> io::Result<()> {
    // ===== Framework setup =====

    // Action channel - receives actions from:
    // 1. Component event handlers
    // 2. Async tasks (API calls)
    let (action_tx, mut action_rx) = mpsc::unbounded_channel::<Action>();

    // Store with middleware for action logging
    let middleware = ActionLoggerMiddleware::with_default_log();
    let mut store = StoreWithMiddleware::new(AppState::new(location), reducer, middleware);

    // Debug layer for inspection (F12)
    let mut debug: DebugLayer<Action, SimpleDebugContext> = DebugLayer::simple();

    // Event poller - converts terminal events to RawEvent
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<RawEvent>();
    let cancel_token = CancellationToken::new();
    let _event_handle = spawn_event_poller(
        event_tx,
        Duration::from_millis(10), // poll timeout
        Duration::from_millis(16), // loop sleep (~60fps)
        cancel_token.clone(),
    );

    // Tick timer for loading animation
    let tick_tx = action_tx.clone();
    let tick_cancel = cancel_token.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        loop {
            tokio::select! {
                _ = tick_cancel.cancelled() => break,
                _ = interval.tick() => {
                    let _ = tick_tx.send(Action::Tick);
                }
            }
        }
    });

    // Auto-refresh timer (every 5 minutes)
    let refresh_tx = action_tx.clone();
    let refresh_cancel = cancel_token.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(refresh_interval));
        interval.tick().await; // Skip immediate first tick
        loop {
            tokio::select! {
                _ = refresh_cancel.cancelled() => break,
                _ = interval.tick() => {
                    let _ = refresh_tx.send(Action::WeatherFetch);
                }
            }
        }
    });

    // Component
    let mut weather_display = WeatherDisplay;

    // Initial render
    let mut should_render = true;

    // Auto-fetch weather on start
    let _ = action_tx.send(Action::WeatherFetch);

    // ===== Main loop =====
    loop {
        // 1. Render if state changed
        if should_render {
            let is_focused = !debug.is_enabled();
            terminal.draw(|frame| {
                debug.render(frame, |f, area| {
                    let props = WeatherDisplayProps {
                        state: store.state(),
                        is_focused,
                    };
                    weather_display.render(f, area, props);
                });
            })?;
            should_render = false;
        }

        // 2. Wait for events or actions
        tokio::select! {
            // Terminal event received
            Some(raw_event) = event_rx.recv() => {
                let event_kind = process_raw_event(raw_event);

                // Handle resize events directly
                if let EventKind::Resize(width, height) = event_kind {
                    let _ = action_tx.send(Action::UiTerminalResize(width, height));
                    should_render = true;
                    continue;
                }

                // Handle debug toggle (F12)
                if let EventKind::Key(key_event) = &event_kind {
                    if key_event.code == KeyCode::F(12) {
                        if let Some(side_effect) = debug.handle_action(DebugAction::Toggle) {
                            handle_debug_side_effect(side_effect, &action_tx);
                        }
                        should_render = true;
                        continue;
                    }

                    // Handle debug mode keys
                    if debug.is_enabled() {
                        let debug_action = match key_event.code {
                            KeyCode::Esc => Some(DebugAction::Toggle),
                            KeyCode::Char('s') | KeyCode::Char('S') => {
                                Some(DebugAction::ToggleState)
                            }
                            KeyCode::Char('a') | KeyCode::Char('A') => {
                                // Show action log
                                if let Some(log) = store.middleware().log() {
                                    debug.show_action_log(log);
                                }
                                should_render = true;
                                continue;
                            }
                            KeyCode::Char('y') | KeyCode::Char('Y') => {
                                Some(DebugAction::CopyFrame)
                            }
                            KeyCode::Char('i') | KeyCode::Char('I') => {
                                Some(DebugAction::ToggleMouseCapture)
                            }
                            KeyCode::Up => Some(DebugAction::ActionLogScrollUp),
                            KeyCode::Down => Some(DebugAction::ActionLogScrollDown),
                            KeyCode::Home => Some(DebugAction::ActionLogScrollTop),
                            KeyCode::End => Some(DebugAction::ActionLogScrollBottom),
                            _ => None,
                        };

                        if let Some(action) = debug_action {
                            if let Some(side_effect) = debug.handle_action(action) {
                                handle_debug_side_effect(side_effect, &action_tx);
                            }
                            should_render = true;
                            continue;
                        }
                    }
                }

                // When debug is enabled, queue app actions instead of processing
                if debug.is_enabled() {
                    continue;
                }

                // Pass to component, collect actions
                let props = WeatherDisplayProps {
                    state: store.state(),
                    is_focused: true,
                };
                let actions = weather_display.handle_event(&event_kind, props);

                // Queue actions for dispatch
                for action in actions {
                    let _ = action_tx.send(action);
                }
            }

            // Action received (from component or async task)
            Some(action) = action_rx.recv() => {
                // Handle quit before dispatch
                if matches!(action, Action::Quit) {
                    break;
                }

                // Handle async trigger before dispatch
                if matches!(action, Action::WeatherFetch) {
                    let loc = store.state().current_location();
                    let tx = action_tx.clone();
                    let lat = loc.lat;
                    let lon = loc.lon;
                    tokio::spawn(async move {
                        api::fetch_weather(lat, lon, tx).await;
                    });
                }

                // Dispatch to store
                let state_changed = store.dispatch(action);
                should_render = state_changed;
            }
        }
    }

    // Cancel background tasks
    cancel_token.cancel();

    Ok(())
}

/// Handle debug side effects
fn handle_debug_side_effect(
    side_effect: DebugSideEffect<Action>,
    action_tx: &mpsc::UnboundedSender<Action>,
) {
    match side_effect {
        DebugSideEffect::ProcessQueuedActions(actions) => {
            for action in actions {
                let _ = action_tx.send(action);
            }
        }
        DebugSideEffect::CopyToClipboard(_text) => {
            // Could integrate with clipboard crate
        }
        DebugSideEffect::EnableMouseCapture | DebugSideEffect::DisableMouseCapture => {
            // Mouse capture is always enabled in this example
        }
    }
}
