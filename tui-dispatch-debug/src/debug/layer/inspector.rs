//! Read-only data adapter for the live debug layer's existing views.
use crate::control::{ComponentTarget, ControlStatus, DebugCommand};
use crate::inspector::{state_diff, StateChange};
use crate::records::{DebugSnapshot, SnapshotOutcome, SnapshotRecord};
use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    style::Style,
    text::Span,
    widgets::{Block, Paragraph},
    Frame,
};
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;
use tui_dispatch_components::StatusBarItem;
use tui_dispatch_core::{Action, EventKind};

use super::DebugLayer;
use crate::debug::{
    ActionLogDisplayEntry, ActionLogOverlay, ComponentSnapshot, DebugEntry, DebugOverlay,
    DebugSection, DebugState, DebugStyle,
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

/// State inspector using the same modal state tree, search, syntax colors,
/// scrolling, details, and status bar as [`DebugLayer`].
///
/// Feed it a [`DebugSnapshot`] with [`update_snapshot`](Self::update_snapshot),
/// or display-only state and newest-first action entries with [`update`](Self::update).
/// No application actions
/// are dispatched by this view. Optional live commands go to its socket client.
/// Updates preserve
/// search/selection; an open detail remains stable until the user goes back.
/// Embedded views are modal by default. The standalone client explicitly opts
/// into [`fullscreen`](Self::fullscreen) presentation.
pub struct DebugInspector {
    layer: DebugLayer<InspectorAction>,
    state: InspectorState,
    actions: ActionLogOverlay,
    remote: Option<ControlStatus>,
    pause_toggle: bool,
    sent_highlight: Option<ComponentTarget>,
    snapshot: Option<Arc<DebugSnapshot>>,
    initial_state: Option<Value>,
    detail_record: Option<(SnapshotRecord, Option<Value>)>,
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
        layer.component_highlight_local = false;
        layer.freeze.enable();
        let actions = ActionLogOverlay::from_entries(Vec::new(), "Action Log");
        layer
            .freeze
            .set_overlay(DebugOverlay::ActionLog(actions.clone()));
        Self {
            layer,
            state: InspectorState(Value::Null),
            actions,
            remote: None,
            pause_toggle: false,
            sent_highlight: None,
            snapshot: None,
            initial_state: None,
            detail_record: None,
        }
    }

    /// Fill the terminal when running as a standalone debugger.
    /// Embedded inspectors should retain the default modal presentation.
    pub fn fullscreen(mut self) -> Self {
        self.layer.overlay_fullscreen = true;
        self
    }

    /// Apply the same palette used to customize a live debug layer.
    pub fn with_style(mut self, style: DebugStyle) -> Self {
        self.layer = self.layer.with_style(style);
        self
    }

    /// Supply the state immediately before sequence 1 of this source. Without
    /// this, the oldest retained record has no known predecessor for a diff.
    /// Use a new inspector when switching to a different recording/session.
    pub fn with_initial_state(mut self, state: Value) -> Self {
        self.initial_state = Some(state);
        self
    }

    /// Replace retained dispatch history, oldest first with strictly increasing
    /// sequences. Accepts an owned snapshot or an `Arc<DebugSnapshot>` shared
    /// with a client, avoiding another full history copy. Does not diff or format
    /// historical states. The source controls the history bound.
    ///
    /// Enter opens payload, boundary state, diff, and dispatch metadata. An open
    /// detail pins one record and its predecessor until returning to the list,
    /// even when a subsequent update evicts them. The `s` tab remains latest state.
    pub fn update_snapshot(&mut self, snapshot: impl Into<Arc<DebugSnapshot>>) {
        let snapshot = snapshot.into();
        let entries = snapshot
            .history
            .iter()
            .rev()
            .map(|record| {
                let outcome = match record.outcome {
                    SnapshotOutcome::Applied => "applied",
                    SnapshotOutcome::Cancelled => "cancelled",
                    _ => "rejected",
                };
                ActionLogDisplayEntry {
                    sequence: record.sequence,
                    name: record.name.clone(),
                    params: format!("[{outcome}] {}", record.action),
                    // Search payload/metadata only; historical state formatting is lazy.
                    params_detail: serde_json::to_string_pretty(&serde_json::json!({
                        "action": record.action,
                        "outcome": record.outcome,
                        "changed": record.changed,
                        "parent_action_id": record.parent_action_id,
                        "serialization_errors": record.serialization_errors,
                    }))
                    .expect("JSON values are serializable"),
                    elapsed: "—".into(),
                }
            })
            .collect();
        self.update(snapshot.state.clone(), entries);
        self.update_components(snapshot.components.clone());
        self.update_control_status(snapshot.control.clone());
        self.snapshot = Some(snapshot);
    }

    fn history(&self) -> &[SnapshotRecord] {
        self.snapshot
            .as_ref()
            .map(|snapshot| snapshot.history.as_slice())
            .unwrap_or_default()
    }

    fn action_list(&self) -> &ActionLogOverlay {
        match self.overlay() {
            Some(DebugOverlay::ActionLog(log)) => log,
            _ => &self.actions,
        }
    }

    /// Visible action sequences, newest first. Search is case-insensitive and
    /// matches action names, payloads, outcomes, and dispatch metadata.
    pub fn visible_sequences(&self) -> Vec<u64> {
        let log = self.action_list();
        log.entries
            .iter()
            .enumerate()
            .filter(|(index, _)| !log.has_search_query() || log.is_search_match(*index))
            .map(|(_, entry)| entry.sequence)
            .collect()
    }

    /// Set the action query and return to the action list.
    pub fn set_search(&mut self, query: impl Into<String>) {
        self.remember_actions();
        self.actions.set_search_query(query);
        self.actions.search_input_active = false;
        self.detail_record = None;
        self.layer
            .freeze
            .set_overlay(DebugOverlay::ActionLog(self.actions.clone()));
    }

    /// Select a visible sequence and return to the list. A hidden/missing
    /// sequence returns false and leaves navigation untouched.
    pub fn select_sequence(&mut self, sequence: u64) -> bool {
        let log = self.action_list();
        let Some(index) = log.entries.iter().enumerate().position(|(index, entry)| {
            entry.sequence == sequence && (!log.has_search_query() || log.is_search_match(index))
        }) else {
            return false;
        };
        self.remember_actions();
        self.actions.selected = index;
        // Keep keyboard navigation aligned with a programmatic selection.
        self.actions
            .set_search_query(self.actions.search_query.clone());
        self.actions.search_input_active = false;
        self.detail_record = None;
        self.layer
            .freeze
            .set_overlay(DebugOverlay::ActionLog(self.actions.clone()));
        true
    }

    fn selected_index(&self) -> Option<usize> {
        let sequence = match self.overlay() {
            Some(DebugOverlay::ActionDetail(detail)) => detail.sequence,
            _ => self.action_list().get_selected()?.sequence,
        };
        self.history()
            .binary_search_by_key(&sequence, |record| record.sequence)
            .ok()
    }

    fn pinned_detail(&self) -> Option<&(SnapshotRecord, Option<Value>)> {
        if matches!(self.overlay(), Some(DebugOverlay::ActionDetail(_))) {
            self.detail_record.as_ref()
        } else {
            None
        }
    }

    /// Selected dispatch, including a pinned open detail after history eviction.
    pub fn selected_record(&self) -> Option<&SnapshotRecord> {
        self.pinned_detail()
            .map(|(record, _)| record)
            .or_else(|| self.selected_index().map(|index| &self.history()[index]))
    }

    /// Selected action payload. JSON null is valid; None means unavailable.
    pub fn selected_payload(&self) -> Option<&Value> {
        let record = self.selected_record()?;
        (!record
            .serialization_errors
            .iter()
            .any(|e| e.starts_with("action:")))
        .then_some(&record.action)
    }

    /// State captured at the selected dispatch boundary, including unchanged or
    /// cancelled actions. None indicates no selection or failed serialization.
    pub fn selected_state(&self) -> Option<&Value> {
        Self::record_state(self.selected_record()?)
    }

    fn record_state(record: &SnapshotRecord) -> Option<&Value> {
        (!record
            .serialization_errors
            .iter()
            .any(|e| e.starts_with("state:")))
        .then_some(&record.state)
    }

    fn selected_before(&self) -> Option<&Value> {
        if let Some((_, before)) = self.pinned_detail() {
            return before.as_ref();
        }
        let index = self.selected_index()?;
        let record = &self.history()[index];
        if index == 0 {
            return (record.sequence == 1)
                .then_some(self.initial_state.as_ref())
                .flatten();
        }
        let previous = &self.history()[index - 1];
        if previous.sequence.checked_add(1) != Some(record.sequence) {
            return None;
        }
        Self::record_state(previous)
    }

    /// On-demand diff against the immediately preceding dispatch (even if it is
    /// hidden by search). None means the baseline/state is unavailable; an empty
    /// vector means a known unchanged state. No cross-gap diff is invented.
    pub fn selected_diff(&self) -> Option<Vec<StateChange>> {
        Some(state_diff(self.selected_before()?, self.selected_state()?))
    }

    fn open_record_detail(&mut self) {
        let Some(record) = self.selected_record() else {
            return;
        };
        let before = self.selected_before().cloned();
        let record = record.clone();
        #[derive(Serialize)]
        struct Detail<'a> {
            payload: Option<&'a Value>,
            state: Option<&'a Value>,
            diff: Option<Vec<StateChange>>,
            diff_status: &'a str,
            sequence: u64,
            action_id: u64,
            parent_action_id: Option<u64>,
            outcome: &'a SnapshotOutcome,
            changed: bool,
            serialization_errors: &'a [String],
        }
        let diff = self.selected_diff();
        let params = serde_json::to_string_pretty(&Detail {
            payload: self.selected_payload(),
            state: self.selected_state(),
            diff_status: if diff.is_some() {
                "available"
            } else {
                "unavailable: predecessor or state not captured"
            },
            diff,
            sequence: record.sequence,
            action_id: record.action_id,
            parent_action_id: record.parent_action_id,
            outcome: &record.outcome,
            changed: record.changed,
            serialization_errors: &record.serialization_errors,
        })
        .expect("JSON values are serializable");
        self.detail_record = Some((record, before));
        if let Some(DebugOverlay::ActionDetail(detail)) = self.layer.freeze.overlay.as_mut() {
            detail.params = params;
        }
    }

    /// Refresh remote/offline data without resetting navigation or search input.
    /// Entries must be in newest-first order, with stable sequence numbers.
    /// This display-only adapter clears retained records; use `update_snapshot`
    /// for selected payload/state/diff access. Already open details stay pinned.
    pub fn update(&mut self, state: Value, entries: Vec<ActionLogDisplayEntry>) {
        self.remember_actions();
        self.actions.replace_entries(entries);
        self.snapshot = None;
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

    /// Refresh component telemetry without interrupting search or open details.
    /// `None` indicates that the app/source does not supply component snapshots;
    /// `Some(Vec::new())` indicates a known empty component frame.
    pub fn update_components(&mut self, components: Option<Vec<ComponentSnapshot>>) {
        let selected = match self.layer.freeze.overlay.as_ref() {
            Some(DebugOverlay::Components(view)) => self
                .layer
                .components_tree_selected
                .as_deref()
                .and_then(|id| id.strip_prefix("comp:"))
                .and_then(|id| id.parse::<usize>().ok())
                .and_then(|index| view.components.get(index))
                .map(|component| (component.bound_id.clone(), component.raw_id)),
            _ => None,
        };
        self.layer.component_snapshotter = components.map(|components| {
            Box::new(move || components.clone()) as Box<dyn Fn() -> Vec<ComponentSnapshot>>
        });
        if matches!(self.layer.freeze.overlay, Some(DebugOverlay::Components(_))) {
            self.layer.show_components();
            if let (Some((bound_id, raw_id)), Some(DebugOverlay::Components(view))) =
                (selected, self.layer.freeze.overlay.as_ref())
            {
                if let Some(index) = view.components.iter().position(|component| {
                    if bound_id.is_some() {
                        component.bound_id == bound_id
                    } else {
                        component.raw_id == raw_id
                    }
                }) {
                    self.layer.components_tree_selected = Some(format!("comp:{index}"));
                }
            }
            self.label_unavailable_components();
        }
    }

    fn label_unavailable_components(&mut self) {
        if self.layer.component_snapshotter.is_none() {
            if let Some(DebugOverlay::Components(view)) = self.layer.freeze.overlay.as_mut() {
                view.title = "Components · not published by app".into();
            }
        }
    }

    /// Current view, including its selection/search state.
    pub fn overlay(&self) -> Option<&DebugOverlay> {
        self.layer.freeze.overlay.as_ref()
    }

    /// Update live app capabilities/confirmed pause state from a socket snapshot.
    pub fn update_control_status(&mut self, status: Option<ControlStatus>) {
        if self.remote.is_none() && status.is_some() {
            self.layer.freeze.clear_message();
        }
        self.remote = status;
    }

    fn selected_component(&self) -> Option<ComponentTarget> {
        let (raw_id, bound_id) = match self.layer.freeze.overlay.as_ref()? {
            DebugOverlay::Components(view) => {
                let id = self.layer.components_tree_selected.as_deref()?;
                let index = id
                    .strip_prefix("comp:")
                    .or_else(|| {
                        id.strip_prefix("entry:")
                            .and_then(|id| id.split(':').next())
                    })?
                    .parse::<usize>()
                    .ok()?;
                let component = view.components.get(index)?;
                (component.raw_id, component.bound_id.clone())
            }
            DebugOverlay::ComponentDetail(detail) => (detail.raw_id, detail.bound_id.clone()),
            _ => return None,
        };
        Some(ComponentTarget {
            raw_id: if bound_id.is_some() { 0 } else { raw_id },
            bound_id,
        })
    }

    /// Coalesced live requests: at most one pause toggle and one selection
    /// change per UI pass. The client sends these on its persistent connection.
    pub fn take_commands(&mut self) -> Vec<DebugCommand> {
        let mut commands = Vec::new();
        if std::mem::take(&mut self.pause_toggle) {
            commands.push(DebugCommand::TogglePause);
        }
        let highlight = self.remote.as_ref().and_then(|_| self.selected_component());
        if highlight != self.sent_highlight {
            self.sent_highlight = highlight.clone();
            commands.push(DebugCommand::Highlight(highlight));
        }
        commands
    }

    /// Handle shared debug navigation; returns true when the client should quit.
    ///
    /// `a`/`c`/`s` switch actions/components/state, `/` searches, Enter opens details, and
    /// Esc/Backspace return from details. `q` or Ctrl+C quit; Esc also quits from
    /// a list/tree. While editing a query, letters (including q/a/s) are text.
    /// `w` saves the current JSON state using the live layer's local snapshot UI.
    /// `P` requests app pause/resume when the source supports live controls.
    pub fn handle_event(&mut self, event: &EventKind) -> bool {
        let was_action_detail = matches!(self.overlay(), Some(DebugOverlay::ActionDetail(_)));
        if !was_action_detail {
            self.detail_record = None;
        }
        self.remember_actions();
        let detail_binding = match self.layer.freeze.overlay.as_ref() {
            Some(DebugOverlay::ComponentDetail(detail)) => detail
                .bound_id
                .clone()
                .map(|id| (id, detail.type_name_full.clone())),
            _ => None,
        };
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
                Some(DebugOverlay::Components(_)) => self.layer.components_search_active,
                _ => false,
            };
            if !searching {
                if matches!(key.code, KeyCode::Char('p' | 'P')) {
                    if key.kind == KeyEventKind::Press {
                        if self.remote.is_some() {
                            self.pause_toggle = !self.pause_toggle;
                            self.layer.freeze.clear_message();
                        } else {
                            self.layer
                                .freeze
                                .set_message("App controls unavailable for this source");
                        }
                    }
                    return false;
                }
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
                    self.detail_record = None;
                    return false;
                }
                if key.code == KeyCode::Esc
                    && !matches!(
                        self.layer.freeze.overlay,
                        Some(DebugOverlay::StateDetail(_)) | Some(DebugOverlay::ComponentDetail(_))
                    )
                {
                    return true;
                }
                match key.code {
                    KeyCode::Char('a' | 'A') => {
                        self.detail_record = None;
                        if matches!(self.layer.freeze.overlay, Some(DebugOverlay::ActionLog(_))) {
                            self.layer.freeze.clear_overlay();
                        } else {
                            self.layer
                                .freeze
                                .set_overlay(DebugOverlay::ActionLog(self.actions.clone()));
                        }
                        return false;
                    }
                    // Frame freezing, mouse-cell capture, and frame copying
                    // require a local app buffer rather than socket metadata.
                    KeyCode::F(_) | KeyCode::Char('i' | 'I' | 'y' | 'Y') => return false,
                    _ => {}
                }
            }
        }
        self.layer.handle_event_with_state(event, &self.state);
        if !was_action_detail && matches!(self.overlay(), Some(DebugOverlay::ActionDetail(_))) {
            self.open_record_detail();
        }
        if !matches!(self.overlay(), Some(DebugOverlay::ActionDetail(_))) {
            self.detail_record = None;
        }
        // A component may move in the published list while its detail is open.
        // Return to its binding rather than the old list index.
        if let (Some((id, type_name)), Some(DebugOverlay::Components(view))) =
            (detail_binding, self.layer.freeze.overlay.as_ref())
        {
            if let Some(index) = view.components.iter().position(|component| {
                component.bound_id.as_deref() == Some(id.as_str())
                    && component.type_name_full == type_name
            }) {
                self.layer.components_tree_selected = Some(format!("comp:{index}"));
            }
        }
        self.label_unavailable_components();
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
        if self.layer.overlay_fullscreen {
            frame.render_widget(
                Block::default().style(Style::default().bg(self.layer.style.overlay_bg_dark)),
                area,
            );
            if self.layer.freeze.overlay.is_none() {
                frame.render_widget(
                    Paragraph::new("a: actions · c: components · s: state · q: quit")
                        .style(Style::default().fg(self.layer.style.text_secondary)),
                    area,
                );
            }
        }
        self.layer.render_current_overlay(frame, area);

        let keys = &self.layer.style.key_styles;
        let mut left = vec![StatusBarItem::span(Span::styled(
            " DEBUG ",
            self.layer.style.title_style,
        ))];
        for (key, label, style) in [
            ("a", "actions", keys.actions),
            ("c", "components", keys.components),
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
        if let Some(remote) = &self.remote {
            left.push(StatusBarItem::span(Span::styled(" P ", keys.toggle)));
            left.push(StatusBarItem::span(Span::styled(
                if remote.pause_requested {
                    " resume "
                } else {
                    " pause "
                },
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
