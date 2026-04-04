use std::io;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use log_viewer_example::ingest::{resolve_input_mode, spawn_ingest, usage, InputMode};
use log_viewer_example::ui::components::{
    FilterPane, FilterPaneProps, LogDetails, LogDetailsProps,
};
use log_viewer_example::ui::{render_app, MountedViews};
use log_viewer_example::{
    default_keybindings, global_commands, reducer, Action, AppBindingContext, AppState, RouteId,
};
use ratatui::{backend::CrosstermBackend, text::Line, Terminal};
use tui_dispatch::prelude::{process_raw_event, EventBus, RawEvent, Store};
use tui_dispatch_components::{
    ComponentHost, SelectList, SelectListBehavior, SelectListProps, SelectListStyle, StatusBar,
};
use tui_dispatch_debug::debug::DebugLayer;
use tui_dispatch_debug::DebugCliArgs;

/// Log viewer — structured log viewer for the terminal
#[derive(Parser, Debug)]
#[command(name = "log-viewer")]
struct Args {
    /// Log file to view (omit to read from stdin)
    file: Option<String>,

    #[command(flatten)]
    debug: DebugCliArgs,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    let positional: Vec<String> = args.file.into_iter().collect();
    let mode = match resolve_input_mode(&positional)? {
        Some(mode) => mode,
        None => {
            eprintln!(
                "{}",
                usage(
                    &std::env::args()
                        .next()
                        .unwrap_or_else(|| "log-viewer".to_string())
                )
            );
            std::process::exit(1);
        }
    };

    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    let result = run_app(&mut terminal, mode, &args.debug);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    mode: InputMode,
    debug_args: &DebugCliArgs,
) -> io::Result<()> {
    let host = ComponentHost::<AppState, Action, RouteId, AppBindingContext>::new();
    let mounted = MountedViews {
        levels: host.mount(FilterPane::new, level_filter_props),
        tags: host.mount(FilterPane::new, tag_filter_props),
        logs: host.mount(SelectList::new, log_list_props),
        details: host.mount(LogDetails::new, details_props),
    };

    let mut bus = EventBus::<AppState, Action, RouteId, AppBindingContext>::new();
    host.bind(&mut bus, RouteId::Levels, mounted.levels);
    host.bind(&mut bus, RouteId::Tags, mounted.tags);
    host.bind(&mut bus, RouteId::Logs, mounted.logs);
    host.bind(&mut bus, RouteId::Details, mounted.details);
    bus.register_global(global_commands);

    let bindings = default_keybindings();
    let (tx, rx) = mpsc::channel::<Vec<String>>();
    let _reader = spawn_ingest(mode.clone(), tx)?;
    let mut store = Store::new(AppState::new(mode.source_label()), reducer);
    let mut status_bar = StatusBar::new();
    let mut debug = DebugLayer::<Action>::simple()
        .with_component_host(&host)
        .with_action_log_filter(debug_args.action_filter())
        .active(debug_args.enabled);

    loop {
        drain_ingest(&mut store, &rx);

        terminal.draw(|frame| {
            debug.render_state(frame, store.state(), |f, app_area| {
                render_app(f, app_area, store.state(), &host, mounted, &mut status_bar);
            });
        })?;
        host.sync_areas(&mut bus);

        if !event::poll(Duration::from_millis(75))? {
            continue;
        }

        let Some(raw) = read_terminal_event()? else {
            continue;
        };
        let event_kind = process_raw_event(raw);

        // Debug layer intercepts first when active
        let dbg = debug.handle_event_with_state(&event_kind, store.state());
        if dbg.consumed {
            for action in dbg.queued_actions {
                store.dispatch(action);
            }
            continue;
        }

        let outcome = bus.handle_event(&event_kind, store.state(), &bindings);

        let mut keep_running = true;
        for action in outcome.actions {
            debug.log_action(&action);
            keep_running = store.dispatch(action);
            if !keep_running {
                break;
            }
        }

        if !keep_running {
            break;
        }
    }

    Ok(())
}

fn drain_ingest(store: &mut Store<AppState, Action>, rx: &Receiver<Vec<String>>) {
    let mut pending = Vec::new();
    while let Ok(mut batch) = rx.try_recv() {
        pending.append(&mut batch);
    }

    if !pending.is_empty() {
        store.dispatch(Action::LogsAppended(pending));
    }
}

fn read_terminal_event() -> io::Result<Option<RawEvent>> {
    Ok(match event::read()? {
        Event::Key(key) => Some(RawEvent::Key(key)),
        Event::Mouse(mouse) => Some(RawEvent::Mouse(mouse)),
        Event::Resize(width, height) => Some(RawEvent::Resize(width, height)),
        _ => None,
    })
}

fn level_filter_props(state: &AppState) -> FilterPaneProps<'_, Action> {
    FilterPaneProps {
        title: "Levels",
        options: &state.level_options,
        active: &state.active_levels,
        is_focused: state.focus == log_viewer_example::Focus::Levels,
        on_toggle: Action::ToggleLevel,
    }
}

fn tag_filter_props(state: &AppState) -> FilterPaneProps<'_, Action> {
    FilterPaneProps {
        title: "Tags",
        options: &state.tag_options,
        active: &state.active_tags,
        is_focused: state.focus == log_viewer_example::Focus::Tags,
        on_toggle: Action::ToggleTag,
    }
}

fn log_list_props(state: &AppState) -> SelectListProps<'_, Line<'static>, Action> {
    SelectListProps {
        items: &state.visible_lines,
        count: state.visible_lines.len(),
        selected: state.selected_visible.unwrap_or(0),
        is_focused: state.focus == log_viewer_example::Focus::Logs,
        style: log_list_style(),
        behavior: SelectListBehavior {
            wrap_navigation: false,
            ..Default::default()
        },
        on_select: Action::SelectLog,
        render_item: &clone_line,
    }
}

fn details_props(state: &AppState) -> LogDetailsProps<'_> {
    LogDetailsProps {
        entry: state.opened_entry(),
        is_focused: state.focus == log_viewer_example::Focus::Details,
    }
}

fn clone_line(line: &Line<'static>) -> Line<'static> {
    line.clone()
}

fn log_list_style() -> SelectListStyle {
    let mut style = SelectListStyle::default();
    style.base.bg = Some(ratatui::style::Color::Rgb(14, 17, 21));
    style.base.border = Some(tui_dispatch_components::BorderStyle {
        style: ratatui::style::Style::default().fg(ratatui::style::Color::Rgb(64, 80, 92)),
        focused_style: Some(
            ratatui::style::Style::default().fg(ratatui::style::Color::Rgb(123, 203, 216)),
        ),
        ..tui_dispatch_components::BorderStyle::all()
    });
    style.selection.style =
        Some(ratatui::style::Style::default().bg(ratatui::style::Color::Rgb(25, 35, 43)));
    style.selection.marker = Some("› ");
    style
}
