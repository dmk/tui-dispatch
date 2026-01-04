//! High-level debug layer for TUI applications
//!
//! Provides a self-contained debug overlay with automatic pause/resume of
//! tasks and subscriptions.

use std::io::Write;

use base64::prelude::*;
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Clear, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;

use super::action_logger::{ActionLog, ActionLogConfig};
use super::actions::{DebugAction, DebugSideEffect};
use super::cell::inspect_cell;
use super::config::DebugStyle;
use super::state::DebugState;
use super::table::{ActionLogOverlay, DebugOverlay, DebugTableBuilder, DebugTableOverlay};
use super::widgets::{
    dim_buffer, paint_snapshot, ActionLogWidget, BannerItem, CellPreviewWidget, DebugBanner,
    DebugTableWidget,
};
use super::DebugFreeze;
#[cfg(feature = "subscriptions")]
use crate::subscriptions::SubPauseHandle;
#[cfg(feature = "tasks")]
use crate::tasks::TaskPauseHandle;
use crate::Action;

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
/// use crossterm::event::KeyCode;
/// use tui_dispatch::debug::DebugLayer;
///
/// // Minimal setup - just the toggle key
/// let mut debug = DebugLayer::new(KeyCode::F(12))
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
    /// Scroll offset for state/inspect table overlays
    table_scroll_offset: usize,
    /// Cached page size for table overlay scrolling
    table_page_size: usize,
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
            .field("banner_position", &self.banner_position)
            .field("table_scroll_offset", &self.table_scroll_offset)
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
            table_scroll_offset: 0,
            table_page_size: 1,
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
    pub fn with_task_manager(mut self, tasks: &crate::tasks::TaskManager<A>) -> Self {
        self.task_handle = Some(tasks.pause_handle());
        self
    }

    /// Connect subscriptions for automatic pause/resume.
    ///
    /// When debug mode is enabled, subscriptions will be paused.
    #[cfg(feature = "subscriptions")]
    pub fn with_subscriptions(mut self, subs: &crate::subscriptions::Subscriptions<A>) -> Self {
        self.sub_handle = Some(subs.pause_handle());
        self
    }

    /// Set the action log capacity.
    pub fn with_action_log_capacity(mut self, capacity: usize) -> Self {
        self.action_log = ActionLog::new(ActionLogConfig::with_capacity(capacity));
        self
    }

    /// Set custom style.
    pub fn with_style(mut self, style: DebugStyle) -> Self {
        self.style = style;
        self
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
    pub fn log_action<T: crate::ActionParams>(&mut self, action: &T) {
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
    pub fn intercepts(&mut self, event: &crate::EventKind) -> bool {
        self.intercepts_with_effects(event).is_some()
    }

    /// Handle a debug event with a single call and return a summary outcome.
    pub fn handle_event(&mut self, event: &crate::EventKind) -> DebugOutcome<A> {
        self.handle_event_internal::<()>(event, None)
    }

    /// Handle a debug event with access to state (for the state overlay).
    pub fn handle_event_with_state<S: DebugState>(
        &mut self,
        event: &crate::EventKind,
        state: &S,
    ) -> DebugOutcome<A> {
        self.handle_event_internal(event, Some(state))
    }

    /// Check if debug layer intercepts an event, returning any side effects.
    ///
    /// Returns `None` if the event was not consumed, `Some(effects)` if it was.
    pub fn intercepts_with_effects(
        &mut self,
        event: &crate::EventKind,
    ) -> Option<Vec<DebugSideEffect<A>>> {
        self.intercepts_with_effects_internal::<()>(event, None)
    }

    /// Check if debug layer intercepts an event with access to app state.
    ///
    /// Use this to populate the state overlay when `S` is pressed.
    pub fn intercepts_with_effects_and_state<S: DebugState>(
        &mut self,
        event: &crate::EventKind,
        state: &S,
    ) -> Option<Vec<DebugSideEffect<A>>> {
        self.intercepts_with_effects_internal(event, Some(state))
    }

    /// Check if debug layer intercepts an event with access to app state.
    pub fn intercepts_with_state<S: DebugState>(
        &mut self,
        event: &crate::EventKind,
        state: &S,
    ) -> bool {
        self.intercepts_with_effects_internal(event, Some(state))
            .is_some()
    }

    fn handle_event_internal<S: DebugState>(
        &mut self,
        event: &crate::EventKind,
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

    fn intercepts_with_effects_internal<S: DebugState>(
        &mut self,
        event: &crate::EventKind,
        state: Option<&S>,
    ) -> Option<Vec<DebugSideEffect<A>>> {
        if !self.active {
            return None;
        }

        use crate::EventKind;

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
            EventKind::Scroll { delta, .. } => {
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
                    Some(DebugOverlay::State(table)) | Some(DebugOverlay::Inspect(table)) => {
                        if *delta > 0 {
                            self.scroll_table_up();
                        } else {
                            self.scroll_table_down(table.rows.len());
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
        if !matches!(self.freeze.overlay, Some(DebugOverlay::State(_))) {
            self.table_scroll_offset = 0;
        }
        self.state_snapshot = Some(table.clone());
        self.freeze.set_overlay(DebugOverlay::State(table));
    }

    fn update_table_scroll(&mut self, table: &DebugTableOverlay, table_area: Rect) {
        let visible_rows = table_area.height.saturating_sub(1) as usize;
        self.table_page_size = visible_rows.max(1);
        let max_offset = table.rows.len().saturating_sub(visible_rows);
        self.table_scroll_offset = self.table_scroll_offset.min(max_offset);
    }

    fn build_scrollbar(&self, orientation: ScrollbarOrientation) -> Scrollbar<'static> {
        let mut scrollbar = Scrollbar::new(orientation)
            .thumb_style(self.style.scrollbar.thumb)
            .track_style(self.style.scrollbar.track)
            .begin_style(self.style.scrollbar.begin)
            .end_style(self.style.scrollbar.end);

        if let Some(symbol) = self.style.scrollbar.thumb_symbol {
            scrollbar = scrollbar.thumb_symbol(symbol);
        }
        if let Some(symbol) = self.style.scrollbar.track_symbol {
            scrollbar = scrollbar.track_symbol(Some(symbol));
        }
        if let Some(symbol) = self.style.scrollbar.begin_symbol {
            scrollbar = scrollbar.begin_symbol(Some(symbol));
        }
        if let Some(symbol) = self.style.scrollbar.end_symbol {
            scrollbar = scrollbar.end_symbol(Some(symbol));
        }

        scrollbar
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

    fn handle_key_event<S: DebugState>(
        &mut self,
        key: KeyEvent,
        state: Option<&S>,
    ) -> Option<Vec<DebugSideEffect<A>>> {
        // Toggle key always works (even when disabled)
        if key.code == self.toggle_key && key.modifiers.is_empty() {
            let effect = self.toggle();
            return Some(effect.into_iter().collect());
        }

        // Esc also toggles off when enabled
        if self.freeze.enabled && key.code == KeyCode::Esc {
            let effect = self.toggle();
            return Some(effect.into_iter().collect());
        }

        // Other commands only work when enabled
        if !self.freeze.enabled {
            return None;
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
            _ => {}
        }

        // Handle internal debug commands (hardcoded keys)
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

        // Handle overlay-specific navigation
        match &self.freeze.overlay {
            Some(DebugOverlay::ActionLog(_)) => {
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
            Some(DebugOverlay::State(table)) | Some(DebugOverlay::Inspect(table)) => {
                if self.handle_table_scroll_key(key.code, table.rows.len()) {
                    return Some(vec![]);
                }
            }
            Some(DebugOverlay::ActionDetail(_)) => {
                // Back to action log on Esc, Backspace, or Enter
                if matches!(key.code, KeyCode::Esc | KeyCode::Backspace | KeyCode::Enter) {
                    self.handle_action(DebugAction::ActionLogBackToList);
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
            self.table_scroll_offset = 0;
            self.table_page_size = 1;

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
            self.table_scroll_offset = 0;
            self.table_page_size = 1;
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
                DebugOverlay::Inspect(table) | DebugOverlay::State(table) => {
                    self.render_table_modal(frame, app_area, table);
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

        let keys = &self.style.key_styles;
        let toggle_key_str = format_key(self.toggle_key);
        let mut banner = DebugBanner::new()
            .title("DEBUG")
            .title_style(self.style.title_style)
            .label_style(self.style.label_style)
            .background(self.style.banner_bg);

        // Add standard debug commands with hardcoded keys
        banner = banner.item(BannerItem::new(&toggle_key_str, "resume", keys.toggle));
        banner = banner.item(BannerItem::new("a", "actions", keys.actions));
        banner = banner.item(BannerItem::new("s", "state", keys.state));
        banner = banner.item(BannerItem::new(
            "b",
            self.banner_position.label(),
            keys.actions,
        ));
        banner = banner.item(BannerItem::new("y", "copy", keys.copy));

        if self.freeze.mouse_capture_enabled {
            banner = banner.item(BannerItem::new("click", "inspect", keys.mouse));
        } else {
            banner = banner.item(BannerItem::new("i", "mouse", keys.mouse));
        }

        // Add message if present
        if let Some(ref msg) = self.freeze.message {
            banner = banner.item(BannerItem::new("", msg, self.style.value_style));
        }

        frame.render_widget(banner, banner_area);
    }

    fn render_table_modal(&mut self, frame: &mut Frame, app_area: Rect, table: &DebugTableOverlay) {
        let modal_width = (app_area.width * 80 / 100)
            .clamp(30, 120)
            .min(app_area.width);
        let modal_height = (app_area.height * 60 / 100)
            .clamp(8, 40)
            .min(app_area.height);

        let modal_x = app_area.x + (app_area.width.saturating_sub(modal_width)) / 2;
        let modal_y = app_area.y + (app_area.height.saturating_sub(modal_height)) / 2;

        let modal_area = Rect::new(modal_x, modal_y, modal_width, modal_height);

        frame.render_widget(Clear, modal_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", table.title))
            .style(self.style.banner_bg);

        let inner = block.inner(modal_area);
        frame.render_widget(block, modal_area);

        let mut table_area = if let Some(ref preview) = table.cell_preview {
            if inner.height > 3 {
                let preview_height = 2u16;
                let preview_area = Rect {
                    x: inner.x,
                    y: inner.y,
                    width: inner.width,
                    height: 1,
                };
                let table_area = Rect {
                    x: inner.x,
                    y: inner.y.saturating_add(preview_height),
                    width: inner.width,
                    height: inner.height.saturating_sub(preview_height),
                };

                let preview_widget = CellPreviewWidget::new(preview)
                    .label_style(Style::default().fg(DebugStyle::text_secondary()))
                    .value_style(Style::default().fg(DebugStyle::text_primary()));
                frame.render_widget(preview_widget, preview_area);
                table_area
            } else {
                inner
            }
        } else {
            inner
        };

        let visible_rows = table_area.height.saturating_sub(1) as usize;
        let show_scrollbar =
            visible_rows > 0 && table.rows.len() > visible_rows && table_area.width > 11;
        let scrollbar_area = if show_scrollbar {
            let scrollbar_area = Rect {
                x: table_area.x + table_area.width.saturating_sub(1),
                width: 1,
                ..table_area
            };
            table_area.width = table_area.width.saturating_sub(1);
            Some(Rect {
                y: scrollbar_area.y.saturating_add(1),
                height: scrollbar_area.height.saturating_sub(1),
                ..scrollbar_area
            })
        } else {
            None
        };

        self.update_table_scroll(table, table_area);
        let table_widget = DebugTableWidget::new(table).scroll_offset(self.table_scroll_offset);
        frame.render_widget(table_widget, table_area);

        if let Some(scrollbar_area) = scrollbar_area {
            let content_length = self.table_max_offset(table.rows.len()).saturating_add(1);
            let mut scrollbar_state = ScrollbarState::new(content_length)
                .position(self.table_scroll_offset)
                .viewport_content_length(self.table_page_size_value());
            let scrollbar = self.build_scrollbar(ScrollbarOrientation::VerticalRight);
            frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
        }
    }

    fn render_action_log_modal(&self, frame: &mut Frame, app_area: Rect, log: &ActionLogOverlay) {
        let modal_width = (app_area.width * 90 / 100)
            .clamp(40, 140)
            .min(app_area.width);
        let modal_height = (app_area.height * 70 / 100)
            .clamp(10, 50)
            .min(app_area.height);

        let modal_x = app_area.x + (app_area.width.saturating_sub(modal_width)) / 2;
        let modal_y = app_area.y + (app_area.height.saturating_sub(modal_height)) / 2;

        let modal_area = Rect::new(modal_x, modal_y, modal_width, modal_height);

        frame.render_widget(Clear, modal_area);

        let entry_count = log.entries.len();
        let title = if entry_count > 0 {
            format!(" {} ({} entries) ", log.title, entry_count)
        } else {
            format!(" {} (empty) ", log.title)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .style(self.style.banner_bg);

        let mut log_area = block.inner(modal_area);
        frame.render_widget(block, modal_area);

        let visible_rows = log_area.height.saturating_sub(1) as usize;
        let show_scrollbar =
            visible_rows > 0 && log.entries.len() > visible_rows && log_area.width > 31;
        let scrollbar_area = if show_scrollbar {
            let scrollbar_area = Rect {
                x: log_area.x + log_area.width.saturating_sub(1),
                width: 1,
                ..log_area
            };
            log_area.width = log_area.width.saturating_sub(1);
            Some(Rect {
                y: scrollbar_area.y.saturating_add(1),
                height: scrollbar_area.height.saturating_sub(1),
                ..scrollbar_area
            })
        } else {
            None
        };

        let widget = ActionLogWidget::new(log);
        frame.render_widget(widget, log_area);

        if let Some(scrollbar_area) = scrollbar_area {
            let visible_rows = log_area.height.saturating_sub(1) as usize;
            let scroll_offset = log.scroll_offset_for(visible_rows);
            let content_length = log
                .entries
                .len()
                .saturating_sub(visible_rows)
                .saturating_add(1);
            let mut scrollbar_state = ScrollbarState::new(content_length)
                .position(scroll_offset)
                .viewport_content_length(visible_rows);
            let scrollbar = self.build_scrollbar(ScrollbarOrientation::VerticalRight);
            frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
        }
    }

    fn render_action_detail_modal(
        &self,
        frame: &mut Frame,
        app_area: Rect,
        detail: &super::table::ActionDetailOverlay,
    ) {
        let modal_width = (app_area.width * 80 / 100)
            .clamp(40, 120)
            .min(app_area.width);
        let modal_height = (app_area.height * 50 / 100)
            .clamp(8, 30)
            .min(app_area.height);

        let modal_x = app_area.x + (app_area.width.saturating_sub(modal_width)) / 2;
        let modal_y = app_area.y + (app_area.height.saturating_sub(modal_height)) / 2;

        let modal_area = Rect::new(modal_x, modal_y, modal_width, modal_height);

        frame.render_widget(Clear, modal_area);

        let title = format!(" Action #{} - {} ", detail.sequence, detail.name);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .style(self.style.banner_bg);

        let inner = block.inner(modal_area);
        frame.render_widget(block, modal_area);

        // Build detail content
        use ratatui::text::{Line, Span};
        use ratatui::widgets::Paragraph;

        let label_style = Style::default().fg(DebugStyle::text_secondary());
        let value_style = Style::default().fg(DebugStyle::text_primary());

        let mut lines = vec![
            // Name
            Line::from(vec![
                Span::styled("Name: ", label_style),
                Span::styled(&detail.name, value_style),
            ]),
            // Sequence
            Line::from(vec![
                Span::styled("Sequence: ", label_style),
                Span::styled(detail.sequence.to_string(), value_style),
            ]),
            // Elapsed
            Line::from(vec![
                Span::styled("Elapsed: ", label_style),
                Span::styled(&detail.elapsed, value_style),
            ]),
            // Empty line before params
            Line::from(""),
            // Parameters header
            Line::from(Span::styled("Parameters:", label_style)),
        ];

        // Parameters content (potentially multi-line)
        if detail.params.is_empty() {
            lines.push(Line::from(Span::styled("  (none)", value_style)));
        } else {
            for param_line in detail.params.lines() {
                lines.push(Line::from(Span::styled(
                    format!("  {}", param_line),
                    value_style,
                )));
            }
        }

        // Footer hint
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Press Enter/Esc/Backspace to go back",
            label_style,
        )));

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
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

    impl crate::Action for TestAction {
        fn name(&self) -> &'static str {
            match self {
                TestAction::Foo => "Foo",
                TestAction::Bar => "Bar",
            }
        }
    }

    impl crate::ActionParams for TestAction {
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
}
