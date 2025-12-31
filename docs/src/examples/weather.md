# Weather Example

A weather TUI that demonstrates the core tui-dispatch patterns with async API calls.

## Running

```bash
# Default city (Kyiv)
cargo run -p weather-example

# Custom city
cargo run -p weather-example -- --city London
```

## What It Shows

### Action Categories

The example uses `#[action(infer_categories)]` to automatically group actions:

```rust
#[derive(tui_dispatch::Action, Clone, Debug, PartialEq)]
#[action(infer_categories)]
pub enum Action {
    // Category: "weather" (inferred from prefix)
    WeatherFetch,
    WeatherDidLoad(WeatherData),
    WeatherDidError(String),

    // Category: "ui" (inferred from prefix)
    UiToggleUnits,
    UiTerminalResize(u16, u16),

    // Uncategorized (global)
    Tick,
    Quit,
}
```

### Async Pattern

Weather fetching follows the intent → result pattern:

1. `WeatherFetch` - Intent action triggers async task
2. API call runs in a spawned tokio task
3. `WeatherDidLoad` or `WeatherDidError` - Result action updates state

```rust
// In the main loop
if matches!(action, Action::WeatherFetch) {
    let tx = action_tx.clone();
    tokio::spawn(async move {
        match api::fetch_weather(lat, lon).await {
            Ok(data) => tx.send(Action::WeatherDidLoad(data)),
            Err(e) => tx.send(Action::WeatherDidError(e.to_string())),
        }
    });
}
```

### Main Loop Structure

The example shows the standard tui-dispatch main loop:

```rust
loop {
    // 1. Render if state changed
    if should_render {
        terminal.draw(|frame| {
            weather_display.render(frame, frame.area(), props);
        })?;
        should_render = false;
    }

    // 2. Wait for events or actions
    tokio::select! {
        Some(raw_event) = event_rx.recv() => {
            // Convert to actions via component
            let actions = component.handle_event(&event_kind, props);
            for action in actions {
                action_tx.send(action);
            }
        }
        Some(action) = action_rx.recv() => {
            // Dispatch to store
            let state_changed = store.dispatch(action);
            should_render = state_changed;
        }
    }
}
```

## Keybindings

| Key | Action |
|-----|--------|
| `r` / `F5` | Refresh weather |
| `u` | Toggle units (°C/°F) |
| `q` / `Esc` | Quit |

## Key Files

| File | Purpose |
|------|---------|
| `src/main.rs` | Entry point, event loop, terminal setup |
| `src/action.rs` | Action enum with category inference |
| `src/state.rs` | AppState, Location, WeatherData types |
| `src/reducer.rs` | State mutation logic |
| `src/api.rs` | Open-Meteo API client |
| `src/components/weather_display.rs` | UI component |
