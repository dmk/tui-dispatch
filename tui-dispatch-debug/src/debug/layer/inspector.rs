//! Read-only data adapter for the live debug layer's existing views.
use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    style::Style,
    text::Span,
    widgets::{Block, Paragraph},
    Frame,
};
use serde::Serialize;
use serde_json::Value;
use tui_dispatch_components::StatusBarItem;
use tui_dispatch_core::{Action, EventKind};

use super::DebugLayer;
use crate::debug::{
    ActionLogDisplayEntry, ActionLogOverlay, DebugEntry, DebugOverlay, DebugSection, DebugState,
    DebugStyle,
};

#[derive(Clone, Debug)]
enum InspectorAction {}
impl Action for InspectorAction {
    fn name(&self) -> &'static str {
        match *self {}
    }
}

#[derive(Serialize)]
struct InspectorState(Value);
impl DebugState for InspectorState {
    fn debug_sections(&self) -> Vec<DebugSection> {
        let entries = match &self.0 {
            Value::Object(fields) => fields
                .iter()
                .map(|(key, value)| DebugEntry::new(key, value.to_string()))
                .collect(),
            value => vec![DebugEntry::new("value", value.to_string())],
        };
        vec![DebugSection {
            title: "State".into(),
            entries,
        }]
    }
}

/// Fullscreen read-only inspector using the same state tree, search, syntax
/// colors, scrolling, details, and status bar as [`DebugLayer`].
///
/// Feed it owned state and newest-first action entries. No application actions
/// are dispatched and no task/freeze handles are attached. Updates preserve
/// search/selection; an open detail remains stable until the user goes back.
pub struct DebugInspector {
    layer: DebugLayer<InspectorAction>,
    state: InspectorState,
    actions: ActionLogOverlay,
}

impl Default for DebugInspector {
    fn default() -> Self {
        Self::new()
    }
}

impl DebugInspector {
    /// Start in the action list, ready for state/history supplied by a client.
    pub fn new() -> Self {
        let mut layer = DebugLayer::simple().with_state_snapshots::<InspectorState>();
        layer.overlay_fullscreen = true;
        layer.freeze.enable();
        let actions = ActionLogOverlay::from_entries(Vec::new(), "Action Log");
        layer
            .freeze
            .set_overlay(DebugOverlay::ActionLog(actions.clone()));
        Self {
            layer,
            state: InspectorState(Value::Null),
            actions,
        }
    }

    /// Apply the same palette used to customize a live debug layer.
    pub fn with_style(mut self, style: DebugStyle) -> Self {
        self.layer = self.layer.with_style(style);
        self
    }

    /// Refresh remote/offline data without resetting navigation or search input.
    /// Entries must be in newest-first order, with stable sequence numbers.
    pub fn update(&mut self, state: Value, entries: Vec<ActionLogDisplayEntry>) {
        self.remember_actions();
        self.actions.replace_entries(entries);
        if matches!(self.layer.freeze.overlay, Some(DebugOverlay::ActionLog(_))) {
            self.layer
                .freeze
                .set_overlay(DebugOverlay::ActionLog(self.actions.clone()));
        }
        self.state = InspectorState(state);
        let table = self.state.build_debug_table("Application State");
        if matches!(self.layer.freeze.overlay, Some(DebugOverlay::State(_))) {
            self.layer.set_state_overlay(table);
        } else {
            self.layer.state_snapshot = Some(table);
        }
    }

    /// Current view, including its selection/search state.
    pub fn overlay(&self) -> Option<&DebugOverlay> {
        self.layer.freeze.overlay.as_ref()
    }

    /// Handle shared debug navigation; returns true when the client should quit.
    ///
    /// `a`/`s` switch actions/state, `/` searches, Enter opens details, and
    /// Esc/Backspace return from details. `q` or Ctrl+C quit; Esc also quits from
    /// a list/tree. While editing a query, letters (including q/a/s) are text.
    /// `w` saves the current JSON state using the live layer's local snapshot UI.
    pub fn handle_event(&mut self, event: &EventKind) -> bool {
        self.remember_actions();
        if let EventKind::Key(key) = event {
            if key.kind == KeyEventKind::Release {
                return false;
            }
            if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                return true;
            }
            let searching = match self.layer.freeze.overlay.as_ref() {
                Some(DebugOverlay::ActionLog(log)) => log.search_input_active,
                Some(DebugOverlay::State(_)) => self.layer.state_search_active,
                _ => false,
            };
            if !searching {
                if matches!(key.code, KeyCode::Char('q' | 'Q')) {
                    return true;
                }
                if matches!(
                    self.layer.freeze.overlay,
                    Some(DebugOverlay::ActionDetail(_))
                ) && matches!(key.code, KeyCode::Esc | KeyCode::Backspace | KeyCode::Enter)
                {
                    self.layer
                        .freeze
                        .set_overlay(DebugOverlay::ActionLog(self.actions.clone()));
                    return false;
                }
                if key.code == KeyCode::Esc
                    && !matches!(
                        self.layer.freeze.overlay,
                        Some(DebugOverlay::StateDetail(_))
                    )
                {
                    return true;
                }
                match key.code {
                    KeyCode::Char('a' | 'A') => {
                        if matches!(self.layer.freeze.overlay, Some(DebugOverlay::ActionLog(_))) {
                            self.layer.freeze.clear_overlay();
                        } else {
                            self.layer
                                .freeze
                                .set_overlay(DebugOverlay::ActionLog(self.actions.clone()));
                        }
                        return false;
                    }
                    // Frame freezing, component inspection, mouse-cell capture,
                    // and frame copying need a local app buffer, not socket data.
                    KeyCode::F(_) | KeyCode::Char('c' | 'C' | 'i' | 'I' | 'y' | 'Y') => {
                        return false
                    }
                    _ => {}
                }
            }
        }
        self.layer.handle_event_with_state(event, &self.state);
        self.remember_actions();
        false
    }

    fn remember_actions(&mut self) {
        if let Some(DebugOverlay::ActionLog(log)) = self.layer.freeze.overlay.as_ref() {
            self.actions = log.clone();
        }
    }

    /// Render the live debug layer's views without capturing/freezing an app.
    /// `status` is shown in the shared status bar (for example connection state).
    pub fn render(&mut self, frame: &mut Frame, status: &str) {
        let (area, banner) = self.layer.split_for_banner(frame.area());
        frame.render_widget(
            Block::default().style(Style::default().bg(self.layer.style.overlay_bg_dark)),
            area,
        );
        if self.layer.freeze.overlay.is_none() {
            frame.render_widget(
                Paragraph::new("a: actions · s: state · q: quit")
                    .style(Style::default().fg(self.layer.style.text_secondary)),
                area,
            );
        }
        self.layer.render_current_overlay(frame, area);

        let keys = &self.layer.style.key_styles;
        let mut left = vec![StatusBarItem::span(Span::styled(
            " DEBUG ",
            self.layer.style.title_style,
        ))];
        for (key, label, style) in [
            ("a", "actions", keys.actions),
            ("s", "state", keys.state),
            ("b", self.layer.banner_position.label(), keys.actions),
            ("q", "quit", keys.toggle),
        ] {
            left.push(StatusBarItem::span(Span::styled(format!(" {key} "), style)));
            left.push(StatusBarItem::span(Span::styled(
                format!(" {label} "),
                self.layer.style.label_style,
            )));
        }
        let message = self.layer.freeze.message.as_deref().unwrap_or(status);
        let right = [StatusBarItem::span(Span::styled(
            format!(" {message} "),
            self.layer.style.value_style,
        ))];
        self.layer.render_banner_items(frame, banner, &left, &right);
    }
}
