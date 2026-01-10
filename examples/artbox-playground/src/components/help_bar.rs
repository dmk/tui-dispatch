use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use tui_dispatch::Component;

use crate::action::{Action, Panel};

pub struct HelpBarProps {
    pub focused_panel: Panel,
    pub show_help: bool,
}

#[derive(Default)]
pub struct HelpBar;

impl Component<Action> for HelpBar {
    type Props<'a> = HelpBarProps;

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        let key_style = Style::default().fg(Color::Yellow);
        let sep_style = Style::default().fg(Color::DarkGray);

        let common_keys = [("Tab", "next panel"), ("?", "help"), ("q/Esc", "quit")];

        let panel_keys: Vec<(&str, &str)> = match props.focused_panel {
            Panel::TextInput => vec![("type", "edit text"), ("Backspace", "delete")],
            Panel::FontSelector => vec![("←/→", "change font")],
            Panel::ColorPicker => vec![("↑/↓", "channel"), ("←/→", "adjust"), ("m", "mode")],
            Panel::GradientEditor => vec![("↑/↓", "field"), ("←/→", "adjust"), ("+/-", "stops")],
            Panel::AlignmentGrid => vec![("1-9", "position"), ("qweasdzxc", "position")],
            Panel::SpacingControl => vec![("←/→", "adjust"), ("0", "reset")],
            Panel::PresetPanel => vec![("↑/↓", "select"), ("Enter", "load"), ("s", "save")],
            Panel::Preview => vec![("click", "set center"), ("c", "copy")],
        };

        let mut spans = Vec::new();

        for (key, desc) in panel_keys.iter().chain(common_keys.iter()) {
            spans.push(Span::styled(*key, key_style));
            spans.push(Span::raw(":"));
            spans.push(Span::raw(*desc));
            spans.push(Span::styled(" | ", sep_style));
        }

        // Remove trailing separator
        if spans.len() >= 3 {
            spans.truncate(spans.len() - 1);
        }

        let line = Line::from(spans);
        let paragraph = Paragraph::new(line);
        frame.render_widget(paragraph, area);
    }
}
