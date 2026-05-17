# tui-dispatch

Centralized state management for Rust TUI apps (ratatui + crossterm). Redux/Elm patterns: actions describe events, reducers mutate state, UI renders from state.

## Quick Start

```toml
[dependencies]
tui-dispatch = "0.7.1"
ratatui = "0.30"
crossterm = "0.29"
```

Minimal counter:

```rust
use std::io;
use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::execute;
use ratatui::{backend::CrosstermBackend, widgets::Paragraph, Terminal};
use tui_dispatch::prelude::*;

#[derive(Default)]
struct State { count: i32 }

#[derive(Action, Clone, Debug)]
enum Action { Inc, Dec, Quit }

fn reducer(state: &mut State, action: Action) -> ReducerResult {
    match action {
        Action::Inc => { state.count += 1; ReducerResult::changed() }
        Action::Dec => { state.count -= 1; ReducerResult::changed() }
        Action::Quit => ReducerResult::unchanged(),
    }
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut store = Store::new(State::default(), reducer);

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
            if matches!(&action, Action::Quit) { break; }
            store.dispatch(action);
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
```

That's the core: State, Action, reducer, Store.

## Add What You Need

tui-dispatch is layered. Start with the core, add extensions when needed.
If you want the bare minimum, use `tui-dispatch-core`; `tui-dispatch` is a batteries-included facade.

| Extension | When to use |
|-----------|-------------|
| **Effects** | Async operations (HTTP, file I/O) |
| **EventBus** | Multiple focusable components |
| **TaskManager** | Task cancellation, debouncing |
| **Subscriptions** | Timers, streams |
| **Debug overlay** | State/action inspection (F12) |

## Async and Side Effects

When you need async work (HTTP, file IO, timers), use the same store/runtime
model with an effect type:

- Reducer returns `ReducerResult<Effect>`
- Reducer emits `Effect` values (data), and an effect handler executes them
- Async completion sends a normal action back into the runtime (often named `Did*`)

Enable helpers:

- `features = ["tasks"]` for cancellation + debouncing via `TaskManager`
- `features = ["subscriptions"]` for continuous sources (interval ticks, streams)

See `docs/src/content/docs/patterns/async.md` and the `github-lookup-example` app.

## Examples

```bash
cargo run -p counter-example
cargo run -p github-lookup-example
cargo run -p log-viewer-example --bin log-viewer -- --help
cargo run -p md-preview-example --bin mdpreview -- README.md --debug
cargo run -p minesweeper-example
```

For more complete apps, see [dmk/tui-stuff](https://github.com/dmk/tui-stuff).

## Documentation

- Docs (Starlight): `docs/` (run `make docs-serve`)
- EventBus guide: `docs/src/content/docs/patterns/event-bus.md`
- API docs: https://docs.rs/tui-dispatch

## Crates

- `tui-dispatch`: re-exports + prelude
- `tui-dispatch-core`: store/runtime/tasks/subscriptions/testing primitives
- `tui-dispatch-macros`: derives (`Action`, `DebugState`, `FeatureFlags`, ...)
- `tui-dispatch-components`: reusable components (SelectList, TextInput, TreeView, ...)
- `tui-dispatch-debug`: debug overlay + headless debug sessions

## License

MIT
