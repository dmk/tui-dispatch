//! High-level debug layer for TUI applications
//!
//! Provides a self-contained debug overlay with automatic pause/resume of
//! tasks and subscriptions.

use std::any::Any;
use std::collections::HashSet;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::prelude::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::Frame;
use serde::Serialize;

use super::action_logger::{ActionLog, ActionLogConfig, ActionLoggerConfig};
use super::actions::{DebugAction, DebugSideEffect};
use super::cell::inspect_cell;
use super::config::DebugStyle;
use super::state::DebugState;
use super::table::{
    ActionLogOverlay, DebugOverlay, DebugTableBuilder, DebugTableOverlay, DebugTableRow,
};
use super::widgets::{
    debug_spans, dim_buffer, paint_snapshot, ActionLogStyle, CellPreviewWidget, DebugSyntaxStyle,
    DebugTableStyle,
};
use super::DebugFreeze;

use tui_dispatch_components::{
    centered_rect, BaseStyle, LinesScroller, Modal, ModalBehavior, ModalProps, ModalStyle, Padding,
    ScrollView, ScrollViewBehavior, ScrollViewProps, ScrollViewStyle,
    ScrollbarStyle as ComponentScrollbarStyle, SelectionStyle, StatusBar, StatusBarItem,
    StatusBarProps, StatusBarSection, StatusBarStyle, TreeBranchMode, TreeBranchStyle, TreeNode,
    TreeNodeRender, TreeView, TreeViewBehavior, TreeViewProps, TreeViewStyle,
};

#[cfg(feature = "subscriptions")]
use tui_dispatch_core::subscriptions::SubPauseHandle;
#[cfg(feature = "tasks")]
use tui_dispatch_core::tasks::TaskPauseHandle;
use tui_dispatch_core::{Action, Component, EventKind};

type StateSnapshotter = Box<dyn Fn(&dyn Any, &Path) -> crate::SnapshotResult<()> + 'static>;

#[derive(Debug, Clone)]
enum StateTreeAction {
    Select(String),
    Toggle(String, bool),
}

fn state_tree_select(id: &str) -> StateTreeAction {
    StateTreeAction::Select(id.to_owned())
}

fn state_tree_toggle(id: &str, expanded: bool) -> StateTreeAction {
    StateTreeAction::Toggle(id.to_owned(), expanded)
}

struct InlineValueStyle {
    base: Style,
    key: Style,
    string: Style,
    number: Style,
    r#type: Style,
}

fn ron_value_spans(value: &str, style: &InlineValueStyle) -> Vec<Span<'static>> {
    let chars: Vec<char> = value.chars().collect();
    let mut spans = Vec::new();
    let mut idx = 0;

    while idx < chars.len() {
        let current = chars[idx];
        if current == '"' {
            let start = idx;
            idx += 1;
            while idx < chars.len() {
                if chars[idx] == '"' && chars.get(idx.saturating_sub(1)) != Some(&'\\') {
                    idx += 1;
                    break;
                }
                idx += 1;
            }
            let text: String = chars[start..idx].iter().collect();
            spans.push(Span::styled(text, style.string));
            continue;
        }

        if current.is_ascii_digit()
            || (current == '-' && chars.get(idx + 1).is_some_and(|c| c.is_ascii_digit()))
        {
            let start = idx;
            idx += 1;
            while idx < chars.len() {
                let c = chars[idx];
                if c.is_ascii_digit() || matches!(c, '.' | '_' | 'e' | 'E' | '+' | '-') {
                    idx += 1;
                } else {
                    break;
                }
            }
            let text: String = chars[start..idx].iter().collect();
            spans.push(Span::styled(text, style.number));
            continue;
        }

        if current.is_alphanumeric() || current == '_' {
            let start = idx;
            idx += 1;
            while idx < chars.len() {
                let c = chars[idx];
                if c.is_alphanumeric() || c == '_' {
                    idx += 1;
                } else {
                    break;
                }
            }
            let mut text: String = chars[start..idx].iter().collect();
            let mut span_style = style.base;

            if idx < chars.len() && chars[idx] == ':' {
                text.push(':');
                idx += 1;
                span_style = style.key;
            } else if text == "true" || text == "false" {
                span_style = style.number;
            } else if text.chars().next().is_some_and(|c| c.is_uppercase()) {
                span_style = style.r#type;
            }

            spans.push(Span::styled(text, span_style));
            continue;
        }

        spans.push(Span::styled(current.to_string(), style.base));
        idx += 1;
    }

    spans
}

fn render_state_tree_node(ctx: TreeNodeRender<'_, String, String>) -> Line<'static> {
    // Section header style - bold accent color for expandable sections
    let section_style = Style::default()
        .fg(DebugStyle::accent())
        .add_modifier(Modifier::BOLD);
    // Key/field label style
    let key_style = Style::default().fg(DebugStyle::text_primary()).bold();
    // Value styling
    let value_style = Style::default().fg(DebugStyle::text_primary());
    let type_style = Style::default().fg(DebugStyle::neon_purple());
    let string_style = Style::default().fg(DebugStyle::neon_green());
    let number_style = Style::default().fg(DebugStyle::neon_amber());

    let selection_patch = if ctx.is_selected {
        Style::default().bg(DebugStyle::bg_highlight())
    } else {
        Style::default()
    };

    let content_width = ctx.available_width.max(1);
    let mut spans = Vec::new();

    // Section headers (nodes with children) - bold accent style
    if ctx.has_children {
        let text = truncate_with_ellipsis(&ctx.node.value, content_width);
        spans.push(Span::styled(text, section_style).patch_style(selection_patch));
        return Line::from(spans);
    }

    // Key: value pairs - render as "key: value" with syntax highlighting
    if let Some((key, value)) = ctx.node.value.split_once(": ") {
        let key_text = format!("{key}: ");
        let key_len = key_text.chars().count();
        spans.push(Span::styled(key_text, key_style).patch_style(selection_patch));

        let remaining = content_width.saturating_sub(key_len);
        if remaining > 0 {
            let value_text = truncate_with_ellipsis(value, remaining);
            let inline_style = InlineValueStyle {
                base: value_style,
                key: key_style,
                string: string_style,
                number: number_style,
                r#type: type_style,
            };
            let value_spans = ron_value_spans(&value_text, &inline_style);
            for span in value_spans {
                spans.push(span.patch_style(selection_patch));
            }
        }

        return Line::from(spans);
    }

    // Plain text nodes
    let text = truncate_with_ellipsis(&ctx.node.value, content_width);
    spans.push(Span::styled(text, value_style).patch_style(selection_patch));

    Line::from(spans)
}

/// Location of the debug banner relative to the app area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BannerPosition {
    Bottom,
    Top,
}

impl BannerPosition {
    /// Toggle between top and bottom.
    pub fn toggle(self) -> Self {
        match self {
            Self::Bottom => Self::Top,
            Self::Top => Self::Bottom,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Bottom => "bar:bottom",
            Self::Top => "bar:top",
        }
    }
}

// ============================================================================
// Modal Footer Hints
// ============================================================================

/// A single hint for modal footer status bar
#[derive(Debug, Clone, Copy)]
pub struct ModalHint {
    pub key: &'static str,
    pub label: &'static str,
}

impl ModalHint {
    pub const fn new(key: &'static str, label: &'static str) -> Self {
        Self { key, label }
    }
}

/// Hints configuration for a modal footer
#[derive(Debug, Clone, Copy)]
pub struct ModalHints {
    pub left: &'static [ModalHint],
    pub right: &'static [ModalHint],
}

// Action Log Modal hints
const ACTION_LOG_HINTS: ModalHints = ModalHints {
    left: &[
        ModalHint::new("j/k", "scroll"),
        ModalHint::new("g/G", "top/bottom"),
        ModalHint::new("/", "filter"),
    ],
    right: &[
        ModalHint::new("n/N", "next/prev"),
        ModalHint::new("Enter", "details"),
        ModalHint::new("Bksp", "close"),
    ],
};

// Action Log Modal hints while search input is active
const ACTION_LOG_SEARCH_INPUT_HINTS: ModalHints = ModalHints {
    left: &[
        ModalHint::new("type", "query"),
        ModalHint::new("Bksp", "edit"),
    ],
    right: &[
        ModalHint::new("Enter", "done"),
        ModalHint::new("Esc", "done"),
    ],
};

// State Tree Modal hints
const STATE_TREE_HINTS: ModalHints = ModalHints {
    left: &[
        ModalHint::new("j/k", "scroll"),
        ModalHint::new("Space", "expand"),
    ],
    right: &[ModalHint::new("w", "save"), ModalHint::new("Bksp", "close")],
};

// Action Detail Modal hints
const ACTION_DETAIL_HINTS: ModalHints = ModalHints {
    left: &[ModalHint::new("j/k", "scroll")],
    right: &[ModalHint::new("Bksp", "back")],
};

// Inspect Modal hints
const INSPECT_HINTS: ModalHints = ModalHints {
    left: &[ModalHint::new("j/k", "scroll")],
    right: &[ModalHint::new("Bksp", "close")],
};

/// Result of handling a debug event.
pub struct DebugOutcome<A> {
    /// Whether the debug layer consumed the event.
    pub consumed: bool,
    /// Actions queued while debug was active (e.g., from pause/resume).
    pub queued_actions: Vec<A>,
    /// Whether a re-render is needed.
    pub needs_render: bool,
}

impl<A> DebugOutcome<A> {
    fn ignored() -> Self {
        Self {
            consumed: false,
            queued_actions: Vec::new(),
            needs_render: false,
        }
    }

    fn consumed(queued_actions: Vec<A>) -> Self {
        Self {
            consumed: true,
            queued_actions,
            needs_render: true,
        }
    }

    /// Dispatch queued actions if the debug layer consumed the event.
    ///
    /// Returns `Some(needs_render)` when consumed, otherwise `None`.
    pub fn dispatch_queued<F>(self, mut dispatch: F) -> Option<bool>
    where
        F: FnMut(A),
    {
        if !self.consumed {
            return None;
        }

        for action in self.queued_actions {
            dispatch(action);
        }

        Some(self.needs_render)
    }
}

impl<A> Default for DebugOutcome<A> {
    fn default() -> Self {
        Self::ignored()
    }
}

/// High-level debug layer with minimal configuration.
///
/// Provides automatic freeze/unfreeze with pause/resume of tasks and subscriptions.
///
/// # Example
///
/// ```ignore
/// use tui_dispatch::debug::DebugLayer;
///
/// // Minimal setup with sensible defaults (F12 toggle key)
/// let mut debug = DebugLayer::simple()
///     .with_task_manager(&tasks)
///     .with_subscriptions(&subs)
///     .active(args.debug);
///
/// // In event loop
/// if debug.intercepts(&event) {
///     continue;
/// }
///
/// // In render
/// debug.render(frame, |f, area| {
///     app.render(f, area);
/// });
///
/// // Log actions for the action log feature
/// debug.log_action(&action);
/// ```
pub struct DebugLayer<A> {
    /// Key to toggle debug mode
    toggle_key: KeyCode,
    /// Internal freeze state
    freeze: DebugFreeze<A>,
    /// Where the debug banner is rendered
    banner_position: BannerPosition,
    /// Style configuration
    style: DebugStyle,
    /// Whether the debug layer is active (can be disabled for release builds)
    active: bool,
    /// Action log for display
    action_log: ActionLog,
    /// Cached state snapshot for the state overlay
    state_snapshot: Option<DebugTableOverlay>,
    /// Optional serializer for saving state snapshots
    state_snapshotter: Option<StateSnapshotter>,
    /// Tree view component for state overlay
    state_tree_view: TreeView<String>,
    /// Cached tree nodes for state overlay
    state_tree_nodes: Vec<TreeNode<String, String>>,
    /// Currently selected tree node id
    state_tree_selected: Option<String>,
    /// Expanded node ids for the state tree
    state_tree_expanded: HashSet<String>,
    /// Scroll offset for state/inspect table overlays
    table_scroll_offset: usize,
    /// Cached page size for table overlay scrolling
    table_page_size: usize,
    /// Scroll offset for action detail modal
    detail_scroll_offset: usize,
    /// Cached page size for action detail scrolling
    detail_page_size: usize,
    /// Handle to pause/resume task manager
    #[cfg(feature = "tasks")]
    task_handle: Option<TaskPauseHandle<A>>,
    /// Handle to pause/resume subscriptions
    #[cfg(feature = "subscriptions")]
    sub_handle: Option<SubPauseHandle>,
}

impl<A> std::fmt::Debug for DebugLayer<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DebugLayer")
            .field("toggle_key", &self.toggle_key)
            .field("active", &self.active)
            .field("enabled", &self.freeze.enabled)
            .field("has_snapshot", &self.freeze.snapshot.is_some())
            .field("has_state_snapshot", &self.state_snapshot.is_some())
            .field("has_state_snapshotter", &self.state_snapshotter.is_some())
            .field("state_tree_nodes", &self.state_tree_nodes.len())
            .field("state_tree_selected", &self.state_tree_selected)
            .field("banner_position", &self.banner_position)
            .field("table_scroll_offset", &self.table_scroll_offset)
            .field("detail_scroll_offset", &self.detail_scroll_offset)
            .field("queued_actions", &self.freeze.queued_actions.len())
            .finish()
    }
}

impl<A: Action> DebugLayer<A> {
    /// Create a new debug layer with the given toggle key.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use crossterm::event::KeyCode;
    ///
    /// let debug = DebugLayer::new(KeyCode::F(12));
    /// ```
    pub fn new(toggle_key: KeyCode) -> Self {
        Self {
            toggle_key,
            freeze: DebugFreeze::new(),
            banner_position: BannerPosition::Bottom,
            style: DebugStyle::default(),
            active: true,
            action_log: ActionLog::new(ActionLogConfig::with_capacity(100)),
            state_snapshot: None,
            state_snapshotter: None,
            state_tree_view: TreeView::new(),
            state_tree_nodes: Vec::new(),
            state_tree_selected: None,
            state_tree_expanded: HashSet::new(),
            table_scroll_offset: 0,
            table_page_size: 1,
            detail_scroll_offset: 0,
            detail_page_size: 1,
            #[cfg(feature = "tasks")]
            task_handle: None,
            #[cfg(feature = "subscriptions")]
            sub_handle: None,
        }
    }

    /// Create a debug layer with sensible defaults (F12 toggle key).
    pub fn simple() -> Self {
        Self::new(KeyCode::F(12))
    }

    /// Create a debug layer with a custom toggle key.
    pub fn simple_with_toggle_key(toggle_key: KeyCode) -> Self {
        Self::new(toggle_key)
    }

    /// Set whether the debug layer is active.
    ///
    /// When inactive (`false`), all methods become no-ops with zero overhead.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Set the initial banner position.
    pub fn with_banner_position(mut self, position: BannerPosition) -> Self {
        self.banner_position = position;
        self
    }

    /// Connect a task manager for automatic pause/resume.
    ///
    /// When debug mode is enabled, the task manager will be paused.
    /// When disabled, queued actions will be returned.
    #[cfg(feature = "tasks")]
    pub fn with_task_manager(mut self, tasks: &tui_dispatch_core::tasks::TaskManager<A>) -> Self {
        self.task_handle = Some(tasks.pause_handle());
        self
    }

    /// Connect subscriptions for automatic pause/resume.
    ///
    /// When debug mode is enabled, subscriptions will be paused.
    #[cfg(feature = "subscriptions")]
    pub fn with_subscriptions(
        mut self,
        subs: &tui_dispatch_core::subscriptions::Subscriptions<A>,
    ) -> Self {
        self.sub_handle = Some(subs.pause_handle());
        self
    }

    /// Set the action log capacity.
    pub fn with_action_log_capacity(mut self, capacity: usize) -> Self {
        self.action_log = ActionLog::new(ActionLogConfig::with_capacity(capacity));
        self
    }

    /// Set full action-log configuration (capacity + filter).
    pub fn with_action_log_config(mut self, config: ActionLogConfig) -> Self {
        self.action_log = ActionLog::new(config);
        self
    }

    /// Set action-log filtering while keeping the current capacity.
    pub fn with_action_log_filter(mut self, filter: ActionLoggerConfig) -> Self {
        let capacity = self.action_log.config().capacity;
        self.action_log = ActionLog::new(ActionLogConfig::new(capacity, filter));
        self
    }

    /// Set custom style.
    pub fn with_style(mut self, style: DebugStyle) -> Self {
        self.style = style;
        self
    }

    /// Provide a custom serializer for saving state snapshots (W key).
    pub fn with_state_snapshotter<S, F>(mut self, snapshotter: F) -> Self
    where
        S: DebugState + 'static,
        F: Fn(&S, &Path) -> crate::SnapshotResult<()> + 'static,
    {
        self.state_snapshotter = Some(Box::new(move |state, path| {
            let state = state.downcast_ref::<S>().ok_or_else(|| {
                crate::SnapshotError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "debug state snapshot type mismatch",
                ))
            })?;
            snapshotter(state, path)
        }));
        self
    }

    /// Save full state snapshots using serde (loadable by `--debug-state-in`).
    pub fn with_state_snapshots<S>(self) -> Self
    where
        S: DebugState + Serialize + 'static,
    {
        self.with_state_snapshotter::<S, _>(|state, path| crate::save_json(path, state))
    }

    /// Check if the debug layer is active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Check if debug mode is enabled (and layer is active).
    pub fn is_enabled(&self) -> bool {
        self.active && self.freeze.enabled
    }

    /// Toggle debug mode on/off and return any side effects.
    ///
    /// Returns `None` when the layer is inactive or no side effects are needed.
    pub fn toggle_enabled(&mut self) -> Option<DebugSideEffect<A>> {
        if !self.active {
            return None;
        }
        self.toggle()
    }

    /// Set debug mode on/off and return any side effects.
    ///
    /// Returns `None` when the layer is inactive or already in the requested state.
    pub fn set_enabled(&mut self, enabled: bool) -> Option<DebugSideEffect<A>> {
        if !self.active || enabled == self.freeze.enabled {
            return None;
        }
        self.toggle()
    }

    /// Update the banner position (top/bottom) and request a new capture.
    pub fn set_banner_position(&mut self, position: BannerPosition) {
        if self.banner_position != position {
            self.banner_position = position;
            if self.freeze.enabled {
                self.freeze.request_capture();
            }
        }
    }

    /// Check if the state overlay is currently visible.
    pub fn is_state_overlay_visible(&self) -> bool {
        matches!(self.freeze.overlay, Some(DebugOverlay::State(_)))
    }

    /// Get a reference to the underlying freeze state.
    pub fn freeze(&self) -> &DebugFreeze<A> {
        &self.freeze
    }

    /// Get a mutable reference to the underlying freeze state.
    pub fn freeze_mut(&mut self) -> &mut DebugFreeze<A> {
        &mut self.freeze
    }

    /// Log an action to the action log.
    ///
    /// Call this when dispatching actions to record them for the debug overlay.
    pub fn log_action<T: tui_dispatch_core::ActionParams>(&mut self, action: &T) {
        if self.active {
            self.action_log.log(action);
        }
    }

    /// Get the action log.
    pub fn action_log(&self) -> &ActionLog {
        &self.action_log
    }

    /// Render with automatic debug handling.
    ///
    /// When debug mode is disabled, simply calls `render_fn` with the full frame area.
    /// When enabled, captures/paints the frozen snapshot and renders debug overlay.
    pub fn render<F>(&mut self, frame: &mut Frame, render_fn: F)
    where
        F: FnOnce(&mut Frame, Rect),
    {
        self.render_with_state(frame, |frame, area, _wants_state| {
            render_fn(frame, area);
            None
        });
    }

    /// Render with optional state capture for the state overlay.
    ///
    /// `render_fn` receives the frame, app area, and a `wants_state` hint that
    /// is `true` when debug mode is active and state data may be requested.
    /// Return `Some(DebugTableOverlay)` to update the cached state overlay.
    pub fn render_with_state<F>(&mut self, frame: &mut Frame, render_fn: F)
    where
        F: FnOnce(&mut Frame, Rect, bool) -> Option<DebugTableOverlay>,
    {
        let screen = frame.area();

        // Inactive or not in debug mode: just render normally
        if !self.active || !self.freeze.enabled {
            let _ = render_fn(frame, screen, false);
            return;
        }

        // Debug mode: reserve line for banner
        let (app_area, banner_area) = self.split_for_banner(screen);

        if self.freeze.pending_capture || self.freeze.snapshot.is_none() {
            // Capture mode: render app, then capture
            let state_snapshot = render_fn(frame, app_area, true);
            self.state_snapshot = state_snapshot;
            if let Some(ref table) = self.state_snapshot {
                if self.is_state_overlay_visible() {
                    self.set_state_overlay(table.clone());
                }
            }
            let buffer_clone = frame.buffer_mut().clone();
            self.freeze.capture(&buffer_clone);
        } else if let Some(ref snapshot) = self.freeze.snapshot {
            // Frozen: paint snapshot
            paint_snapshot(frame, snapshot);
        }

        // Render debug overlay
        self.render_debug_overlay(frame, app_area, banner_area);
    }

    /// Render with a DebugState reference and automatic state table generation.
    ///
    /// This is a convenience wrapper around `render_with_state`.
    pub fn render_state<S: DebugState, F>(&mut self, frame: &mut Frame, state: &S, render_fn: F)
    where
        F: FnOnce(&mut Frame, Rect),
    {
        self.render_with_state(frame, |frame, area, wants_state| {
            render_fn(frame, area);
            if wants_state {
                Some(state.build_debug_table("Application State"))
            } else {
                None
            }
        });
    }

    /// Split area for manual layout control.
    pub fn split_area(&self, area: Rect) -> (Rect, Rect) {
        if !self.freeze.enabled {
            return (area, Rect::ZERO);
        }
        self.split_for_banner(area)
    }

    /// Check if debug layer intercepts an event.
    ///
    /// Call this before your normal event handling. If it returns `true`,
    /// the event was consumed by the debug layer.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if debug.intercepts(&event) {
    ///     continue;
    /// }
    /// // Normal event handling
    /// ```
    pub fn intercepts(&mut self, event: &tui_dispatch_core::EventKind) -> bool {
        self.intercepts_with_effects(event).is_some()
    }

    /// Handle a debug event with a single call and return a summary outcome.
    pub fn handle_event(&mut self, event: &tui_dispatch_core::EventKind) -> DebugOutcome<A> {
        self.handle_event_internal::<()>(event, None)
    }

    /// Handle a debug event with access to state (for the state overlay).
    pub fn handle_event_with_state<S: DebugState + 'static>(
        &mut self,
        event: &tui_dispatch_core::EventKind,
        state: &S,
    ) -> DebugOutcome<A> {
        self.handle_event_internal(event, Some(state))
    }

    /// Check if debug layer intercepts an event, returning any side effects.
    ///
    /// Returns `None` if the event was not consumed, `Some(effects)` if it was.
    pub fn intercepts_with_effects(
        &mut self,
        event: &tui_dispatch_core::EventKind,
    ) -> Option<Vec<DebugSideEffect<A>>> {
        self.intercepts_with_effects_internal::<()>(event, None)
    }

    /// Check if debug layer intercepts an event with access to app state.
    ///
    /// Use this to populate the state overlay when `S` is pressed.
    pub fn intercepts_with_effects_and_state<S: DebugState + 'static>(
        &mut self,
        event: &tui_dispatch_core::EventKind,
        state: &S,
    ) -> Option<Vec<DebugSideEffect<A>>> {
        self.intercepts_with_effects_internal(event, Some(state))
    }

    /// Check if debug layer intercepts an event with access to app state.
    pub fn intercepts_with_state<S: DebugState + 'static>(
        &mut self,
        event: &tui_dispatch_core::EventKind,
        state: &S,
    ) -> bool {
        self.intercepts_with_effects_internal(event, Some(state))
            .is_some()
    }

    fn handle_event_internal<S: DebugState + 'static>(
        &mut self,
        event: &tui_dispatch_core::EventKind,
        state: Option<&S>,
    ) -> DebugOutcome<A> {
        let effects = self.intercepts_with_effects_internal(event, state);
        let Some(effects) = effects else {
            return DebugOutcome::ignored();
        };

        let mut queued_actions = Vec::new();
        for effect in effects {
            if let DebugSideEffect::ProcessQueuedActions(actions) = effect {
                queued_actions.extend(actions);
            }
        }

        DebugOutcome::consumed(queued_actions)
    }

    fn intercepts_with_effects_internal<S: DebugState + 'static>(
        &mut self,
        event: &tui_dispatch_core::EventKind,
        state: Option<&S>,
    ) -> Option<Vec<DebugSideEffect<A>>> {
        if !self.active {
            return None;
        }

        use tui_dispatch_core::EventKind;

        match event {
            EventKind::Key(key) => self.handle_key_event(*key, state),
            EventKind::Mouse(mouse) => {
                if !self.freeze.enabled {
                    return None;
                }

                // Only capture mouse when mouse_capture_enabled (toggle with 'i')
                // When disabled, let terminal handle mouse (allows text selection)
                if !self.freeze.mouse_capture_enabled {
                    return None;
                }

                // Handle click for cell inspection
                if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
                    let effect = self.handle_action(DebugAction::InspectCell {
                        column: mouse.column,
                        row: mouse.row,
                    });
                    return Some(effect.into_iter().collect());
                }

                // Consume mouse events when capturing
                Some(vec![])
            }
            EventKind::Scroll {
                delta,
                column,
                row,
                modifiers,
            } => {
                if !self.freeze.enabled {
                    return None;
                }

                match self.freeze.overlay.as_ref() {
                    Some(DebugOverlay::ActionLog(_)) => {
                        let action = if *delta > 0 {
                            DebugAction::ActionLogScrollUp
                        } else {
                            DebugAction::ActionLogScrollDown
                        };
                        self.handle_action(action);
                    }
                    Some(DebugOverlay::State(_)) => {
                        self.sync_state_tree_state();
                        let nodes = &self.state_tree_nodes;
                        let selected_id = self.state_tree_selected.as_ref();
                        let expanded_ids = &self.state_tree_expanded;
                        let style = self.state_tree_style();
                        let props =
                            Self::build_state_tree_props(nodes, selected_id, expanded_ids, style);
                        let actions: Vec<_> = self
                            .state_tree_view
                            .handle_event(
                                &EventKind::Scroll {
                                    delta: *delta,
                                    column: *column,
                                    row: *row,
                                    modifiers: *modifiers,
                                },
                                props,
                            )
                            .into_iter()
                            .collect();
                        if !actions.is_empty() {
                            self.apply_state_tree_actions(actions);
                        }
                    }
                    Some(DebugOverlay::Inspect(table)) => {
                        if *delta > 0 {
                            self.scroll_table_up();
                        } else {
                            self.scroll_table_down(table.rows.len());
                        }
                    }
                    Some(DebugOverlay::ActionDetail(detail)) => {
                        let params_lines = self.detail_params_lines(detail);
                        if *delta > 0 {
                            self.scroll_detail_up();
                        } else {
                            self.scroll_detail_down(params_lines.len());
                        }
                    }
                    _ => {}
                }

                Some(vec![])
            }
            // Don't intercept resize or tick events
            EventKind::Resize(_, _) | EventKind::Tick => None,
        }
    }

    /// Show state overlay using a DebugState implementor.
    pub fn show_state_overlay<S: DebugState>(&mut self, state: &S) {
        let table = state.build_debug_table("Application State");
        self.set_state_overlay(table);
    }

    /// Show action log overlay.
    pub fn show_action_log(&mut self) {
        let overlay = ActionLogOverlay::from_log(&self.action_log, "Action Log");
        self.freeze.set_overlay(DebugOverlay::ActionLog(overlay));
    }

    /// Queue an action to be processed when debug mode is disabled.
    pub fn queue_action(&mut self, action: A) {
        self.freeze.queue(action);
    }

    /// Take any queued actions (from task manager resume).
    ///
    /// Call this after `intercepts()` returns effects to get queued actions
    /// that should be dispatched.
    pub fn take_queued_actions(&mut self) -> Vec<A> {
        std::mem::take(&mut self.freeze.queued_actions)
    }

    // =========================================================================
    // Private helpers
    // =========================================================================

    fn set_state_overlay(&mut self, table: DebugTableOverlay) {
        let is_new_overlay = !matches!(self.freeze.overlay, Some(DebugOverlay::State(_)));
        if is_new_overlay {
            self.table_scroll_offset = 0;
            self.state_tree_view = TreeView::new();
            self.state_tree_selected = None;
            self.state_tree_expanded.clear();
        }

        self.state_tree_nodes = self.build_state_tree_nodes(&table);
        if is_new_overlay {
            self.state_tree_expanded = self
                .state_tree_nodes
                .iter()
                .map(|node| node.id.clone())
                .collect();
        }
        self.sync_state_tree_state();
        self.state_snapshot = Some(table.clone());
        self.freeze.set_overlay(DebugOverlay::State(table));
    }

    fn build_state_tree_nodes(&self, table: &DebugTableOverlay) -> Vec<TreeNode<String, String>> {
        let mut sections: Vec<TreeNode<String, String>> = Vec::new();
        let mut current: Option<TreeNode<String, String>> = None;
        let mut section_index = 0usize;
        let mut entry_index = 0usize;
        let mut current_section_index = 0usize;

        for row in &table.rows {
            match row {
                DebugTableRow::Section(title) => {
                    if let Some(section) = current.take() {
                        sections.push(section);
                    }
                    current_section_index = section_index;
                    entry_index = 0;
                    let id = format!("section:{section_index}:{title}");
                    section_index = section_index.saturating_add(1);
                    current = Some(TreeNode::with_children(id, title.clone(), Vec::new()));
                }
                DebugTableRow::Entry { key, value } => {
                    if current.is_none() {
                        current_section_index = section_index;
                        entry_index = 0;
                        let id = format!("section:{section_index}:State");
                        section_index = section_index.saturating_add(1);
                        current =
                            Some(TreeNode::with_children(id, "State".to_string(), Vec::new()));
                    }
                    if let Some(section) = current.as_mut() {
                        let entry_id = format!("entry:{current_section_index}:{entry_index}:{key}");
                        entry_index = entry_index.saturating_add(1);
                        section
                            .children
                            .push(TreeNode::new(entry_id, format!("{key}: {value}")));
                    }
                }
            }
        }

        if let Some(section) = current.take() {
            sections.push(section);
        }

        sections
    }

    fn sync_state_tree_state(&mut self) {
        let nodes = &self.state_tree_nodes;
        self.state_tree_expanded
            .retain(|id| Self::tree_contains_id(nodes, id));

        let selected_valid = self
            .state_tree_selected
            .as_deref()
            .map(|id| self.state_tree_contains_id(id))
            .unwrap_or(false);

        if !selected_valid {
            self.state_tree_selected = self.state_tree_nodes.first().map(|node| node.id.clone());
        }
    }

    fn state_tree_contains_id(&self, id: &str) -> bool {
        Self::tree_contains_id(&self.state_tree_nodes, id)
    }

    fn tree_contains_id(nodes: &[TreeNode<String, String>], id: &str) -> bool {
        nodes
            .iter()
            .any(|node| node.id == id || Self::tree_contains_id(&node.children, id))
    }

    fn state_tree_style(&self) -> TreeViewStyle {
        TreeViewStyle {
            base: BaseStyle {
                border: None,
                padding: Padding::default(),
                bg: Some(DebugStyle::overlay_bg()),
                fg: Some(DebugStyle::text_primary()),
            },
            selection: SelectionStyle::disabled(),
            scrollbar: self.component_scrollbar_style(),
            branches: TreeBranchStyle {
                mode: TreeBranchMode::Branch,
                connector_style: Style::default().fg(DebugStyle::text_secondary()),
                ..Default::default()
            },
        }
    }

    fn build_state_tree_props<'a>(
        nodes: &'a [TreeNode<String, String>],
        selected_id: Option<&'a String>,
        expanded_ids: &'a HashSet<String>,
        style: TreeViewStyle,
    ) -> TreeViewProps<'a, String, String, StateTreeAction> {
        TreeViewProps {
            nodes,
            selected_id,
            expanded_ids,
            is_focused: true,
            style,
            behavior: TreeViewBehavior::default(),
            measure_node: None, // No fixed column width
            column_padding: 0,
            on_select: |id| state_tree_select(id),
            on_toggle: |id, expanded| state_tree_toggle(id, expanded),
            render_node: &render_state_tree_node,
        }
    }

    fn apply_state_tree_actions(&mut self, actions: impl IntoIterator<Item = StateTreeAction>) {
        for action in actions {
            match action {
                StateTreeAction::Select(id) => {
                    self.state_tree_selected = Some(id);
                }
                StateTreeAction::Toggle(id, expand) => {
                    if expand {
                        self.state_tree_expanded.insert(id);
                    } else {
                        self.state_tree_expanded.remove(&id);
                    }
                }
            }
        }
    }

    fn save_state_snapshot<S: DebugState + 'static>(&mut self, state: Option<&S>) {
        let Some(state) = state else {
            self.freeze
                .set_message("State unavailable: call render_state() first");
            return;
        };

        let path = self.state_snapshot_path();

        let result = if let Some(snapshotter) = self.state_snapshotter.as_ref() {
            snapshotter(state as &dyn Any, &path)
        } else {
            let snapshot = DebugStateSnapshot::from_state(state, "Application State");
            crate::save_json(&path, &snapshot)
        };

        match result {
            Ok(()) => self
                .freeze
                .set_message(format!("Saved state: {}", path.display())),
            Err(err) => self
                .freeze
                .set_message(format!("State save failed: {err:?}")),
        }
    }

    fn state_snapshot_path(&self) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        PathBuf::from(format!("debug-state-{timestamp}.json"))
    }

    #[allow(dead_code)]
    fn update_table_scroll(&mut self, table: &DebugTableOverlay, visible_rows: usize) {
        self.table_page_size = visible_rows.max(1);
        let max_offset = table.rows.len().saturating_sub(visible_rows);
        self.table_scroll_offset = self.table_scroll_offset.min(max_offset);
    }

    fn overlay_modal_area(&self, app_area: Rect) -> Rect {
        let modal_width = (app_area.width * 80 / 100)
            .clamp(40, 160)
            .min(app_area.width);
        let modal_height = (app_area.height * 80 / 100)
            .clamp(12, 50)
            .min(app_area.height);
        centered_rect(modal_width, modal_height, app_area)
    }

    fn overlay_modal_style(&self) -> ModalStyle {
        ModalStyle {
            dim_factor: 0.0,
            base: BaseStyle {
                border: None,
                padding: Padding::default(), // No outer padding - title/footer at edges
                bg: Some(DebugStyle::overlay_bg()),
                fg: None,
            },
        }
    }

    fn render_overlay_container<F>(
        &self,
        frame: &mut Frame,
        app_area: Rect,
        title: &str,
        hints: Option<ModalHints>,
        footer_center_items: Option<Vec<StatusBarItem<'static>>>,
        mut render_body: F,
    ) where
        F: FnMut(&mut Frame, Rect),
    {
        let modal_area = self.overlay_modal_area(app_area);
        let mut modal = Modal::new();
        let mut render_content = |frame: &mut Frame, content_area: Rect| {
            if content_area.height == 0 || content_area.width == 0 {
                return;
            }

            // Render title bar (top)
            let content_area = self.render_overlay_title(frame, content_area, title);
            if content_area.height == 0 || content_area.width == 0 {
                return;
            }

            // Render footer bar (bottom) if hints provided
            let content_area = if let Some(hints) = hints {
                self.render_overlay_footer(
                    frame,
                    content_area,
                    hints,
                    footer_center_items.as_deref(),
                )
            } else {
                content_area
            };
            if content_area.height == 0 || content_area.width == 0 {
                return;
            }

            // Apply inner padding to body content only (not title/footer)
            let body_area = Rect {
                x: content_area.x.saturating_add(1),
                y: content_area.y,
                width: content_area.width.saturating_sub(2),
                height: content_area.height,
            };
            if body_area.width == 0 {
                return;
            }

            render_body(frame, body_area);
        };

        modal.render(
            frame,
            app_area,
            ModalProps {
                is_open: true,
                is_focused: false,
                area: modal_area,
                style: self.overlay_modal_style(),
                behavior: ModalBehavior::default(),
                on_close: || (),
                render_content: &mut render_content,
            },
        );
    }

    fn render_overlay_title(&self, frame: &mut Frame, area: Rect, title: &str) -> Rect {
        if area.height == 0 || area.width == 0 {
            return area;
        }

        use ratatui::text::Span;

        let title_style = Style::default()
            .fg(DebugStyle::accent())
            .add_modifier(Modifier::BOLD);
        let title_items = [StatusBarItem::span(Span::styled(
            format!(" {title} "),
            title_style,
        ))];
        let title_bar_style = StatusBarStyle {
            base: BaseStyle {
                border: None,
                padding: Padding::default(),
                bg: None,
                fg: None,
            },
            text: Style::default().fg(DebugStyle::accent()),
            hint_key: title_style,
            hint_label: Style::default().fg(DebugStyle::accent()),
            separator: Style::default().fg(DebugStyle::accent()),
        };
        let title_area = Rect { height: 1, ..area };

        let mut status_bar = StatusBar::new();
        <StatusBar as tui_dispatch_core::Component<()>>::render(
            &mut status_bar,
            frame,
            title_area,
            StatusBarProps {
                left: StatusBarSection::empty(),
                center: StatusBarSection::items(&title_items),
                right: StatusBarSection::empty(),
                style: title_bar_style,
                is_focused: false,
            },
        );

        Rect {
            x: area.x,
            y: area.y.saturating_add(title_area.height),
            width: area.width,
            height: area.height.saturating_sub(title_area.height),
        }
    }

    /// Render a modal footer status bar with hints.
    /// Returns the remaining content area (above the footer).
    fn render_overlay_footer(
        &self,
        frame: &mut Frame,
        area: Rect,
        hints: ModalHints,
        center_items: Option<&[StatusBarItem<'static>]>,
    ) -> Rect {
        if area.height < 2 || area.width == 0 {
            return area;
        }

        let footer_area = Rect {
            x: area.x,
            y: area.y.saturating_add(area.height.saturating_sub(1)),
            width: area.width,
            height: 1,
        };

        // Key style: bold, dark text on muted background
        let key_style = Style::default()
            .fg(DebugStyle::bg_deep())
            .bg(DebugStyle::text_secondary())
            .add_modifier(Modifier::BOLD);
        let label_style = Style::default().fg(DebugStyle::text_secondary());

        // Build left items from hints
        let mut left_items: Vec<StatusBarItem<'static>> = Vec::new();
        for hint in hints.left {
            left_items.push(StatusBarItem::span(Span::styled(
                format!(" {} ", hint.key),
                key_style,
            )));
            left_items.push(StatusBarItem::span(Span::styled(
                format!(" {} ", hint.label),
                label_style,
            )));
        }

        // Build right items from hints
        let mut right_items: Vec<StatusBarItem<'static>> = Vec::new();
        for hint in hints.right {
            right_items.push(StatusBarItem::span(Span::styled(
                format!(" {} ", hint.key),
                key_style,
            )));
            right_items.push(StatusBarItem::span(Span::styled(
                format!(" {} ", hint.label),
                label_style,
            )));
        }

        let footer_style = StatusBarStyle {
            base: BaseStyle {
                border: None,
                padding: Padding::default(),
                bg: Some(DebugStyle::overlay_bg_dark()),
                fg: None,
            },
            text: label_style,
            hint_key: key_style,
            hint_label: label_style,
            separator: label_style,
        };

        let left = StatusBarSection::items(&left_items).with_separator("");
        let center = if let Some(items) = center_items {
            StatusBarSection::items(items).with_separator("")
        } else {
            StatusBarSection::empty()
        };
        let right = if right_items.is_empty() {
            StatusBarSection::empty()
        } else {
            StatusBarSection::items(&right_items).with_separator("")
        };

        let mut status_bar = StatusBar::new();
        <StatusBar as tui_dispatch_core::Component<()>>::render(
            &mut status_bar,
            frame,
            footer_area,
            StatusBarProps {
                left,
                center,
                right,
                style: footer_style,
                is_focused: false,
            },
        );

        // Return area above footer
        Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height.saturating_sub(1),
        }
    }

    fn component_scrollbar_style(&self) -> ComponentScrollbarStyle {
        let style = &self.style.scrollbar;
        ComponentScrollbarStyle {
            thumb: style.thumb,
            track: style.track,
            begin: style.begin,
            end: style.end,
            thumb_symbol: style.thumb_symbol,
            track_symbol: style.track_symbol,
            begin_symbol: style.begin_symbol,
            end_symbol: style.end_symbol,
        }
    }

    fn table_page_size_value(&self) -> usize {
        self.table_page_size.max(1)
    }

    fn table_max_offset(&self, rows_len: usize) -> usize {
        rows_len.saturating_sub(self.table_page_size_value())
    }

    fn scroll_table_up(&mut self) {
        self.table_scroll_offset = self.table_scroll_offset.saturating_sub(1);
    }

    fn scroll_table_down(&mut self, rows_len: usize) {
        let max_offset = self.table_max_offset(rows_len);
        self.table_scroll_offset = (self.table_scroll_offset + 1).min(max_offset);
    }

    fn scroll_table_to_top(&mut self) {
        self.table_scroll_offset = 0;
    }

    fn scroll_table_to_bottom(&mut self, rows_len: usize) {
        self.table_scroll_offset = self.table_max_offset(rows_len);
    }

    fn scroll_table_page_up(&mut self) {
        let page_size = self.table_page_size_value();
        self.table_scroll_offset = self.table_scroll_offset.saturating_sub(page_size);
    }

    fn scroll_table_page_down(&mut self, rows_len: usize) {
        let page_size = self.table_page_size_value();
        let max_offset = self.table_max_offset(rows_len);
        self.table_scroll_offset = (self.table_scroll_offset + page_size).min(max_offset);
    }

    fn detail_page_size_value(&self) -> usize {
        self.detail_page_size.max(1)
    }

    fn detail_max_offset(&self, rows_len: usize) -> usize {
        rows_len.saturating_sub(self.detail_page_size_value())
    }

    fn scroll_detail_up(&mut self) {
        self.detail_scroll_offset = self.detail_scroll_offset.saturating_sub(1);
    }

    fn scroll_detail_down(&mut self, rows_len: usize) {
        let max_offset = self.detail_max_offset(rows_len);
        self.detail_scroll_offset = (self.detail_scroll_offset + 1).min(max_offset);
    }

    fn scroll_detail_to_top(&mut self) {
        self.detail_scroll_offset = 0;
    }

    fn scroll_detail_to_bottom(&mut self, rows_len: usize) {
        self.detail_scroll_offset = self.detail_max_offset(rows_len);
    }

    fn scroll_detail_page_up(&mut self) {
        let page_size = self.detail_page_size_value();
        self.detail_scroll_offset = self.detail_scroll_offset.saturating_sub(page_size);
    }

    fn scroll_detail_page_down(&mut self, rows_len: usize) {
        let page_size = self.detail_page_size_value();
        let max_offset = self.detail_max_offset(rows_len);
        self.detail_scroll_offset = (self.detail_scroll_offset + page_size).min(max_offset);
    }

    fn handle_table_scroll_key(&mut self, key: KeyCode, rows_len: usize) -> bool {
        match key {
            KeyCode::Char('j') | KeyCode::Down => {
                self.scroll_table_down(rows_len);
                true
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.scroll_table_up();
                true
            }
            KeyCode::Char('g') => {
                self.scroll_table_to_top();
                true
            }
            KeyCode::Char('G') => {
                self.scroll_table_to_bottom(rows_len);
                true
            }
            KeyCode::PageDown => {
                self.scroll_table_page_down(rows_len);
                true
            }
            KeyCode::PageUp => {
                self.scroll_table_page_up();
                true
            }
            _ => false,
        }
    }

    fn handle_detail_scroll_key(&mut self, key: KeyCode, rows_len: usize) -> bool {
        match key {
            KeyCode::Char('j') | KeyCode::Down => {
                self.scroll_detail_down(rows_len);
                true
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.scroll_detail_up();
                true
            }
            KeyCode::Char('g') => {
                self.scroll_detail_to_top();
                true
            }
            KeyCode::Char('G') => {
                self.scroll_detail_to_bottom(rows_len);
                true
            }
            KeyCode::PageDown => {
                self.scroll_detail_page_down(rows_len);
                true
            }
            KeyCode::PageUp => {
                self.scroll_detail_page_up();
                true
            }
            _ => false,
        }
    }

    fn handle_key_event<S: DebugState + 'static>(
        &mut self,
        key: KeyEvent,
        state: Option<&S>,
    ) -> Option<Vec<DebugSideEffect<A>>> {
        // Toggle key always works (even when disabled)
        if key.code == self.toggle_key && key.modifiers.is_empty() {
            let effect = self.toggle();
            return Some(effect.into_iter().collect());
        }

        // Other commands only work when enabled
        if !self.freeze.enabled {
            return None;
        }

        // Handle action-log specific search/navigation first.
        // We intentionally do a non-borrowing presence check before taking a mutable
        // overlay reference, so we can still call other `&mut self` helpers in this
        // function without extending the `log` borrow across the whole match block.
        if matches!(self.freeze.overlay, Some(DebugOverlay::ActionLog(_))) {
            let Some(DebugOverlay::ActionLog(ref mut log)) = self.freeze.overlay else {
                return Some(vec![]);
            };

            if log.search_input_active {
                let is_text_input_char = !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT);
                match key.code {
                    KeyCode::Esc | KeyCode::Enter => {
                        log.search_input_active = false;
                        return Some(vec![]);
                    }
                    KeyCode::Backspace => {
                        if !log.pop_search_char() {
                            log.search_input_active = false;
                        }
                        return Some(vec![]);
                    }
                    KeyCode::Char(c) if is_text_input_char => {
                        log.push_search_char(c);
                        return Some(vec![]);
                    }
                    _ => {
                        return Some(vec![]);
                    }
                }
            }

            // Backspace closes this overlay when not editing search query.
            if key.code == KeyCode::Backspace {
                self.handle_action(DebugAction::CloseOverlay);
                return Some(vec![]);
            }

            if key.code == KeyCode::Char('/') {
                log.search_input_active = true;
                return Some(vec![]);
            }

            // Crossterm normalizes Shift+n to `Char('N')`.
            let prev_match = key.code == KeyCode::Char('N');
            if prev_match {
                log.search_prev();
                return Some(vec![]);
            }

            if key.code == KeyCode::Char('n') {
                log.search_next();
                return Some(vec![]);
            }

            let action = match key.code {
                KeyCode::Char('j') | KeyCode::Down => Some(DebugAction::ActionLogScrollDown),
                KeyCode::Char('k') | KeyCode::Up => Some(DebugAction::ActionLogScrollUp),
                KeyCode::Char('g') => Some(DebugAction::ActionLogScrollTop),
                KeyCode::Char('G') => Some(DebugAction::ActionLogScrollBottom),
                KeyCode::PageDown => Some(DebugAction::ActionLogPageDown),
                KeyCode::PageUp => Some(DebugAction::ActionLogPageUp),
                KeyCode::Enter => Some(DebugAction::ActionLogShowDetail),
                _ => None,
            };
            if let Some(action) = action {
                self.handle_action(action);
                return Some(vec![]);
            }
        }

        // Esc toggles debug mode when action-log search input did not consume it.
        if key.code == KeyCode::Esc {
            let effect = self.toggle();
            return Some(effect.into_iter().collect());
        }

        match key.code {
            KeyCode::Char('b') | KeyCode::Char('B') => {
                self.banner_position = self.banner_position.toggle();
                self.freeze.request_capture();
                return Some(vec![]);
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                if matches!(self.freeze.overlay, Some(DebugOverlay::State(_))) {
                    self.freeze.clear_overlay();
                } else if let Some(state) = state {
                    let table = state.build_debug_table("Application State");
                    self.set_state_overlay(table);
                } else if let Some(ref table) = self.state_snapshot {
                    self.set_state_overlay(table.clone());
                } else {
                    let table = DebugTableBuilder::new()
                        .section("State")
                        .entry(
                            "hint",
                            "Press 's' after providing state via render_with_state() or show_state_overlay()",
                        )
                        .finish("Application State");
                    self.freeze.set_overlay(DebugOverlay::State(table));
                }
                return Some(vec![]);
            }
            KeyCode::Char('w') | KeyCode::Char('W') => {
                self.save_state_snapshot(state);
                return Some(vec![]);
            }
            _ => {}
        }

        // Handle internal debug commands (hardcoded keys).
        // Note: this runs after action-log search handling so text input in `/`
        // mode can accept letters like 'A' without toggling the overlay.
        let action = match key.code {
            KeyCode::Char('a') | KeyCode::Char('A') => Some(DebugAction::ToggleActionLog),
            KeyCode::Char('y') | KeyCode::Char('Y') => Some(DebugAction::CopyFrame),
            KeyCode::Char('i') | KeyCode::Char('I') => Some(DebugAction::ToggleMouseCapture),
            KeyCode::Char('q') | KeyCode::Char('Q') => Some(DebugAction::CloseOverlay),
            _ => None,
        };

        if let Some(action) = action {
            let effect = self.handle_action(action);
            return Some(effect.into_iter().collect());
        }

        // Handle remaining overlay-specific navigation.
        match &self.freeze.overlay {
            Some(DebugOverlay::State(_)) => {
                // Backspace closes this overlay
                if key.code == KeyCode::Backspace {
                    self.handle_action(DebugAction::CloseOverlay);
                    return Some(vec![]);
                }
                self.sync_state_tree_state();
                let nodes = &self.state_tree_nodes;
                let selected_id = self.state_tree_selected.as_ref();
                let expanded_ids = &self.state_tree_expanded;
                let style = self.state_tree_style();
                let props = Self::build_state_tree_props(nodes, selected_id, expanded_ids, style);
                let actions: Vec<_> = self
                    .state_tree_view
                    .handle_event(&EventKind::Key(key), props)
                    .into_iter()
                    .collect();
                if !actions.is_empty() {
                    self.apply_state_tree_actions(actions);
                    return Some(vec![]);
                }
            }
            Some(DebugOverlay::Inspect(table)) => {
                // Backspace closes this overlay
                if key.code == KeyCode::Backspace {
                    self.handle_action(DebugAction::CloseOverlay);
                    return Some(vec![]);
                }
                if self.handle_table_scroll_key(key.code, table.rows.len()) {
                    return Some(vec![]);
                }
            }
            Some(DebugOverlay::ActionDetail(detail)) => {
                // Back to action log on Esc, Backspace, or Enter
                if matches!(key.code, KeyCode::Esc | KeyCode::Backspace | KeyCode::Enter) {
                    self.handle_action(DebugAction::ActionLogBackToList);
                    return Some(vec![]);
                }

                let params_lines = self.detail_params_lines(detail);
                if self.handle_detail_scroll_key(key.code, params_lines.len()) {
                    return Some(vec![]);
                }
            }
            _ => {}
        }

        // Consume all key events when debug is enabled
        Some(vec![])
    }

    fn toggle(&mut self) -> Option<DebugSideEffect<A>> {
        if self.freeze.enabled {
            // Disable: resume tasks/subs
            #[cfg(feature = "subscriptions")]
            if let Some(ref handle) = self.sub_handle {
                handle.resume();
            }

            #[cfg(feature = "tasks")]
            let task_queued = if let Some(ref handle) = self.task_handle {
                handle.resume()
            } else {
                vec![]
            };
            #[cfg(not(feature = "tasks"))]
            let task_queued: Vec<A> = vec![];

            let queued = self.freeze.take_queued();
            self.freeze.disable();
            self.state_snapshot = None;
            self.state_tree_nodes.clear();
            self.state_tree_selected = None;
            self.state_tree_expanded.clear();
            self.state_tree_view = TreeView::new();
            self.table_scroll_offset = 0;
            self.table_page_size = 1;
            self.detail_scroll_offset = 0;
            self.detail_page_size = 1;

            // Combine queued actions from freeze and task manager
            let mut all_queued = queued;
            all_queued.extend(task_queued);

            if all_queued.is_empty() {
                None
            } else {
                Some(DebugSideEffect::ProcessQueuedActions(all_queued))
            }
        } else {
            // Enable: pause tasks/subs
            #[cfg(feature = "tasks")]
            if let Some(ref handle) = self.task_handle {
                handle.pause();
            }
            #[cfg(feature = "subscriptions")]
            if let Some(ref handle) = self.sub_handle {
                handle.pause();
            }
            self.freeze.enable();
            self.state_snapshot = None;
            self.state_tree_nodes.clear();
            self.state_tree_selected = None;
            self.state_tree_expanded.clear();
            self.state_tree_view = TreeView::new();
            self.table_scroll_offset = 0;
            self.table_page_size = 1;
            self.detail_scroll_offset = 0;
            self.detail_page_size = 1;
            None
        }
    }

    fn handle_action(&mut self, action: DebugAction) -> Option<DebugSideEffect<A>> {
        match action {
            DebugAction::Toggle => self.toggle(),
            DebugAction::CopyFrame => {
                let text = &self.freeze.snapshot_text;
                // Use OSC52 escape sequence to copy to clipboard
                let encoded = BASE64_STANDARD.encode(text);
                print!("\x1b]52;c;{}\x07", encoded);
                std::io::stdout().flush().ok();
                self.freeze.set_message("Copied to clipboard");
                None
            }
            DebugAction::ToggleState => {
                if matches!(self.freeze.overlay, Some(DebugOverlay::State(_))) {
                    self.freeze.clear_overlay();
                } else if let Some(ref table) = self.state_snapshot {
                    self.set_state_overlay(table.clone());
                } else {
                    // Show placeholder - user should call show_state_overlay()
                    let table = DebugTableBuilder::new()
                        .section("State")
                        .entry(
                            "hint",
                            "Press 's' after providing state via render_with_state() or show_state_overlay()",
                        )
                        .finish("Application State");
                    self.freeze.set_overlay(DebugOverlay::State(table));
                }
                None
            }
            DebugAction::ToggleActionLog => {
                if matches!(self.freeze.overlay, Some(DebugOverlay::ActionLog(_))) {
                    self.freeze.clear_overlay();
                } else {
                    self.show_action_log();
                }
                None
            }
            DebugAction::ActionLogScrollUp => {
                if let Some(DebugOverlay::ActionLog(ref mut log)) = self.freeze.overlay {
                    log.scroll_up();
                }
                None
            }
            DebugAction::ActionLogScrollDown => {
                if let Some(DebugOverlay::ActionLog(ref mut log)) = self.freeze.overlay {
                    log.scroll_down();
                }
                None
            }
            DebugAction::ActionLogScrollTop => {
                if let Some(DebugOverlay::ActionLog(ref mut log)) = self.freeze.overlay {
                    log.scroll_to_top();
                }
                None
            }
            DebugAction::ActionLogScrollBottom => {
                if let Some(DebugOverlay::ActionLog(ref mut log)) = self.freeze.overlay {
                    log.scroll_to_bottom();
                }
                None
            }
            DebugAction::ActionLogPageUp => {
                if let Some(DebugOverlay::ActionLog(ref mut log)) = self.freeze.overlay {
                    log.page_up(10);
                }
                None
            }
            DebugAction::ActionLogPageDown => {
                if let Some(DebugOverlay::ActionLog(ref mut log)) = self.freeze.overlay {
                    log.page_down(10);
                }
                None
            }
            DebugAction::ActionLogShowDetail => {
                if let Some(DebugOverlay::ActionLog(ref log)) = self.freeze.overlay {
                    if let Some(detail) = log.selected_detail() {
                        self.detail_scroll_offset = 0;
                        self.detail_page_size = 1;
                        self.freeze.set_overlay(DebugOverlay::ActionDetail(detail));
                    }
                }
                None
            }
            DebugAction::ActionLogBackToList => {
                // Go back to action log from detail view
                if matches!(self.freeze.overlay, Some(DebugOverlay::ActionDetail(_))) {
                    self.show_action_log();
                }
                None
            }
            DebugAction::ToggleMouseCapture => {
                self.freeze.toggle_mouse_capture();
                None
            }
            DebugAction::InspectCell { column, row } => {
                if let Some(ref snapshot) = self.freeze.snapshot {
                    let overlay = self.build_inspect_overlay(column, row, snapshot);
                    self.table_scroll_offset = 0;
                    self.freeze.set_overlay(DebugOverlay::Inspect(overlay));
                }
                self.freeze.mouse_capture_enabled = false;
                None
            }
            DebugAction::CloseOverlay => {
                self.freeze.clear_overlay();
                None
            }
            DebugAction::RequestCapture => {
                self.freeze.request_capture();
                None
            }
        }
    }

    fn split_for_banner(&self, area: Rect) -> (Rect, Rect) {
        let banner_height = area.height.min(1);
        match self.banner_position {
            BannerPosition::Bottom => {
                let app_area = Rect {
                    height: area.height.saturating_sub(banner_height),
                    ..area
                };
                let banner_area = Rect {
                    y: area.y.saturating_add(app_area.height),
                    height: banner_height,
                    ..area
                };
                (app_area, banner_area)
            }
            BannerPosition::Top => {
                let banner_area = Rect {
                    y: area.y,
                    height: banner_height,
                    ..area
                };
                let app_area = Rect {
                    y: area.y.saturating_add(banner_height),
                    height: area.height.saturating_sub(banner_height),
                    ..area
                };
                (app_area, banner_area)
            }
        }
    }

    fn render_debug_overlay(&mut self, frame: &mut Frame, app_area: Rect, banner_area: Rect) {
        let overlay = self.freeze.overlay.clone();

        // Only dim when there's an overlay open
        if let Some(ref overlay) = overlay {
            dim_buffer(frame.buffer_mut(), self.style.dim_factor);

            match overlay {
                DebugOverlay::Inspect(table) => {
                    self.render_table_modal(frame, app_area, table);
                }
                DebugOverlay::State(table) => {
                    self.render_state_tree_modal(frame, app_area, table);
                }
                DebugOverlay::ActionLog(log) => {
                    self.render_action_log_modal(frame, app_area, log);
                }
                DebugOverlay::ActionDetail(detail) => {
                    self.render_action_detail_modal(frame, app_area, detail);
                }
            }
        }

        // Render banner
        self.render_banner(frame, banner_area);
    }

    fn render_banner(&self, frame: &mut Frame, banner_area: Rect) {
        if banner_area.height == 0 {
            return;
        }

        use ratatui::text::Span;

        let keys = &self.style.key_styles;
        let toggle_key_str = format_key(self.toggle_key);
        let label_style = self.style.label_style;
        let value_style = self.style.value_style;

        let mut left_items: Vec<StatusBarItem<'static>> = Vec::new();
        left_items.push(StatusBarItem::span(Span::styled(
            " DEBUG ",
            self.style.title_style,
        )));
        left_items.push(StatusBarItem::text(" "));

        let push_item =
            |items: &mut Vec<StatusBarItem<'static>>, key: &str, label: &str, key_style: Style| {
                items.push(StatusBarItem::span(Span::styled(
                    format!(" {key} "),
                    key_style,
                )));
                items.push(StatusBarItem::span(Span::styled(
                    format!(" {label} "),
                    label_style,
                )));
            };

        push_item(&mut left_items, &toggle_key_str, "resume", keys.toggle);
        push_item(&mut left_items, "a", "actions", keys.actions);
        push_item(&mut left_items, "s", "state", keys.state);
        push_item(
            &mut left_items,
            "b",
            self.banner_position.label(),
            keys.actions,
        );
        push_item(&mut left_items, "y", "copy", keys.copy);

        if self.freeze.mouse_capture_enabled {
            push_item(&mut left_items, "click", "inspect", keys.mouse);
        } else {
            push_item(&mut left_items, "i", "mouse", keys.mouse);
        }

        let mut right_items: Vec<StatusBarItem<'static>> = Vec::new();
        if let Some(ref msg) = self.freeze.message {
            right_items.push(StatusBarItem::span(Span::styled(
                format!(" {msg} "),
                value_style,
            )));
        }

        let style = StatusBarStyle {
            base: BaseStyle {
                border: None,
                padding: Padding::default(),
                bg: self.style.banner_bg.bg,
                fg: None,
            },
            text: label_style,
            hint_key: keys.toggle,
            hint_label: label_style,
            separator: label_style,
        };

        let left = StatusBarSection::items(&left_items).with_separator("");
        let right = if right_items.is_empty() {
            StatusBarSection::empty()
        } else {
            StatusBarSection::items(&right_items).with_separator("")
        };

        let mut status_bar = StatusBar::new();
        <StatusBar as tui_dispatch_core::Component<()>>::render(
            &mut status_bar,
            frame,
            banner_area,
            StatusBarProps {
                left,
                center: StatusBarSection::empty(),
                right,
                style,
                is_focused: false,
            },
        );
    }

    fn render_state_tree_modal(
        &mut self,
        frame: &mut Frame,
        app_area: Rect,
        table: &DebugTableOverlay,
    ) {
        self.sync_state_tree_state();

        // Inline the overlay container logic to avoid borrow issues
        let modal_area = self.overlay_modal_area(app_area);
        let modal_style = self.overlay_modal_style();
        let title = &table.title;

        let mut modal = Modal::new();
        let mut render_content = |frame: &mut Frame, content_area: Rect| {
            if content_area.height == 0 || content_area.width == 0 {
                return;
            }

            // Render title bar (top)
            let content_area = self.render_overlay_title(frame, content_area, title);
            if content_area.height == 0 || content_area.width == 0 {
                return;
            }

            // Render footer bar (bottom)
            let content_area =
                self.render_overlay_footer(frame, content_area, STATE_TREE_HINTS, None);
            if content_area.height == 0 || content_area.width == 0 {
                return;
            }

            // Apply inner padding to body content only (not title/footer)
            let body_area = Rect {
                x: content_area.x.saturating_add(1),
                y: content_area.y,
                width: content_area.width.saturating_sub(2),
                height: content_area.height,
            };
            if body_area.width == 0 {
                return;
            }

            let nodes = &self.state_tree_nodes;
            let selected_id = self.state_tree_selected.as_ref();
            let expanded_ids = &self.state_tree_expanded;
            let style = self.state_tree_style();
            let props = Self::build_state_tree_props(nodes, selected_id, expanded_ids, style);

            <TreeView<String> as tui_dispatch_core::Component<StateTreeAction>>::render(
                &mut self.state_tree_view,
                frame,
                body_area,
                props,
            );
        };

        modal.render(
            frame,
            app_area,
            ModalProps {
                is_open: true,
                is_focused: false,
                area: modal_area,
                style: modal_style,
                behavior: ModalBehavior::default(),
                on_close: || (),
                render_content: &mut render_content,
            },
        );
    }

    fn render_table_modal(&mut self, frame: &mut Frame, app_area: Rect, table: &DebugTableOverlay) {
        let scrollbar_style = self.component_scrollbar_style();
        let table_scroll_offset = self.table_scroll_offset;

        self.render_overlay_container(
            frame,
            app_area,
            &table.title,
            Some(INSPECT_HINTS),
            None,
            |frame, content_area| {
                let mut table_area = content_area;
                if let Some(ref preview) = table.cell_preview {
                    if table_area.height > 1 {
                        let preview_area = Rect {
                            height: 1,
                            ..table_area
                        };
                        let preview_widget = CellPreviewWidget::new(preview)
                            .label_style(Style::default().fg(DebugStyle::text_secondary()))
                            .value_style(Style::default().fg(DebugStyle::text_primary()));
                        frame.render_widget(preview_widget, preview_area);

                        table_area = Rect {
                            y: table_area.y.saturating_add(1),
                            height: table_area.height.saturating_sub(1),
                            ..table_area
                        };
                    }
                }

                if table_area.height == 0 || table_area.width == 0 {
                    return;
                }

                use ratatui::text::{Line, Span};
                use ratatui::widgets::Paragraph;

                let header_area = Rect {
                    height: 1,
                    ..table_area
                };
                let rows_area = Rect {
                    y: table_area.y.saturating_add(1),
                    height: table_area.height.saturating_sub(1),
                    ..table_area
                };

                let style = DebugTableStyle::default();
                let show_scrollbar = rows_area.height > 0
                    && table.rows.len() > rows_area.height as usize
                    && table_area.width > 1;
                let text_width = if show_scrollbar {
                    table_area.width.saturating_sub(1)
                } else {
                    table_area.width
                } as usize;

                if text_width == 0 {
                    return;
                }

                let max_key_len = table
                    .rows
                    .iter()
                    .filter_map(|row| match row {
                        super::table::DebugTableRow::Entry { key, .. } => Some(key.chars().count()),
                        super::table::DebugTableRow::Section(_) => None,
                    })
                    .max()
                    .unwrap_or(0);
                let max_label = text_width.saturating_sub(8).max(10);
                let label_width = (max_key_len + 2).clamp(12, 30).min(max_label);
                let value_width = text_width.saturating_sub(label_width + 2).max(1);

                let header_line = Line::from(vec![
                    Span::styled(pad_text("Field", label_width), style.header),
                    Span::styled("  ", style.header),
                    Span::styled(pad_text("Value", value_width), style.header),
                ]);
                frame.render_widget(Paragraph::new(header_line), header_area);

                if rows_area.height == 0 {
                    return;
                }

                let syntax_style = DebugSyntaxStyle::with_base(style.value);
                let mut rows = Vec::new();
                let mut entry_index = 0usize;
                for row in table.rows.iter() {
                    match row {
                        super::table::DebugTableRow::Section(title) => {
                            entry_index = 0;
                            let mut text = format!(" {title} ");
                            text = truncate_with_ellipsis(&text, text_width);
                            text = pad_text(&text, text_width);
                            rows.push(Line::from(vec![Span::styled(text, style.section)]));
                        }
                        super::table::DebugTableRow::Entry { key, value } => {
                            let row_style = if entry_index % 2 == 0 {
                                style.row_styles.0
                            } else {
                                style.row_styles.1
                            };
                            entry_index = entry_index.saturating_add(1);
                            let key_text = pad_text(key, label_width);
                            let mut value_text = truncate_with_ellipsis(value, value_width);
                            value_text = pad_text(&value_text, value_width);

                            let mut value_spans: Vec<Span<'static>> =
                                debug_spans(&value_text, &syntax_style)
                                    .into_iter()
                                    .map(|span| span.patch_style(row_style))
                                    .collect();

                            let mut spans = vec![
                                Span::styled(key_text, style.key).patch_style(row_style),
                                Span::styled("  ", row_style),
                            ];
                            spans.append(&mut value_spans);

                            rows.push(Line::from(spans));
                        }
                    }
                }

                // Note: page size update removed to avoid borrow issues
                let scroll_style = ScrollViewStyle {
                    base: BaseStyle {
                        border: None,
                        padding: Padding::default(),
                        bg: None,
                        fg: None,
                    },
                    scrollbar: scrollbar_style.clone(),
                };
                let scroller = LinesScroller::new(&rows);
                let mut scroll_view = ScrollView::new();
                <ScrollView as tui_dispatch_core::Component<()>>::render(
                    &mut scroll_view,
                    frame,
                    rows_area,
                    ScrollViewProps {
                        content_height: scroller.content_height(),
                        scroll_offset: table_scroll_offset,
                        is_focused: true,
                        style: scroll_style,
                        behavior: ScrollViewBehavior::default(),
                        on_scroll: |_| (),
                        render_content: &mut scroller.renderer(),
                    },
                );
            },
        );
    }

    fn render_action_log_modal(&self, frame: &mut Frame, app_area: Rect, log: &ActionLogOverlay) {
        let entry_count = log.entries.len();
        let filter_active = log.has_search_query();
        let filtered_count = if filter_active {
            log.search_match_count()
        } else {
            entry_count
        };
        let title = if log.has_search_query() {
            format!("{} ({} / {} shown)", log.title, filtered_count, entry_count)
        } else if entry_count > 0 {
            format!("{} ({} entries)", log.title, entry_count)
        } else {
            format!("{} (empty)", log.title)
        };

        let action_log_hints = if log.search_input_active {
            ACTION_LOG_SEARCH_INPUT_HINTS
        } else {
            ACTION_LOG_HINTS
        };

        self.render_overlay_container(
            frame,
            app_area,
            &title,
            Some(action_log_hints),
            self.action_log_footer_center_items(log),
            |frame, log_area| {
                use ratatui::text::{Line, Span};
                use ratatui::widgets::Paragraph;

                let style = ActionLogStyle::default();
                let spacing = 1usize;
                let seq_width = 5usize;
                let name_width = 20usize;
                let elapsed_width = 8usize;

                let header_area = Rect {
                    height: 1,
                    ..log_area
                };
                let body_area = Rect {
                    y: log_area.y.saturating_add(1),
                    height: log_area.height.saturating_sub(1),
                    ..log_area
                };

                let selected_visible_idx = if filtered_count == 0 {
                    0
                } else if filter_active {
                    let selected_match_position = log
                        .search_matches
                        .iter()
                        .position(|&idx| idx == log.selected);
                    // Defensive fallback in case selection/matches drift out of sync.
                    selected_match_position
                        .unwrap_or(log.search_match_index.min(filtered_count.saturating_sub(1)))
                } else {
                    log.selected.min(filtered_count.saturating_sub(1))
                };

                let show_scrollbar = body_area.height > 0
                    && filtered_count > body_area.height as usize
                    && log_area.width > 1;
                let text_width = if show_scrollbar {
                    log_area.width.saturating_sub(1)
                } else {
                    log_area.width
                } as usize;
                let params_width = text_width
                    .saturating_sub(seq_width + name_width + elapsed_width + spacing * 3)
                    .max(1);

                let header_line = Line::from(vec![
                    Span::styled(pad_text("#", seq_width), style.header),
                    Span::styled(" ", style.header),
                    Span::styled(pad_text("Action", name_width), style.header),
                    Span::styled(" ", style.header),
                    Span::styled(pad_text("Params", params_width), style.header),
                    Span::styled(" ", style.header),
                    Span::styled(pad_text("Elapsed", elapsed_width), style.header),
                ]);
                frame.render_widget(Paragraph::new(header_line), header_area);

                if body_area.height == 0 {
                    return;
                }

                let syntax_style = DebugSyntaxStyle::with_base(style.params);
                let rows: Vec<Line> = if filtered_count == 0 {
                    vec![Line::from(vec![Span::styled(
                        " (no matching actions) ",
                        Style::default().fg(DebugStyle::text_secondary()),
                    )])]
                } else {
                    (0..filtered_count)
                        .map(|visible_idx| {
                            let entry_idx = if filter_active {
                                log.search_matches[visible_idx]
                            } else {
                                visible_idx
                            };
                            let entry = &log.entries[entry_idx];
                            let is_selected = visible_idx == selected_visible_idx;
                            let row_style = if is_selected {
                                style.selected
                            } else if visible_idx % 2 == 0 {
                                style.row_styles.0
                            } else {
                                style.row_styles.1
                            };

                            let seq_text = pad_text(&entry.sequence.to_string(), seq_width);
                            let name_text = pad_text(&entry.name, name_width);
                            let elapsed_text = pad_text(&entry.elapsed, elapsed_width);

                            let params_compact = entry.params.replace('\n', " ");
                            let params_compact = params_compact
                                .split_whitespace()
                                .collect::<Vec<_>>()
                                .join(" ");
                            let params_trimmed =
                                truncate_with_ellipsis(&params_compact, params_width);
                            let params_text = pad_text(&params_trimmed, params_width);
                            let mut params_spans: Vec<Span<'static>> =
                                debug_spans(&params_text, &syntax_style)
                                    .into_iter()
                                    .map(|span| span.patch_style(row_style))
                                    .collect();

                            let mut spans = vec![
                                Span::styled(seq_text, style.sequence).patch_style(row_style),
                                Span::styled(" ", row_style),
                                Span::styled(name_text, style.name).patch_style(row_style),
                                Span::styled(" ", row_style),
                            ];
                            spans.append(&mut params_spans);
                            spans.push(Span::styled(" ", row_style));
                            spans.push(
                                Span::styled(elapsed_text, style.elapsed).patch_style(row_style),
                            );

                            Line::from(spans)
                        })
                        .collect()
                };

                let scroll_style = ScrollViewStyle {
                    base: BaseStyle {
                        border: None,
                        padding: Padding::default(),
                        bg: None,
                        fg: None,
                    },
                    scrollbar: self.component_scrollbar_style(),
                };
                let content_height = rows.len();
                let visible_rows = body_area.height as usize;
                let scroll_offset = if visible_rows == 0 || selected_visible_idx < visible_rows {
                    0
                } else {
                    selected_visible_idx - visible_rows + 1
                };
                let scroller = LinesScroller::new(&rows);
                let mut scroll_view = ScrollView::new();
                <ScrollView as tui_dispatch_core::Component<()>>::render(
                    &mut scroll_view,
                    frame,
                    body_area,
                    ScrollViewProps {
                        content_height,
                        scroll_offset,
                        is_focused: true,
                        style: scroll_style,
                        behavior: ScrollViewBehavior::default(),
                        on_scroll: |_| (),
                        render_content: &mut scroller.renderer(),
                    },
                );
            },
        );
    }

    fn action_log_footer_center_items(
        &self,
        log: &ActionLogOverlay,
    ) -> Option<Vec<StatusBarItem<'static>>> {
        use ratatui::text::Span;

        let key_style = Style::default()
            .fg(DebugStyle::bg_deep())
            .bg(DebugStyle::text_secondary())
            .add_modifier(Modifier::BOLD);
        let label_style = Style::default().fg(DebugStyle::text_secondary());
        let value_style = Style::default().fg(DebugStyle::text_primary());

        let filter_value = if log.search_input_active {
            format!("/{}_", log.search_query)
        } else if log.search_query.is_empty() {
            "off".to_string()
        } else {
            format!("/{}", log.search_query)
        };

        let matches = if log.search_query.is_empty() {
            format!("{}", log.entries.len())
        } else {
            format!("{}", log.search_match_count())
        };

        Some(vec![
            StatusBarItem::span(Span::styled(" filter ", key_style)),
            StatusBarItem::span(Span::styled(format!(" {} ", filter_value), value_style)),
            StatusBarItem::span(Span::styled(" matches ", key_style)),
            StatusBarItem::span(Span::styled(format!(" {} ", matches), label_style)),
        ])
    }

    fn render_action_detail_modal(
        &mut self,
        frame: &mut Frame,
        app_area: Rect,
        detail: &super::table::ActionDetailOverlay,
    ) {
        let title = format!("Action #{} - {}", detail.sequence, detail.name);
        let param_lines = self.detail_params_lines(detail);
        let scrollbar_style = self.component_scrollbar_style();
        let detail_scroll_offset = self.detail_scroll_offset;
        let detail_page_size = self.detail_page_size;

        self.render_overlay_container(
            frame,
            app_area,
            &title,
            Some(ACTION_DETAIL_HINTS),
            None,
            |frame, detail_area| {
                use ratatui::text::{Line, Span};
                use ratatui::widgets::Paragraph;

                let label_style = Style::default().fg(DebugStyle::text_secondary());
                let value_style = Style::default().fg(DebugStyle::text_primary());

                let header_lines = vec![
                    Line::from(vec![
                        Span::styled("Name: ", label_style),
                        Span::styled(&detail.name, value_style),
                    ]),
                    Line::from(vec![
                        Span::styled("Sequence: ", label_style),
                        Span::styled(detail.sequence.to_string(), value_style),
                    ]),
                    Line::from(vec![
                        Span::styled("Elapsed: ", label_style),
                        Span::styled(&detail.elapsed, value_style),
                    ]),
                    Line::from(""),
                    Line::from(Span::styled("Parameters:", label_style)),
                ];

                let mut param_lines = param_lines.clone();
                if param_lines.is_empty() {
                    param_lines.push(Line::from(Span::styled("  (none)", value_style)));
                }
                let param_lines_len = param_lines.len();

                let footer_lines = vec![Line::from("")];

                let header_height = header_lines.len() as u16;
                let footer_height = footer_lines.len() as u16;

                let header_area_height = header_height.min(detail_area.height);
                let header_area = Rect {
                    height: header_area_height,
                    ..detail_area
                };
                if header_area.height > 0 {
                    let paragraph = Paragraph::new(header_lines);
                    frame.render_widget(paragraph, header_area);
                }

                let footer_area_height =
                    footer_height.min(detail_area.height.saturating_sub(header_area_height));
                let footer_area = Rect {
                    x: detail_area.x,
                    y: detail_area
                        .y
                        .saturating_add(detail_area.height.saturating_sub(footer_area_height)),
                    width: detail_area.width,
                    height: footer_area_height,
                };
                if footer_area.height > 0 {
                    let paragraph = Paragraph::new(footer_lines);
                    frame.render_widget(paragraph, footer_area);
                }

                let params_area = Rect {
                    x: detail_area.x,
                    y: detail_area.y.saturating_add(header_area_height),
                    width: detail_area.width,
                    height: detail_area
                        .height
                        .saturating_sub(header_area_height + footer_area_height),
                };

                // Note: page size and scroll offset updates removed to avoid borrow issues
                let max_offset = param_lines_len.saturating_sub(detail_page_size.max(1));
                let current_scroll_offset = detail_scroll_offset.min(max_offset);

                if params_area.height > 0 {
                    let scroll_style = ScrollViewStyle {
                        base: BaseStyle {
                            border: None,
                            padding: Padding::default(),
                            bg: Some(DebugStyle::overlay_bg_dark()),
                            fg: None,
                        },
                        scrollbar: scrollbar_style.clone(),
                    };

                    let scroller = LinesScroller::new(&param_lines);
                    let mut scroll_view = ScrollView::new();
                    <ScrollView as tui_dispatch_core::Component<()>>::render(
                        &mut scroll_view,
                        frame,
                        params_area,
                        ScrollViewProps {
                            content_height: param_lines_len,
                            scroll_offset: current_scroll_offset,
                            is_focused: true,
                            style: scroll_style,
                            behavior: ScrollViewBehavior::default(),
                            on_scroll: |_| (),
                            render_content: &mut scroller.renderer(),
                        },
                    );
                }
            },
        );
    }

    fn detail_params_lines(
        &self,
        detail: &super::table::ActionDetailOverlay,
    ) -> Vec<ratatui::text::Line<'static>> {
        use ratatui::text::{Line, Span};

        if detail.params.is_empty() {
            return Vec::new();
        }

        let value_style = Style::default().fg(DebugStyle::text_primary());
        let syntax_style = DebugSyntaxStyle::with_base(value_style);

        detail
            .params
            .lines()
            .map(|line| {
                let mut spans = vec![Span::styled("  ", value_style)];
                spans.extend(debug_spans(line, &syntax_style));
                Line::from(spans)
            })
            .collect()
    }

    fn build_inspect_overlay(&self, column: u16, row: u16, snapshot: &Buffer) -> DebugTableOverlay {
        let mut builder = DebugTableBuilder::new();

        builder.push_section("Position");
        builder.push_entry("column", column.to_string());
        builder.push_entry("row", row.to_string());

        if let Some(preview) = inspect_cell(snapshot, column, row) {
            builder.set_cell_preview(preview);
        }

        builder.finish(format!("Inspect ({column}, {row})"))
    }
}

#[derive(Debug, Serialize)]
struct DebugStateSnapshot {
    title: String,
    sections: Vec<DebugStateSnapshotSection>,
}

#[derive(Debug, Serialize)]
struct DebugStateSnapshotSection {
    title: String,
    entries: Vec<DebugStateSnapshotEntry>,
}

#[derive(Debug, Serialize)]
struct DebugStateSnapshotEntry {
    key: String,
    value: String,
}

impl DebugStateSnapshot {
    fn from_state<S: DebugState>(state: &S, title: &str) -> Self {
        let sections = state
            .debug_sections()
            .into_iter()
            .map(|section| DebugStateSnapshotSection {
                title: section.title,
                entries: section
                    .entries
                    .into_iter()
                    .map(|entry| DebugStateSnapshotEntry {
                        key: entry.key,
                        value: entry.value,
                    })
                    .collect(),
            })
            .collect();

        Self {
            title: title.to_string(),
            sections,
        }
    }
}

fn pad_text(value: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let mut text: String = value.chars().take(width).collect();
    let len = text.chars().count();
    if len < width {
        text.push_str(&" ".repeat(width - len));
    }
    text
}

fn truncate_with_ellipsis(value: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let count = value.chars().count();
    if count <= width {
        return value.to_string();
    }
    if width <= 3 {
        return value.chars().take(width).collect();
    }
    let mut text: String = value.chars().take(width - 3).collect();
    text.push_str("...");
    text
}

/// Format a KeyCode for display in the banner.
fn format_key(key: KeyCode) -> String {
    match key {
        KeyCode::F(n) => format!("F{}", n),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::Backspace => "Bksp".to_string(),
        KeyCode::Delete => "Del".to_string(),
        KeyCode::Up => "↑".to_string(),
        KeyCode::Down => "↓".to_string(),
        KeyCode::Left => "←".to_string(),
        KeyCode::Right => "→".to_string(),
        _ => format!("{:?}", key),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    enum TestAction {
        Foo,
        Bar,
    }

    impl tui_dispatch_core::Action for TestAction {
        fn name(&self) -> &'static str {
            match self {
                TestAction::Foo => "Foo",
                TestAction::Bar => "Bar",
            }
        }
    }

    impl tui_dispatch_core::ActionParams for TestAction {
        fn params(&self) -> String {
            String::new()
        }
    }

    #[test]
    fn test_debug_layer_creation() {
        let layer: DebugLayer<TestAction> = DebugLayer::new(KeyCode::F(12));
        assert!(!layer.is_enabled());
        assert!(layer.freeze().snapshot.is_none());
    }

    #[test]
    fn test_toggle() {
        let mut layer: DebugLayer<TestAction> = DebugLayer::new(KeyCode::F(12));

        // Enable
        let effect = layer.toggle();
        assert!(effect.is_none());
        assert!(layer.is_enabled());

        // Disable
        let effect = layer.toggle();
        assert!(effect.is_none()); // No queued actions
        assert!(!layer.is_enabled());
    }

    #[test]
    fn test_set_enabled() {
        let mut layer: DebugLayer<TestAction> = DebugLayer::new(KeyCode::F(12));

        layer.set_enabled(true);
        assert!(layer.is_enabled());

        layer.set_enabled(false);
        assert!(!layer.is_enabled());
    }

    #[test]
    fn test_simple_constructor() {
        let layer: DebugLayer<TestAction> = DebugLayer::simple();
        assert!(!layer.is_enabled());
    }

    #[test]
    fn test_queued_actions_returned_on_disable() {
        let mut layer: DebugLayer<TestAction> = DebugLayer::new(KeyCode::F(12));

        layer.toggle(); // Enable
        layer.queue_action(TestAction::Foo);
        layer.queue_action(TestAction::Bar);

        let effect = layer.toggle(); // Disable

        match effect {
            Some(DebugSideEffect::ProcessQueuedActions(actions)) => {
                assert_eq!(actions.len(), 2);
            }
            _ => panic!("Expected ProcessQueuedActions"),
        }
    }

    #[test]
    fn test_split_area() {
        let layer: DebugLayer<TestAction> = DebugLayer::new(KeyCode::F(12));

        // Disabled: full area returned
        let area = Rect::new(0, 0, 80, 24);
        let (app, banner) = layer.split_area(area);
        assert_eq!(app, area);
        assert_eq!(banner, Rect::ZERO);
    }

    #[test]
    fn test_split_area_enabled() {
        let mut layer: DebugLayer<TestAction> = DebugLayer::new(KeyCode::F(12));
        layer.toggle();

        let area = Rect::new(0, 0, 80, 24);
        let (app, banner) = layer.split_area(area);

        assert_eq!(app.height, 23);
        assert_eq!(banner.height, 1);
        assert_eq!(banner.y, 23);
    }

    #[test]
    fn test_split_area_enabled_top() {
        let mut layer: DebugLayer<TestAction> = DebugLayer::new(KeyCode::F(12));
        layer.toggle();
        layer.set_banner_position(BannerPosition::Top);

        let area = Rect::new(0, 0, 80, 24);
        let (app, banner) = layer.split_area(area);

        assert_eq!(banner.y, 0);
        assert_eq!(banner.height, 1);
        assert_eq!(app.y, 1);
        assert_eq!(app.height, 23);
    }

    #[test]
    fn test_inactive_layer() {
        let layer: DebugLayer<TestAction> = DebugLayer::new(KeyCode::F(12)).active(false);

        assert!(!layer.is_active());
        assert!(!layer.is_enabled());
    }

    #[test]
    fn test_action_log() {
        let mut layer: DebugLayer<TestAction> = DebugLayer::new(KeyCode::F(12));

        layer.log_action(&TestAction::Foo);
        layer.log_action(&TestAction::Bar);

        assert_eq!(layer.action_log().entries().count(), 2);
    }

    #[test]
    fn test_action_log_filter_configuration() {
        let mut layer: DebugLayer<TestAction> = DebugLayer::new(KeyCode::F(12))
            .with_action_log_filter(ActionLoggerConfig::new(Some("name:Foo"), None));

        layer.log_action(&TestAction::Foo);
        layer.log_action(&TestAction::Bar);

        let names: Vec<_> = layer
            .action_log()
            .entries()
            .map(|entry| entry.name)
            .collect();
        assert_eq!(names, vec!["Foo"]);
    }

    #[test]
    fn test_action_log_search_input_does_not_trigger_global_shortcuts() {
        use crossterm::event::{KeyEventKind, KeyEventState};
        use tui_dispatch_core::EventKind;

        let mut layer: DebugLayer<TestAction> = DebugLayer::new(KeyCode::F(12));
        layer.toggle();
        layer.show_action_log();

        if let Some(DebugOverlay::ActionLog(log)) = layer.freeze.overlay.as_mut() {
            log.search_input_active = true;
        } else {
            panic!("expected action log overlay");
        }

        let keys = [
            KeyEvent {
                code: KeyCode::Char('A'),
                modifiers: KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            },
            KeyEvent {
                code: KeyCode::Char('s'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            },
            KeyEvent {
                code: KeyCode::Char('b'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            },
            KeyEvent {
                code: KeyCode::Char('i'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            },
        ];
        for key in keys {
            let outcome = layer.handle_event(&EventKind::Key(key));
            assert!(outcome.consumed);
        }

        match layer.freeze.overlay.as_ref() {
            Some(DebugOverlay::ActionLog(log)) => {
                assert!(log.search_input_active);
                assert_eq!(log.search_query, "Asbi");
            }
            _ => panic!("action log overlay should remain open"),
        }
    }
}
