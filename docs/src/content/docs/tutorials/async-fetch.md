---
title: "Tutorial: Fetching Data from an API"
description: Build a TUI that looks up GitHub users with async effects
---

In this tutorial, you'll build a small TUI that looks up GitHub users.

The goal isn't "a fancy UI" — it's learning the tui-dispatch async story:

- reducers stay synchronous and pure
- reducers *declare* side effects as `Effect` values
- the runtime runs those effects (usually by spawning tasks)
- async completion comes back as normal actions (often named `Did*`)

This is the same pattern used in `examples/github-lookup`.

## What You'll Build

- Type a GitHub username
- Press Enter to fetch
- Show loading / error / user info
- Esc clears

## Prereqs

- Rust + async/await familiarity
- You've skimmed [Core Concepts](/tui-dispatch/getting-started/core-concepts/)

## Project Setup

```bash
cargo new github-lookup
cd github-lookup
```

`Cargo.toml`:

```toml
[package]
name = "github-lookup"
version = "0.1.0"
edition = "2021"

[dependencies]
tui-dispatch = { version = "0.5.4", features = ["tasks"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time"] }
ratatui = "0.29"
crossterm = "0.28"
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

## The Data Flow (Keep This Mental Model)

```
key press
  ↓
EventBus routes event
  ↓
Action (intent)
  ↓
reducer updates state + returns Effect
  ↓
effect handler spawns async task
  ↓
Action (DidLoad / DidError)
  ↓
reducer updates state
  ↓
render from state
```

The reducer never does IO.

## Part 1: State

Create `src/state.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GitHubUser {
    pub login: String,
    pub name: Option<String>,
    pub bio: Option<String>,
    pub public_repos: u32,
    pub followers: u32,
    pub following: u32,
    pub avatar_url: String,
}

#[derive(Clone, Debug, Default)]
pub struct AppState {
    pub query: String,
    pub user: Option<GitHubUser>,
    pub is_loading: bool,
    pub error: Option<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }
}
```

## Part 2: Actions

Create `src/action.rs`:

```rust
use crate::state::GitHubUser;

#[derive(tui_dispatch::Action, Clone, Debug, PartialEq)]
pub enum Action {
    QueryChange(String),

    // Intent
    UserFetch(String),

    // Results
    UserDidLoad(GitHubUser),
    UserDidError(String),

    Clear,
    Quit,
}
```

Naming note:
- `UserFetch` is an intent action (user asked for work)
- `UserDidLoad` / `UserDidError` are results (async finished)

## Part 3: Effects

Create `src/effect.rs`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    FetchUser { username: String },
}
```

Effects are data. You can test them easily.

## Part 4: Reducer

Create `src/reducer.rs`:

```rust
use tui_dispatch::DispatchResult;

use crate::action::Action;
use crate::effect::Effect;
use crate::state::AppState;

pub fn reducer(state: &mut AppState, action: Action) -> DispatchResult<Effect> {
    match action {
        Action::QueryChange(query) => {
            state.query = query;
            DispatchResult::changed()
        }

        Action::UserFetch(username) => {
            let username = username.trim().to_string();
            if username.is_empty() {
                return DispatchResult::unchanged();
            }

            state.is_loading = true;
            state.error = None;
            state.user = None;

            DispatchResult::changed_with(Effect::FetchUser { username })
        }

        Action::UserDidLoad(user) => {
            state.user = Some(user);
            state.is_loading = false;
            state.error = None;
            DispatchResult::changed()
        }

        Action::UserDidError(msg) => {
            state.is_loading = false;
            state.error = Some(msg);
            state.user = None;
            DispatchResult::changed()
        }

        Action::Clear => {
            state.query.clear();
            state.user = None;
            state.error = None;
            state.is_loading = false;
            DispatchResult::changed()
        }

        Action::Quit => DispatchResult::unchanged(),
    }
}
```

## Part 5: GitHub API Client

Create `src/api.rs`:

```rust
use serde::Deserialize;

use crate::state::GitHubUser;

#[derive(Debug, Deserialize)]
struct GitHubApiResponse {
    login: String,
    name: Option<String>,
    bio: Option<String>,
    public_repos: u32,
    followers: u32,
    following: u32,
    avatar_url: String,
}

pub async fn fetch_user(username: &str) -> Result<GitHubUser, String> {
    let url = format!("https://api.github.com/users/{}", username);

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("User-Agent", "tui-dispatch-example")
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(format!("User '{}' not found", username));
    }

    if !response.status().is_success() {
        return Err(format!("GitHub API error: {}", response.status()));
    }

    let data: GitHubApiResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(GitHubUser {
        login: data.login,
        name: data.name,
        bio: data.bio,
        public_repos: data.public_repos,
        followers: data.followers,
        following: data.following,
        avatar_url: data.avatar_url,
    })
}
```

## Part 6: UI

Create `src/ui.rs`:

```rust
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::action::Action;
use crate::state::AppState;

pub fn render_area(frame: &mut Frame, area: Rect, state: &AppState) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(10),
        Constraint::Length(1),
    ])
    .split(area);

    render_input(frame, chunks[0], state);
    render_content(frame, chunks[1], state);
    render_help(frame, chunks[2]);
}

fn render_input(frame: &mut Frame, area: Rect, state: &AppState) {
    let input = Paragraph::new(state.query.as_str())
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::ALL).title(" GitHub Username "));
    frame.render_widget(input, area);

    frame.set_cursor_position((area.x + state.query.len() as u16 + 1, area.y + 1));
}

fn render_content(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default().borders(Borders::ALL).title(" User Info ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.is_loading {
        let loading = Paragraph::new("Loading...")
            .style(Style::default().fg(Color::Cyan))
            .alignment(Alignment::Center);
        frame.render_widget(loading, inner);
        return;
    }

    if let Some(error) = &state.error {
        let error_text = Paragraph::new(error.as_str())
            .style(Style::default().fg(Color::Red))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
        frame.render_widget(error_text, inner);
        return;
    }

    if let Some(user) = &state.user {
        let name_display = user
            .name
            .as_ref()
            .map(|n| format!("{} (@{})", n, user.login))
            .unwrap_or_else(|| format!("@{}", user.login));

        let bio_display = user.bio.as_deref().unwrap_or("No bio");

        let lines = vec![
            Line::from(vec![Span::styled(
                name_display,
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Bio: ", Style::default().fg(Color::Cyan)),
                Span::raw(bio_display),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Repos: ", Style::default().fg(Color::Cyan)),
                Span::raw(user.public_repos.to_string()),
                Span::raw("  "),
                Span::styled("Followers: ", Style::default().fg(Color::Cyan)),
                Span::raw(user.followers.to_string()),
                Span::raw("  "),
                Span::styled("Following: ", Style::default().fg(Color::Cyan)),
                Span::raw(user.following.to_string()),
            ]),
        ];

        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
        return;
    }

    let help = Paragraph::new("Enter a GitHub username and press Enter to search")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    frame.render_widget(help, inner);
}

fn render_help(frame: &mut Frame, area: Rect) {
    let help = Paragraph::new(" Enter: Search | Esc: Clear | Ctrl+C: Quit ")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, area);
}

pub fn handle_key(key: KeyEvent, state: &AppState) -> Vec<Action> {
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => vec![Action::Quit],
        KeyCode::Enter => {
            if state.query.is_empty() {
                vec![]
            } else {
                vec![Action::UserFetch(state.query.clone())]
            }
        }
        KeyCode::Esc => vec![Action::Clear],
        KeyCode::Backspace => {
            let mut new_query = state.query.clone();
            new_query.pop();
            vec![Action::QueryChange(new_query)]
        }
        KeyCode::Char(c) => {
            let new_query = format!("{}{}", state.query, c);
            vec![Action::QueryChange(new_query)]
        }
        _ => vec![],
    }
}
```

## Part 7: Wire It Up With EffectRuntime + EventBus

Create `src/main.rs`:

```rust
mod action;
mod api;
mod effect;
mod reducer;
mod state;
mod ui;

use std::io;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tui_dispatch::prelude::*;

use crate::action::Action;
use crate::effect::Effect;
use crate::reducer::reducer;
use crate::state::AppState;

#[derive(ComponentId, Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum AppComponentId {
    Main,
}

impl EventRoutingState<AppComponentId, DefaultBindingContext> for AppState {
    fn focused(&self) -> Option<AppComponentId> { Some(AppComponentId::Main) }
    fn modal(&self) -> Option<AppComponentId> { None }
    fn binding_context(&self, _id: AppComponentId) -> DefaultBindingContext { DefaultBindingContext }
    fn default_context(&self) -> DefaultBindingContext { DefaultBindingContext }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    result
}

async fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>) -> io::Result<()> {
    let mut runtime: EffectRuntime<AppState, Action, Effect> =
        EffectRuntime::new(AppState::new(), reducer);
    let mut bus: SimpleEventBus<AppState, Action, AppComponentId> = SimpleEventBus::new();
    let keybindings: Keybindings<DefaultBindingContext> = Keybindings::new();

    bus.register(AppComponentId::Main, |event, state| {
        let actions = match event.kind {
            EventKind::Key(key) => ui::handle_key(key, state),
            _ => Vec::new(),
        };
        if actions.is_empty() {
            HandlerResponse::ignored()
        } else {
            HandlerResponse::actions(actions)
        }
    });

    runtime
        .run_with_bus(
            terminal,
            &mut bus,
            &keybindings,
            |frame, area, state, _ctx: RenderContext, event_ctx| {
                event_ctx.set_component_area(AppComponentId::Main, area);
                ui::render_area(frame, area, state);
            },
            |action| matches!(action, Action::Quit),
            handle_effect,
        )
        .await
}

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
```

Run it:

```bash
cargo run
```

## Part 8: Testing The Reducer (No HTTP Needed)

You can test the full intent/effect/result flow without doing network IO.

Create `tests/reducer_tests.rs`:

```rust
use github_lookup::{action::Action, effect::Effect, reducer::reducer, state::{AppState, GitHubUser}};
use tui_dispatch::testing::{EffectAssertions, EffectStoreTestHarness};

fn mock_user() -> GitHubUser {
    GitHubUser {
        login: "octocat".into(),
        name: Some("The Octocat".into()),
        bio: Some("GitHub mascot".into()),
        public_repos: 8,
        followers: 1000,
        following: 9,
        avatar_url: "https://example.com/avatar.png".into(),
    }
}

#[test]
fn test_fetch_emits_effect() {
    let mut harness = EffectStoreTestHarness::new(AppState::default(), reducer);

    harness.dispatch_collect(Action::UserFetch("octocat".into()));
    harness.assert_state(|s| s.is_loading);

    let effects = harness.drain_effects();
    effects.effects_count(1);
    effects.effects_first_matches(|e| matches!(e, Effect::FetchUser { username } if username == "octocat"));
}

#[test]
fn test_complete_flow() {
    let mut harness = EffectStoreTestHarness::new(AppState::default(), reducer);

    harness.dispatch_collect(Action::UserFetch("octocat".into()));

    // simulate async completion:
    harness.complete_action(Action::UserDidLoad(mock_user()));
    harness.process_emitted();

    harness.assert_state(|s| !s.is_loading);
    harness.assert_state(|s| s.user.as_ref().is_some_and(|u| u.login == "octocat"));
}
```

## Summary

You now have a complete async app where:

- reducer code is synchronous and testable
- side effects are explicit (`Effect`)
- async code lives in the effect handler
- UI stays dumb: state in, actions out

If you want a reference implementation, compare your code with `examples/github-lookup` in the tui-dispatch repo.
