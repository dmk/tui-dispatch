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
use ratatui::widgets::{Block, Borders, Clear};
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
    /// Style configuration
    style: DebugStyle,
    /// Whether the debug layer is active (can be disabled for release builds)
    active: bool,
    /// Action log for display
    action_log: ActionLog,
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
            style: DebugStyle::default(),
            active: true,
            action_log: ActionLog::new(ActionLogConfig::with_capacity(100)),
            #[cfg(feature = "tasks")]
            task_handle: None,
            #[cfg(feature = "subscriptions")]
            sub_handle: None,
        }
    }

    /// Set whether the debug layer is active.
    ///
    /// When inactive (`false`), all methods become no-ops with zero overhead.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
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
        let screen = frame.area();

        // Inactive or not in debug mode: just render normally
        if !self.active || !self.freeze.enabled {
            render_fn(frame, screen);
            return;
        }

        // Debug mode: reserve bottom line for banner
        let (app_area, banner_area) = self.split_for_banner(screen);

        if self.freeze.pending_capture || self.freeze.snapshot.is_none() {
            // Capture mode: render app, then capture
            render_fn(frame, app_area);
            let buffer_clone = frame.buffer_mut().clone();
            self.freeze.capture(&buffer_clone);
        } else if let Some(ref snapshot) = self.freeze.snapshot {
            // Frozen: paint snapshot
            paint_snapshot(frame, snapshot);
        }

        // Render debug overlay
        self.render_debug_overlay(frame, app_area, banner_area);
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

    /// Check if debug layer intercepts an event, returning any side effects.
    ///
    /// Returns `None` if the event was not consumed, `Some(effects)` if it was.
    pub fn intercepts_with_effects(
        &mut self,
        event: &crate::EventKind,
    ) -> Option<Vec<DebugSideEffect<A>>> {
        if !self.active {
            return None;
        }

        use crate::EventKind;

        match event {
            EventKind::Key(key) => self.handle_key_event(*key),
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

                // Handle scrolling in action log overlay
                if let Some(DebugOverlay::ActionLog(_)) = self.freeze.overlay {
                    let action = if *delta > 0 {
                        DebugAction::ActionLogScrollUp
                    } else {
                        DebugAction::ActionLogScrollDown
                    };
                    self.handle_action(action);
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
        self.freeze.set_overlay(DebugOverlay::State(table));
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

    fn handle_key_event(&mut self, key: KeyEvent) -> Option<Vec<DebugSideEffect<A>>> {
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

        // Handle internal debug commands (hardcoded keys)
        let action = match key.code {
            KeyCode::Char('s') | KeyCode::Char('S') => Some(DebugAction::ToggleState),
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
                } else {
                    // Show placeholder - user should call show_state_overlay()
                    let table = DebugTableBuilder::new()
                        .section("State")
                        .entry("hint", "Press 's' after calling show_state_overlay()")
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
        let banner_height = 1;
        let app_area = Rect {
            height: area.height.saturating_sub(banner_height),
            ..area
        };
        let banner_area = Rect {
            y: area.y.saturating_add(app_area.height),
            height: banner_height.min(area.height),
            ..area
        };
        (app_area, banner_area)
    }

    fn render_debug_overlay(&self, frame: &mut Frame, app_area: Rect, banner_area: Rect) {
        // Only dim when there's an overlay open
        if let Some(ref overlay) = self.freeze.overlay {
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

    fn render_table_modal(&self, frame: &mut Frame, app_area: Rect, table: &DebugTableOverlay) {
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

        // Cell preview on top if present
        if let Some(ref preview) = table.cell_preview {
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

                let table_widget = DebugTableWidget::new(table);
                frame.render_widget(table_widget, table_area);
                return;
            }
        }

        let table_widget = DebugTableWidget::new(table);
        frame.render_widget(table_widget, inner);
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

        let inner = block.inner(modal_area);
        frame.render_widget(block, modal_area);

        let widget = ActionLogWidget::new(log);
        frame.render_widget(widget, inner);
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
