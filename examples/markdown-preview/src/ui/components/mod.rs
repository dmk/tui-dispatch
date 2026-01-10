pub mod content_view;
pub mod status_bar;
pub mod title_bar;

// Re-export core Component trait
pub use tui_dispatch::Component;

pub use content_view::{ContentView, ContentViewProps};
pub use status_bar::{StatusBar, StatusBarProps};
pub use title_bar::{TitleBar, TitleBarProps};
