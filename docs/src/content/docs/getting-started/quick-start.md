---
title: Quick Start
description: Get started with tui-dispatch in minutes
---

This page gets you to a running app quickly, using the runtime helpers that ship with tui-dispatch.

## Install

Add the basics:

```toml
[dependencies]
tui-dispatch = "0.5.3"
ratatui = "0.29"
crossterm = "0.28"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time"] }
```

If you know you'll be doing async work, enable the runtime helpers:

```toml
[dependencies]
tui-dispatch = { version = "0.5.3", features = ["tasks", "subscriptions"] }
```

- `tasks`: `TaskManager` (cancellation + debounce)
- `subscriptions`: timers/streams that emit actions

## Quick Start: A Tiny Counter

This is the smallest thing that feels like a real app: proper terminal init/cleanup + a `DispatchRuntime` event loop.

```rust
use std::io;

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, widgets::Paragraph, Terminal};
use tui_dispatch::prelude::*;

#[derive(Default)]
struct AppState {
    count: i32,
}

#[derive(Action, Clone, Debug)]
enum Action {
    Inc,
    Dec,
    Quit,
}

#[derive(ComponentId, Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum AppComponentId {
    Counter,
}

impl EventRoutingState<AppComponentId, DefaultBindingContext> for AppState {
    fn focused(&self) -> Option<AppComponentId> { Some(AppComponentId::Counter) }
    fn modal(&self) -> Option<AppComponentId> { None }
    fn binding_context(&self, _id: AppComponentId) -> DefaultBindingContext { DefaultBindingContext }
    fn default_context(&self) -> DefaultBindingContext { DefaultBindingContext }
}

fn reducer(state: &mut AppState, action: Action) -> bool {
    match action {
        Action::Inc => {
            state.count += 1;
            true
        }
        Action::Dec => {
            state.count -= 1;
            true
        }
        Action::Quit => false,
    }
}

fn render(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    _ctx: RenderContext,
    event_ctx: &mut EventContext<AppComponentId>,
) {
    event_ctx.set_component_area(AppComponentId::Counter, area);
    frame.render_widget(
        Paragraph::new(format!("count = {}  (k/j, q)", state.count)),
        area,
    );
}

fn handle_event(event: &EventKind) -> Option<Action> {
    if let EventKind::Key(key) = event {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Char('k') | KeyCode::Up => Some(Action::Inc),
            KeyCode::Char('j') | KeyCode::Down => Some(Action::Dec),
            KeyCode::Char('q') | KeyCode::Esc => Some(Action::Quit),
            _ => None,
        }
    } else {
        None
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>) -> io::Result<()> {
    let mut runtime = DispatchRuntime::new(AppState::default(), reducer);
    let mut bus: SimpleEventBus<AppState, Action, AppComponentId> = SimpleEventBus::new();
    let keybindings: Keybindings<DefaultBindingContext> = Keybindings::new();

    bus.register(AppComponentId::Counter, |event, _state| {
        match handle_event(&event.kind) {
            Some(action) => HandlerResponse::action(action),
            None => HandlerResponse::ignored(),
        }
    });

    runtime
        .run_with_bus(terminal, &mut bus, &keybindings, render, |a| {
            matches!(a, Action::Quit)
        })
        .await
}
```

Run it:

```bash
cargo run
```

Keys: `k`/`Up` increments, `j`/`Down` decrements, `q`/`Esc` quits.

> **Next:** If you want to learn the async/effects pattern, continue to [Tutorial: Fetching Data from an API](/tui-dispatch/tutorials/async-fetch/). It builds a complete app and explains the mental model.

## Choose Your Pattern

| Do you need side effects (HTTP, file IO, timers)? | Use | Example |
|---|---|---|
| No | `DispatchRuntime` + `EventBus` + `fn reducer(...) -> bool` | [Counter](/tui-dispatch/examples/counter/) |
| Yes | `EffectRuntime` + `EventBus` + `fn reducer(...) -> DispatchResult<Effect>` | [GitHub Lookup](/tui-dispatch/tutorials/async-fetch/), [Weather](/tui-dispatch/examples/weather/) |

## When You Need Async: Effects + TaskManager

The recommended pattern is:

- Intent action (user asks for work) -> reducer returns an `Effect`
- Effect handler spawns async work
- Async completion dispatches a normal action back into the runtime

Enable `tasks`:

```toml
tui-dispatch = { version = "0.5.3", features = ["tasks"] }
```

Skeleton:

```rust
use tui_dispatch::prelude::*;

#[derive(Action, Clone, Debug)]
enum Action {
    Fetch,
    DidLoad(String),
    DidError(String),
}

#[derive(Debug, Clone)]
enum Effect {
    FetchData,
}

#[derive(Default)]
struct State {
    loading: bool,
    data: Option<String>,
    error: Option<String>,
}

fn reducer(state: &mut State, action: Action) -> DispatchResult<Effect> {
    match action {
        Action::Fetch => {
            state.loading = true;
            state.error = None;
            DispatchResult::changed_with(Effect::FetchData)
        }
        Action::DidLoad(data) => {
            state.loading = false;
            state.data = Some(data);
            DispatchResult::changed()
        }
        Action::DidError(err) => {
            state.loading = false;
            state.error = Some(err);
            DispatchResult::changed()
        }
    }
}

fn handle_effect(effect: Effect, ctx: &mut EffectContext<Action>) {
    match effect {
        Effect::FetchData => {
            ctx.tasks().spawn("fetch", async move {
                match api::fetch().await {
                    Ok(data) => Action::DidLoad(data),
                    Err(e) => Action::DidError(e.to_string()),
                }
            });
        }
    }
}
```

Then run it with `EffectRuntime` (using the same EventBus setup as above):

```rust
let mut runtime = EffectRuntime::new(State::default(), reducer);
runtime
    .run_with_bus(terminal, &mut bus, &keybindings, render, is_quit, handle_effect)
    .await?;
```

## Debug Mode (F12)

If you want the debug overlay via `DispatchRuntime::with_debug(...)` / `EffectRuntime::with_debug(...)`, your state type must implement `DebugState`.

```rust
use tui_dispatch::debug::DebugLayer;

#[derive(Default, tui_dispatch::DebugState)]
struct AppState {
    count: i32,
}

let debug: DebugLayer<Action> = DebugLayer::simple().active(true);
let mut runtime = DispatchRuntime::new(AppState::default(), reducer).with_debug(debug);
```

In debug mode:
- `F12` toggles debug
- `S` opens the state tree
- `A` opens the action log

## Testing

Reducers and effects are easy to test because they are plain functions returning plain data.

```rust
use tui_dispatch::testing::{EffectAssertions, EffectStoreTestHarness};

let mut harness = EffectStoreTestHarness::new(State::default(), reducer);

harness.dispatch_collect(Action::Fetch);
harness.assert_state(|s| s.loading);

let effects = harness.drain_effects();
effects.effects_count(1);
```

## Next Steps

- [Async Patterns](/tui-dispatch/patterns/async/) - tasks, subscriptions, debouncing
- [Event Bus](/tui-dispatch/patterns/event-bus/) - routing, focus, handler responses
- [Tutorial: Fetching Data from an API](/tui-dispatch/tutorials/async-fetch/)
- [Debug Layer](/tui-dispatch/debugging/debug-layer/)
- [Runtime Feature Flags](/tui-dispatch/debugging/feature-flags/) - toggle app features at runtime
- [Examples](/tui-dispatch/examples/)
- [FAQ](/tui-dispatch/reference/faq/) - common questions
