pub mod action;
pub mod ingest;
pub mod reducer;
pub mod state;
pub mod ui;

use tui_dispatch::prelude::{BindingContext, HandlerResponse, Keybindings, RoutedEvent};

pub use action::Action;
pub use reducer::reducer;
pub use state::{AppState, Focus};

#[derive(tui_dispatch::ComponentId, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RouteId {
    Levels,
    Tags,
    Logs,
    Details,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AppBindingContext {
    #[default]
    Filter,
    Logs,
    Details,
}

impl BindingContext for AppBindingContext {
    fn name(&self) -> &'static str {
        match self {
            Self::Filter => "filter",
            Self::Logs => "logs",
            Self::Details => "details",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "filter" => Some(Self::Filter),
            "logs" => Some(Self::Logs),
            "details" => Some(Self::Details),
            _ => None,
        }
    }

    fn all() -> &'static [Self] {
        &[Self::Filter, Self::Logs, Self::Details]
    }
}

pub fn default_keybindings() -> Keybindings<AppBindingContext> {
    let mut bindings = Keybindings::new();
    bindings.add_global("next_focus", vec!["tab".into()]);
    bindings.add_global("prev_focus", vec!["shift+tab".into()]);
    bindings.add_global("toggle_follow", vec!["f".into()]);
    bindings.add_global("close_details", vec!["esc".into()]);
    bindings.add_global("quit", vec!["q".into()]);

    bindings.add(
        AppBindingContext::Logs,
        "next",
        vec!["j".into(), "down".into()],
    );
    bindings.add(
        AppBindingContext::Logs,
        "prev",
        vec!["k".into(), "up".into()],
    );
    bindings.add(
        AppBindingContext::Logs,
        "first",
        vec!["g".into(), "home".into()],
    );
    bindings.add(
        AppBindingContext::Logs,
        "last",
        vec!["G".into(), "end".into()],
    );
    bindings.add(
        AppBindingContext::Logs,
        "open_details",
        vec!["enter".into()],
    );

    bindings.add(AppBindingContext::Details, "toggle_view", vec!["v".into()]);

    bindings
}

pub fn global_commands(
    event: RoutedEvent<'_, RouteId, AppBindingContext>,
    state: &AppState,
) -> HandlerResponse<Action> {
    match event.command {
        Some("next_focus") => HandlerResponse::action(Action::FocusNext),
        Some("prev_focus") => HandlerResponse::action(Action::FocusPrev),
        Some("toggle_follow") => HandlerResponse::action(Action::ToggleFollow),
        Some("open_details") if state.focus == Focus::Logs => {
            HandlerResponse::action(Action::OpenSelectedLog)
        }
        Some("close_details") if state.details_open() => {
            HandlerResponse::action(Action::CloseDetails)
        }
        Some("quit") => HandlerResponse::action(Action::Quit),
        _ => HandlerResponse::ignored(),
    }
}
