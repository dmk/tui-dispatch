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
2. API call runs in a TaskManager task via the effect handler
3. `WeatherDidLoad` or `WeatherDidError` - Result action updates state

```rust
fn handle_effect(effect: Effect, ctx: &mut EffectContext<Action>) {
    match effect {
        Effect::FetchWeather { lat, lon } => {
            ctx.tasks().spawn("weather", async move {
                match api::fetch_weather(lat, lon).await {
                    Ok(data) => Action::WeatherDidLoad(data),
                    Err(e) => Action::WeatherDidError(e.to_string()),
                }
            });
        }
    }
}
```

### Main Loop Structure

The example uses the `EffectRuntime` helper:

```rust
let mut runtime = EffectRuntime::from_store(store)
    .with_debug(DebugLayer::simple().active(debug_enabled));

runtime
    .run(
        terminal,
        |frame, area, state, render_ctx| render(frame, area, state, render_ctx),
        |event, state| {
            if let EventKind::Resize(width, height) = event {
                return EventOutcome::action(Action::UiTerminalResize(*width, *height))
                    .with_render();
            }
            EventOutcome::from(component.handle_event(event, props))
        },
        |action| matches!(action, Action::Quit),
        |effect, ctx| handle_effect(effect, ctx),
    )
    .await?;
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
