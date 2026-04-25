//! GitHub User Lookup TUI
//!
//! This example demonstrates the async/effects pattern in tui-dispatch:
//!
//! 1. User types a username and presses Enter
//! 2. UI emits `Action::UserFetch(username)`
//! 3. Reducer sets loading state and returns `Effect::FetchUser`
//! 4. Effect handler spawns async task via `ctx.tasks().spawn()`
//! 5. Task completes and sends `Action::UserDidLoad` or `Action::UserDidError`
//! 6. Reducer updates state, UI re-renders
//!
//! Run with: cargo run -p github-lookup-example

use std::io;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tui_dispatch::{EffectContext, EventKind, EventOutcome, RenderContext, Runtime};

use github_lookup::action::Action;
use github_lookup::api;
use github_lookup::effect::Effect;
use github_lookup::reducer::reducer;
use github_lookup::state::AppState;
use github_lookup::ui;

#[tokio::main]
async fn main() -> io::Result<()> {
    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run the app
    let result = run_app(&mut terminal).await;

    // Cleanup
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

async fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>) -> io::Result<()> {
    // Create the runtime with initial state and reducer
    let mut runtime: Runtime<AppState, Action, Effect> = Runtime::new(AppState::new(), reducer);

    // Run the main event loop
    runtime
        .run_with_effects(
            terminal,
            // Render function
            |frame, area, state, _render_ctx: RenderContext| {
                ui::render_area(frame, area, state);
            },
            // Event handler - convert terminal events to actions
            |event, state| -> EventOutcome<Action> {
                if let EventKind::Key(key) = event {
                    EventOutcome::from_actions(ui::handle_key(*key, state))
                } else {
                    EventOutcome::ignored()
                }
            },
            // Quit predicate
            |action| matches!(action, Action::Quit),
            // Effect handler
            handle_effect,
        )
        .await
}

/// Handle effects by spawning async tasks
fn handle_effect(effect: Effect, ctx: &mut EffectContext<Action>) {
    match effect {
        Effect::FetchUser { username } => {
            ctx.tasks().spawn("user_fetch", async move {
                match api::fetch_user(&username).await {
                    Ok(user) => Action::UserDidLoad(user),
                    Err(e) => Action::UserDidError(e),
                }
            });
        }
    }
}
