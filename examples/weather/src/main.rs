//! Weather TUI - tui-dispatch example
//!
//! This example demonstrates the full tui-dispatch pattern with effects:
//! 1. Event (keyboard) -> Component.handle_event() -> Actions
//! 2. Actions dispatched to EffectStore
//! 3. Reducer updates state and returns effects
//! 4. Effects handled by TaskManager
//! 5. If state changed, re-render
//!
//! FRAMEWORK PATTERN: EffectRuntime loop
//! - EffectStore for state management with declarative effects
//! - EffectRuntime handles event polling + action routing
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

use std::cell::RefCell;
use std::io;
use std::time::Duration;

use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Frame, Terminal, backend::CrosstermBackend, layout::Rect};
use tui_dispatch::debug::DebugLayer;
use tui_dispatch::{
    EffectContext, EffectRuntime, EffectStoreWithMiddleware, EventKind, EventOutcome,
    RenderContext, TaskKey,
};

use crate::action::Action;
use crate::api::GeocodingError;
use crate::components::{
    Component, SearchOverlay, SearchOverlayProps, WeatherDisplay, WeatherDisplayProps,
};
use crate::effect::Effect;
use crate::reducer::reducer;
use crate::state::{AppState, LOADING_ANIM_TICK_MS, Location};

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

struct WeatherUi {
    display: WeatherDisplay,
    search: SearchOverlay,
}

impl WeatherUi {
    fn new() -> Self {
        Self {
            display: WeatherDisplay,
            search: SearchOverlay::new(),
        }
    }

    fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        state: &AppState,
        render_ctx: RenderContext,
    ) {
        let props = WeatherDisplayProps {
            state,
            is_focused: render_ctx.is_focused() && !state.search_mode,
        };
        self.display.render(frame, area, props);

        self.search.set_open(state.search_mode);
        if state.search_mode {
            let props = SearchOverlayProps {
                query: &state.search_query,
                results: &state.search_results,
                selected: state.search_selected,
                is_focused: render_ctx.is_focused(),
                error: state.search_error.as_deref(),
                on_query_change: Action::SearchQueryChange,
                on_query_submit: Action::SearchQuerySubmit,
                on_select: Action::SearchSelect,
            };
            self.search.render(frame, area, props);
        }
    }

    fn map_event(&mut self, event: &EventKind, state: &AppState) -> EventOutcome<Action> {
        if let EventKind::Resize(width, height) = event {
            return EventOutcome::action(Action::UiTerminalResize(*width, *height)).with_render();
        }

        if state.search_mode {
            let props = SearchOverlayProps {
                query: &state.search_query,
                results: &state.search_results,
                selected: state.search_selected,
                is_focused: true,
                error: state.search_error.as_deref(),
                on_query_change: Action::SearchQueryChange,
                on_query_submit: Action::SearchQuerySubmit,
                on_select: Action::SearchSelect,
            };
            return EventOutcome::from_actions(self.search.handle_event(event, props));
        }

        let props = WeatherDisplayProps {
            state,
            is_focused: true,
        };
        EventOutcome::from_actions(self.display.handle_event(event, props))
    }
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    location: Location,
    refresh_interval: u64,
    debug_enabled: bool,
) -> io::Result<()> {
    // EffectStore for state management
    let store = EffectStoreWithMiddleware::new(
        AppState::new(location),
        reducer,
        tui_dispatch::NoopMiddleware,
    );

    // Debug layer for inspection (F12) - only active when --debug
    let debug = DebugLayer::simple().active(debug_enabled);

    let mut runtime = EffectRuntime::from_store(store).with_debug(debug);

    // Tick timer for loading animation
    runtime
        .subscriptions()
        .interval("tick", Duration::from_millis(LOADING_ANIM_TICK_MS), || {
            Action::Tick
        });

    // Auto-refresh timer
    runtime
        .subscriptions()
        .interval("refresh", Duration::from_secs(refresh_interval), || {
            Action::WeatherFetch
        });

    // Auto-fetch weather on start
    runtime.enqueue(Action::WeatherFetch);

    let ui = RefCell::new(WeatherUi::new());

    runtime
        .run(
            terminal,
            |frame, area, state, render_ctx| {
                ui.borrow_mut().render(frame, area, state, render_ctx);
            },
            |event, state| ui.borrow_mut().map_event(event, state),
            |action| matches!(action, Action::Quit),
            handle_effect,
        )
        .await
}

/// Handle effects by spawning tasks
fn handle_effect(effect: Effect, ctx: &mut EffectContext<Action>) {
    match effect {
        Effect::FetchWeather { lat, lon } => {
            ctx.tasks().spawn("weather", async move {
                match api::fetch_weather_data(lat, lon).await {
                    Ok(data) => Action::WeatherDidLoad(data),
                    Err(e) => Action::WeatherDidError(e),
                }
            });
        }
        Effect::SearchCities { query } => {
            let query = query.trim().to_string();
            if query.is_empty() {
                ctx.tasks().cancel(&TaskKey::new("city_search"));
                return;
            }
            ctx.tasks()
                .debounce("city_search", Duration::from_millis(300), async move {
                    match api::search_cities(&query).await {
                        Ok(results) => Action::SearchDidLoad(results),
                        Err(e) => Action::SearchDidError(e.to_string()),
                    }
                });
        }
    }
}
