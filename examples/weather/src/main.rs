//! Weather TUI - tui-dispatch example
//!
//! This example demonstrates the full tui-dispatch pattern with effects:
//! 1. Event (keyboard) -> Component.handle_event() -> Actions
//! 2. Actions dispatched to EffectStore
//! 3. Reducer updates state and returns effects
//! 4. Effects handled by TaskManager
//! 5. If state changed, re-render
//!
//! FRAMEWORK PATTERN: The Main Loop with Effects
//! - spawn_event_poller for terminal events
//! - EffectStore for state management with declarative effects
//! - TaskManager for async operations (API calls)
//! - Subscriptions for continuous sources (tick timer, auto-refresh)
//! - Debug layer for inspection (F12)
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
mod effect;
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
use tui_dispatch::debug::{DebugLayer, DebugSideEffect};
use tui_dispatch::{
    EffectStoreWithMiddleware, EventKind, RawEvent, Subscriptions, TaskManager, process_raw_event,
    spawn_event_poller,
};

use crate::action::Action;
use crate::api::GeocodingError;
use crate::components::{WeatherDisplay, WeatherDisplayProps};
use crate::effect::Effect;
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

    /// Refresh interval in seconds
    #[arg(long, short, default_value = "30")]
    refresh_interval: u64,

    /// Enable debug mode (F12 to toggle overlay)
    #[arg(long)]
    debug: bool,
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
    let result = run_app(&mut terminal, location, args.refresh_interval, args.debug).await;

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
    debug_enabled: bool,
) -> io::Result<()> {
    // ===== Framework setup =====

    // Action channel - receives actions from:
    // 1. Component event handlers
    // 2. Async tasks (via TaskManager)
    // 3. Subscriptions (tick, refresh timers)
    let (action_tx, mut action_rx) = mpsc::unbounded_channel::<Action>();

    // EffectStore for state management
    let mut store = EffectStoreWithMiddleware::new(
        AppState::new(location),
        reducer,
        tui_dispatch::NoopMiddleware,
    );

    // TaskManager for async operations (API calls)
    let tasks = TaskManager::new(action_tx.clone());

    // Subscriptions for continuous action sources
    let subs = Subscriptions::new(action_tx.clone());

    // Debug layer for inspection (F12) - only active when --debug
    // Automatically pauses tasks/subs when debug mode is enabled
    let mut debug = DebugLayer::new(KeyCode::F(12))
        .with_task_manager(&tasks)
        .with_subscriptions(&subs)
        .active(debug_enabled);

    // Now we can mutate tasks and subs
    let mut tasks = tasks;
    let mut subs = subs;

    // Tick timer for loading animation (100ms)
    subs.interval("tick", Duration::from_millis(100), || Action::Tick);

    // Auto-refresh timer
    subs.interval("refresh", Duration::from_secs(refresh_interval), || {
        Action::WeatherFetch
    });

    // Event poller - converts terminal events to RawEvent
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<RawEvent>();
    let cancel_token = CancellationToken::new();
    let _event_handle = spawn_event_poller(
        event_tx,
        Duration::from_millis(10), // poll timeout
        Duration::from_millis(16), // loop sleep (~60fps)
        cancel_token.clone(),
    );

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

                // Debug layer handles F12, state overlay, action log, etc.
                if let Some(effects) = debug.intercepts_with_effects(&event_kind) {
                    for effect in effects {
                        handle_debug_side_effect(effect, &action_tx);
                    }
                    // Refresh state overlay if it's currently shown
                    if debug.is_state_overlay_visible() {
                        debug.show_state_overlay(store.state());
                    }
                    should_render = true;
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

            // Action received (from component, TaskManager, or Subscriptions)
            Some(action) = action_rx.recv() => {
                // Handle quit before dispatch
                if matches!(action, Action::Quit) {
                    break;
                }

                // Log action for debug overlay
                debug.log_action(&action);

                // Dispatch to store - returns effects
                let result = store.dispatch(action);

                // Handle effects via TaskManager
                for effect in result.effects {
                    handle_effect(effect, &mut tasks);
                }

                should_render = result.changed;
            }
        }
    }

    // Cleanup
    cancel_token.cancel();
    subs.cancel_all();
    tasks.cancel_all();

    Ok(())
}

/// Handle effects by spawning tasks
fn handle_effect(effect: Effect, tasks: &mut TaskManager<Action>) {
    match effect {
        Effect::FetchWeather { lat, lon } => {
            tasks.spawn("weather", async move {
                match api::fetch_weather_data(lat, lon).await {
                    Ok(data) => Action::WeatherDidLoad(data),
                    Err(e) => Action::WeatherDidError(e),
                }
            });
        }
    }
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
    }
}
