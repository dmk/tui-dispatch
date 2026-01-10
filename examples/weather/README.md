# Weather Example

A weather TUI that demonstrates tui-dispatch patterns with async tasks, effect runtime, and component-driven input.

## Running

From the repository root:

```bash
# Default city (Kyiv)
cargo run -p weather-example

# Custom city, refresh interval (seconds), debug overlay
cargo run -p weather-example -- --city London --refresh-interval 60 --debug
```

From this directory:

```bash
cargo run
```

## Controls

| Key | Action |
|-----|--------|
| `r` / `F5` | Refresh weather |
| `/` | Open city search |
| `u` | Toggle units (C/F) |
| `q` / `Esc` | Quit |

Search overlay:

| Key | Action |
|-----|--------|
| `Enter` | Submit query or confirm selection |
| `Up` / `Down` | Navigate results |
| `Esc` | Close search |

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `--city`, `-c` | `Kyiv` | City name to look up |
| `--refresh-interval`, `-r` | `30` | Auto-refresh interval in seconds |
| `--debug` | `false` | Enable debug overlay (F12) |

## What It Shows

- Action categories inferred from action names.
- Intent -> result async pattern with effects and task manager.
- Auto-refresh subscription plus manual refresh.
- Search overlay for city lookup.

## Data Source

Weather data and geocoding come from the Open-Meteo APIs (no API key required).

## Key Files

| File | Purpose |
|------|---------|
| `src/main.rs` | Entry point, runtime wiring, CLI options |
| `src/action.rs` | Action enum and categories |
| `src/reducer.rs` | State mutation logic |
| `src/effect.rs` | Effect definitions |
| `src/api.rs` | Open-Meteo API client |
| `src/components/weather_display.rs` | Main UI component |
| `src/components/search_overlay.rs` | City search overlay |
| `src/sprites.rs` | Weather sprite art |
