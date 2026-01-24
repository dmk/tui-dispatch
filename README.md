# tui-dispatch

Centralized state management for Rust TUI apps (ratatui + crossterm). Think Redux/Elm: terminal events become actions, reducers mutate state, and the UI renders from state.

- Predictable updates: all state mutations live in your reducer.
- Testable by construction: reducers and emitted effects are plain data.
- Ergonomic runtime: `DispatchRuntime` / `EffectRuntime` run the event loop.
- Debuggable: built-in debug overlay (F12) for state + action inspection.

## Quick Start

Add dependencies:

```toml
[dependencies]
tui-dispatch = "0.5.3"
ratatui = "0.29"
crossterm = "0.28"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time"] }
```

Minimal counter app (copy-paste-run):

```rust
use std::io;

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, widgets::Paragraph, Terminal};
use tui_dispatch::prelude::*;

#[derive(Default)]
struct State {
    count: i32,
}

#[derive(Action, Clone, Debug)]
enum Action {
    Inc,
    Dec,
    Quit,
}

fn reducer(state: &mut State, action: Action) -> bool {
    match action {
        Action::Inc => {
            state.count += 1;
            true
        }
        Action::Dec => {
            state.count -= 1;
            true
        }
        Action::Quit => false, // quit is handled by the runtime predicate
    }
}

fn render(frame: &mut Frame, area: Rect, state: &State, _ctx: RenderContext) {
    frame.render_widget(
        Paragraph::new(format!("count = {}  (j/k, q)", state.count)),
        area,
    );
}

fn map_event(event: &EventKind, _state: &State) -> Option<Action> {
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

    let mut runtime = DispatchRuntime::new(State::default(), reducer);
    let result = runtime
        .run(&mut terminal, render, map_event, |a| matches!(a, Action::Quit))
        .await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}
```

> **Note:** The runtime helpers (`DispatchRuntime`, `EffectRuntime`) require a Tokio runtime. If you need a different async runtime, use the lower-level `Store`/`EffectStore` directly.

## Async And Side Effects

When you need async work (HTTP, file IO, timers), switch to the effect pattern:

- Reducer returns `DispatchResult<Effect>` instead of `bool`
- Reducer emits `Effect` values (data), and an effect handler executes them
- Async completion sends a normal action back into the runtime (often named `Did*`)

Enable helpers:

- `features = ["tasks"]` for cancellation + debouncing via `TaskManager`
- `features = ["subscriptions"]` for continuous sources (interval ticks, streams)

See `docs/src/async.md` and the `weather-example` / `github-lookup-example` apps.

## Examples (In This Repo)

```bash
cargo run -p counter
cargo run -p github-lookup-example
cargo run -p weather-example -- --city London --debug
cargo run -p markdown-preview -- README.md --debug
```

## Documentation

- Book (mdBook): `docs/` (run `make docs-serve`)
- API docs: https://docs.rs/tui-dispatch

## Crates

- `tui-dispatch`: re-exports + prelude
- `tui-dispatch-core`: store/runtime/tasks/subscriptions/testing primitives
- `tui-dispatch-macros`: derives (`Action`, `DebugState`, `FeatureFlags`, ...)
- `tui-dispatch-components`: reusable components (SelectList, TextInput, TreeView, ...)
- `tui-dispatch-debug`: debug overlay + headless debug sessions

## Real-World Usage

Used in production by [memtui](https://github.com/dmk/memtui), a TUI for Redis/Memcached/etcd.

## License

MIT
