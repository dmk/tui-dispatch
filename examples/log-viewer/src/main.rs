use std::{io, rc::Rc};

use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use log_viewer_example::ingest::{resolve_input_mode, spawn_ingest, usage, InputMode};
use log_viewer_example::ui::components::{
    FilterPane, FilterPaneProps, FilterToggleCallback, LogDetails, LogDetailsProps,
};
use log_viewer_example::ui::{render_app, MountedViews};
use log_viewer_example::{
    default_keybindings, global_commands, reducer, Action, AppBindingContext, AppState, RouteId,
};
use ratatui::{backend::CrosstermBackend, text::Line, Terminal};
use tokio::sync::mpsc;
use tui_dispatch::prelude::{EventBus, Runtime};
use tui_dispatch_components::{
    ComponentHost, PropsFactory, RuntimeHostExt, SelectList, SelectListBehavior,
    SelectListCallback, SelectListProps, SelectListStyle, StatusBar,
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

#[tokio::main]
async fn main() -> io::Result<()> {
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

    let result = run_app(&mut terminal, mode, &args.debug).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

async fn run_app<B>(
    terminal: &mut Terminal<B>,
    mode: InputMode,
    debug_args: &DebugCliArgs,
) -> io::Result<()>
where
    B: ratatui::backend::Backend,
    B::Error: Send + Sync + 'static,
{
    let host = ComponentHost::<AppState, Action, RouteId, AppBindingContext>::new();
    let level_toggle = Rc::new(Action::ToggleLevel);
    let tag_toggle = Rc::new(Action::ToggleTag);
    let log_select = Rc::new(Action::SelectLog);
    let mounted = MountedViews {
        levels: host.mount(
            FilterPane::new,
            LevelFilterPropsFactory {
                on_toggle: level_toggle,
            },
        ),
        tags: host.mount(
            FilterPane::new,
            TagFilterPropsFactory {
                on_toggle: tag_toggle,
            },
        ),
        logs: host.mount(
            SelectList::new,
            LogListPropsFactory {
                on_select: log_select,
            },
        ),
        details: host.mount(LogDetails::new, details_props),
    };

    let mut bus = EventBus::<AppState, Action, RouteId, AppBindingContext>::new();
    host.bind(&mut bus, RouteId::Levels, mounted.levels);
    host.bind(&mut bus, RouteId::Tags, mounted.tags);
    host.bind(&mut bus, RouteId::Logs, mounted.logs);
    host.bind(&mut bus, RouteId::Details, mounted.details);
    bus.register_global(global_commands);

    let bindings = default_keybindings();

    let debug_layer = DebugLayer::<Action>::simple()
        .with_component_host(&host)
        .with_action_log_filter(debug_args.action_filter())
        .active(debug_args.enabled);

    let mut runtime = Runtime::new(AppState::new(mode.source_label()), reducer)
        .with_debug(debug_layer)
        .with_event_bus(bus, bindings)
        .with_component_host(host.clone());

    // Forward ingested log batches into the action queue.
    let (ingest_tx, mut ingest_rx) = mpsc::unbounded_channel::<Vec<String>>();
    let _reader = spawn_ingest(mode.clone(), ingest_tx)?;
    let action_tx = runtime.action_tx();
    tokio::spawn(async move {
        while let Some(batch) = ingest_rx.recv().await {
            if action_tx.send(Action::LogsAppended(batch)).is_err() {
                break;
            }
        }
    });

    let mut status_bar = StatusBar::new();
    let host_for_render = host.clone();

    runtime
        .run(
            terminal,
            |frame, area, state, _render_ctx, _event_ctx| {
                render_app(
                    frame,
                    area,
                    state,
                    &host_for_render,
                    mounted,
                    &mut status_bar,
                );
            },
            |action| matches!(action, Action::Quit),
        )
        .await
}

struct LevelFilterPropsFactory {
    on_toggle: FilterToggleCallback<Action>,
}

impl PropsFactory<AppState, FilterPane, Action, AppBindingContext> for LevelFilterPropsFactory {
    fn props<'a>(&self, state: &'a AppState) -> FilterPaneProps<'a, Action> {
        level_filter_props(state, self.on_toggle.clone())
    }
}

struct TagFilterPropsFactory {
    on_toggle: FilterToggleCallback<Action>,
}

impl PropsFactory<AppState, FilterPane, Action, AppBindingContext> for TagFilterPropsFactory {
    fn props<'a>(&self, state: &'a AppState) -> FilterPaneProps<'a, Action> {
        tag_filter_props(state, self.on_toggle.clone())
    }
}

struct LogListPropsFactory {
    on_select: SelectListCallback<Action>,
}

impl PropsFactory<AppState, SelectList<Line<'static>>, Action, AppBindingContext>
    for LogListPropsFactory
{
    fn props<'a>(&self, state: &'a AppState) -> SelectListProps<'a, Line<'static>, Action> {
        log_list_props(state, self.on_select.clone())
    }
}

fn level_filter_props<'a>(
    state: &'a AppState,
    on_toggle: FilterToggleCallback<Action>,
) -> FilterPaneProps<'a, Action> {
    FilterPaneProps {
        title: "Levels",
        options: &state.level_options,
        active: &state.active_levels,
        is_focused: state.focus == log_viewer_example::Focus::Levels,
        on_toggle,
    }
}

fn tag_filter_props<'a>(
    state: &'a AppState,
    on_toggle: FilterToggleCallback<Action>,
) -> FilterPaneProps<'a, Action> {
    FilterPaneProps {
        title: "Tags",
        options: &state.tag_options,
        active: &state.active_tags,
        is_focused: state.focus == log_viewer_example::Focus::Tags,
        on_toggle,
    }
}

fn log_list_props<'a>(
    state: &'a AppState,
    on_select: SelectListCallback<Action>,
) -> SelectListProps<'a, Line<'static>, Action> {
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
        on_select,
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
