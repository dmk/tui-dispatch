pub mod components;
mod render;

use tui_dispatch_components::{Mounted, SelectList, StatusBar};

pub use render::render_app;

use crate::ui::components::{FilterPane, LogDetails};

#[derive(Clone, Copy)]
pub struct MountedViews {
    pub levels: Mounted<FilterPane>,
    pub tags: Mounted<FilterPane>,
    pub logs: Mounted<SelectList>,
    pub details: Mounted<LogDetails>,
}

pub type AppStatusBar = StatusBar;
