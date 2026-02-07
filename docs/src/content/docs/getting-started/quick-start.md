---
title: Quick Start
description: Get started with tui-dispatch in minutes
---

This page gets you to a running app quickly, using the runtime helpers that ship with tui-dispatch.

## Install

Add the basics:

```toml
[dependencies]
tui-dispatch = "0.5.4"
ratatui = "0.29"
crossterm = "0.28"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time"] }
```

If you know you'll be doing async work, enable the runtime helpers:

```toml
[dependencies]
tui-dispatch = { version = "0.5.4", features = ["tasks", "subscriptions"] }
```

- `tasks`: `TaskManager` (cancellation + debounce)
- `subscriptions`: timers/streams that emit actions

## Minimal Example: Counter

The core pattern: Store + reducer + your own event loop.

```rust
use std::io;
use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::execute;
use ratatui::{backend::CrosstermBackend, widgets::Paragraph, Terminal, Frame};
use tui_dispatch::prelude::*;

#[derive(Default)]
struct AppState { count: i32 }

#[derive(Action, Clone, Debug)]
enum Action { Inc, Dec, Quit }

fn reducer(state: &mut AppState, action: Action) -> bool {
    match action {
        Action::Inc => { state.count += 1; true }
        Action::Dec => { state.count -= 1; true }
        Action::Quit => false,
    }
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    let mut store = Store::new(AppState::default(), reducer);

    loop {
        terminal.draw(|f| {
            f.render_widget(
                Paragraph::new(format!("count = {}  (k/j, q)", store.state().count)),
                f.area(),
            );
        })?;

        if let Event::Key(key) = event::read()? {
            let action = match key.code {
                KeyCode::Char('k') | KeyCode::Up => Action::Inc,
                KeyCode::Char('j') | KeyCode::Down => Action::Dec,
                KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
                _ => continue,
            };
            if !store.dispatch(action) { break; }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
```

That's the core: State, Action, reducer, Store.

> **Next:** [Tutorial: Fetching Data from an API](/tui-dispatch/tutorials/async-fetch/) builds a complete async app and explains the mental model.

## When You Need Async: Effects + TaskManager

The recommended pattern is:

- Intent action (user asks for work) -> reducer returns an `Effect`
- Effect handler spawns async work
- Async completion dispatches a normal action back into the runtime

Enable `tasks`:

```toml
tui-dispatch = { version = "0.5.4", features = ["tasks"] }
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
