pub mod content_view;
pub mod status_bar;
pub mod title_bar;

use ratatui::{Frame, layout::Rect};

/// Base trait for UI components in this example.
pub trait Component {
    type Props<'a>;

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>);
}

pub use content_view::{ContentView, ContentViewProps};
pub use status_bar::{StatusBar, StatusBarProps};
pub use title_bar::{TitleBar, TitleBarProps};
