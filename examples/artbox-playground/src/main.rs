use std::cell::RefCell;
use std::io;
use std::time::Duration;

use artbox::{Fill, Renderer};
use clap::Parser;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use tui_dispatch::debug::DebugLayer;
use tui_dispatch::{
    Component, EffectContext, EffectRuntime, EffectStoreWithMiddleware, EventKind, EventOutcome,
    RenderContext,
};

use artbox_playground::action::Action;
use artbox_playground::components::Playground;
use artbox_playground::components::playground::PlaygroundProps;
use artbox_playground::effect::{Effect, preset_dir};
use artbox_playground::reducer::reducer;
use artbox_playground::state::{AppState, FontFamily};

// Thread-local storage for the Playground component to persist state between events
thread_local! {
    static PLAYGROUND: RefCell<Playground> = RefCell::new(Playground::default());
}

#[derive(Parser, Debug)]
#[command(name = "artbox-playground")]
#[command(about = "Interactive ASCII art playground with gradients and effects")]
struct Args {
    /// Enable debug overlay (F12 to toggle)
    #[arg(long, default_value_t = false)]
    debug: bool,

    /// Initial text to render
    #[arg(short, long, default_value = "Hello")]
    text: String,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;

    // Run the app
    let result = run_app(terminal, args);

    // Restore terminal
    disable_raw_mode()?;
    crossterm::execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;

    result
}

#[tokio::main]
async fn run_app(
    mut terminal: Terminal<CrosstermBackend<io::Stdout>>,
    args: Args,
) -> io::Result<()> {
    // Initialize state
    let mut state = AppState {
        text: args.text,
        ..Default::default()
    };

    // Get terminal size
    let size = terminal.size()?;
    state.terminal_size = (size.width, size.height);

    // Create store
    let store = EffectStoreWithMiddleware::new(state, reducer, tui_dispatch::NoopMiddleware);

    // Create debug layer
    let debug = DebugLayer::simple().active(args.debug);

    // Create runtime
    let mut runtime = EffectRuntime::from_store(store).with_debug(debug);

    // Setup tick subscription for animations and status message dismissal
    runtime
        .subscriptions()
        .interval("tick", Duration::from_millis(100), || Action::Tick);

    // Load presets on startup
    runtime.enqueue(Action::PresetRefresh);

    // Run the main loop
    runtime
        .run(
            &mut terminal,
            render_app,
            map_event,
            |action| matches!(action, Action::Quit),
            handle_effect,
        )
        .await?;

    Ok(())
}

fn render_app(frame: &mut ratatui::Frame, area: Rect, state: &AppState, _ctx: RenderContext) {
    PLAYGROUND.with_borrow_mut(|playground| {
        playground.render(frame, area, PlaygroundProps { state });
    });
}

fn map_event(event: &EventKind, state: &AppState) -> EventOutcome<Action> {
    // Handle resize specially
    if let EventKind::Resize(width, height) = event {
        return EventOutcome::action(Action::UiTerminalResize(*width, *height));
    }

    // Route all other events through the Playground component
    let actions = PLAYGROUND
        .with_borrow_mut(|playground| playground.handle_event(event, PlaygroundProps { state }));

    if actions.is_empty() {
        EventOutcome::ignored()
    } else {
        EventOutcome::actions(actions)
    }
}

fn handle_effect(effect: Effect, ctx: &mut EffectContext<Action>) {
    match effect {
        Effect::SavePreset { name, preset } => {
            ctx.tasks().spawn("save_preset", async move {
                let dir = preset_dir();
                if let Err(e) = std::fs::create_dir_all(&dir) {
                    return Action::PresetDidSaveError(e.to_string());
                }
                let path = dir.join(format!("{}.json", name));
                match serde_json::to_string_pretty(&preset) {
                    Ok(json) => match std::fs::write(&path, json) {
                        Ok(_) => Action::PresetDidSave(name),
                        Err(e) => Action::PresetDidSaveError(e.to_string()),
                    },
                    Err(e) => Action::PresetDidSaveError(e.to_string()),
                }
            });
        }
        Effect::LoadPreset { name } => {
            ctx.tasks().spawn("load_preset", async move {
                let path = preset_dir().join(format!("{}.json", name));
                match std::fs::read_to_string(&path) {
                    Ok(content) => match serde_json::from_str(&content) {
                        Ok(preset) => Action::PresetDidLoad(preset),
                        Err(e) => Action::PresetDidLoadError(e.to_string()),
                    },
                    Err(e) => Action::PresetDidLoadError(e.to_string()),
                }
            });
        }
        Effect::DeletePreset { name } => {
            ctx.tasks().spawn("delete_preset", async move {
                let path = preset_dir().join(format!("{}.json", name));
                let _ = std::fs::remove_file(&path);
                // Refresh the list after deletion
                let names = list_presets();
                Action::PresetDidRefresh(names)
            });
        }
        Effect::RefreshPresets => {
            ctx.tasks().spawn("refresh_presets", async move {
                let names = list_presets();
                Action::PresetDidRefresh(names)
            });
        }
        Effect::ExportClipboard {
            text,
            font_family,
            fill,
            alignment,
            letter_spacing,
        } => {
            ctx.tasks().spawn("export_clipboard", async move {
                match render_to_ansi(&text, font_family, &fill, alignment, letter_spacing) {
                    Ok(ansi) => {
                        match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(ansi)) {
                            Ok(_) => Action::ExportDidClipboard,
                            Err(e) => Action::ExportDidError(e.to_string()),
                        }
                    }
                    Err(e) => Action::ExportDidError(e),
                }
            });
        }
        Effect::ExportFile {
            path,
            text,
            font_family,
            fill,
            alignment,
            letter_spacing,
        } => {
            ctx.tasks().spawn("export_file", async move {
                match render_to_ansi(&text, font_family, &fill, alignment, letter_spacing) {
                    Ok(ansi) => match std::fs::write(&path, ansi) {
                        Ok(_) => Action::ExportDidFile(path),
                        Err(e) => Action::ExportDidError(e.to_string()),
                    },
                    Err(e) => Action::ExportDidError(e),
                }
            });
        }
    }
}

fn list_presets() -> Vec<String> {
    let dir = preset_dir();
    std::fs::read_dir(&dir)
        .ok()
        .map(|entries| {
            let mut names: Vec<String> = entries
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    name.strip_suffix(".json").map(|s| s.to_string())
                })
                .collect();
            names.sort();
            names
        })
        .unwrap_or_default()
}

fn render_to_ansi(
    text: &str,
    font_family: FontFamily,
    fill: &Fill,
    alignment: artbox::Alignment,
    letter_spacing: i16,
) -> Result<String, String> {
    let fonts = artbox::fonts::family(font_family.name()).unwrap_or_default();
    let renderer = Renderer::new(fonts)
        .with_plain_fallback()
        .with_alignment(alignment)
        .with_letter_spacing(letter_spacing)
        .with_fill(fill.clone());

    // Render to a reasonable size
    let result = renderer
        .render_styled(text, 120, 40)
        .map_err(|e| format!("{:?}", e))?;

    Ok(result.to_ansi_string())
}
