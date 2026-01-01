//! High-level debug layer for TUI applications
//!
//! Provides a wrapper that handles debug UI rendering with sensible defaults.

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Clear};
use ratatui::Frame;
use std::marker::PhantomData;

use super::action_logger::ActionLog;
use super::actions::{DebugAction, DebugSideEffect};
use super::cell::inspect_cell;
use super::config::{
    default_debug_keybindings, default_debug_keybindings_with_toggle, DebugConfig, DebugStyle,
    StatusItem,
};
use super::state::DebugState;
use super::table::{ActionLogOverlay, DebugOverlay, DebugTableBuilder, DebugTableOverlay};
use super::widgets::{
    dim_buffer, paint_snapshot, ActionLogWidget, BannerItem, CellPreviewWidget, DebugBanner,
    DebugTableWidget,
};
use super::{DebugFreeze, SimpleDebugContext};
use crate::keybindings::{format_key_for_display, BindingContext};

/// High-level debug layer with sensible defaults
///
/// Wraps `DebugFreeze` and provides automatic rendering with:
/// - 1-line status bar at bottom when debug mode is active
/// - Frame capture/restore with dimming
/// - Modal overlays for state inspection
///
/// # Type Parameters
///
/// - `A`: The application's action type (for queuing actions while frozen)
/// - `C`: The keybinding context type
///
/// # Example
///
/// ```ignore
/// use tui_dispatch::debug::{DebugLayer, DebugConfig};
///
/// // In your app:
/// struct App {
///     debug: DebugLayer<MyAction, MyContext>,
///     // ...
/// }
///
/// // In render loop:
/// app.debug.render(frame, |f, area| {
///     // Render your normal UI here
///     app.render_main(f, area);
/// });
/// ```
pub struct DebugLayer<A, C: BindingContext> {
    /// Internal freeze state
    freeze: DebugFreeze<A>,
    /// Configuration
    config: DebugConfig<C>,
}

impl<A, C: BindingContext> std::fmt::Debug for DebugLayer<A, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DebugLayer")
            .field("enabled", &self.freeze.enabled)
            .field("has_snapshot", &self.freeze.snapshot.is_some())
            .field("queued_actions", &self.freeze.queued_actions.len())
            .finish()
    }
}

impl<A, C: BindingContext> DebugLayer<A, C> {
    /// Create a new debug layer with the given configuration
    pub fn new(config: DebugConfig<C>) -> Self {
        Self {
            freeze: DebugFreeze::new(),
            config,
        }
    }

    /// Check if debug mode is enabled
    pub fn is_enabled(&self) -> bool {
        self.freeze.enabled
    }

    /// Get a reference to the underlying freeze state
    pub fn freeze(&self) -> &DebugFreeze<A> {
        &self.freeze
    }

    /// Get a mutable reference to the underlying freeze state
    pub fn freeze_mut(&mut self) -> &mut DebugFreeze<A> {
        &mut self.freeze
    }

    /// Get the configuration
    pub fn config(&self) -> &DebugConfig<C> {
        &self.config
    }

    /// Get mutable configuration
    pub fn config_mut(&mut self) -> &mut DebugConfig<C> {
        &mut self.config
    }

    /// Render with automatic debug handling (primary API)
    ///
    /// When debug mode is disabled, simply calls `render_fn` with the full frame area.
    ///
    /// When debug mode is enabled:
    /// - Reserves 1 line at bottom for the debug banner
    /// - Captures the frame on first render or when requested
    /// - Paints the frozen snapshot with dimming
    /// - Renders debug overlay (banner + modal if open)
    ///
    /// # Example
    ///
    /// ```ignore
    /// terminal.draw(|frame| {
    ///     app.debug.render(frame, |f, area| {
    ///         render_main_ui(f, area, &app.state);
    ///     });
    /// })?;
    /// ```
    pub fn render<F>(&mut self, frame: &mut Frame, render_fn: F)
    where
        F: FnOnce(&mut Frame, Rect),
    {
        let screen = frame.area();

        if !self.freeze.enabled {
            // Normal mode: render full screen
            render_fn(frame, screen);
            return;
        }

        // Debug mode: reserve bottom line for banner
        let (app_area, banner_area) = self.split_for_banner(screen);

        if self.freeze.pending_capture || self.freeze.snapshot.is_none() {
            // Capture mode: render app, then capture
            render_fn(frame, app_area);
            // Clone the buffer for capture (buffer_mut is available, buffer is not public)
            let buffer_clone = frame.buffer_mut().clone();
            self.freeze.capture(&buffer_clone);
        } else if let Some(ref snapshot) = self.freeze.snapshot {
            // Frozen: paint snapshot
            paint_snapshot(frame, snapshot);
        }

        // Render debug overlay
        self.render_debug_overlay(frame, app_area, banner_area);
    }

    /// Split area for manual layout control (escape hatch)
    ///
    /// Returns (app_area, debug_banner_area). When debug mode is disabled,
    /// returns the full area and an empty rect.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let (app_area, banner_area) = debug.split_area(frame.area());
    ///
    /// // Custom layout
    /// let chunks = Layout::vertical([...]).split(app_area);
    /// app.render_main(frame, chunks[0]);
    /// app.render_status(frame, chunks[1]);
    ///
    /// // Let debug layer render its UI
    /// if debug.is_enabled() {
    ///     debug.render_overlay(frame, app_area);
    ///     debug.render_banner(frame, banner_area);
    /// }
    /// ```
    pub fn split_area(&self, area: Rect) -> (Rect, Rect) {
        if !self.freeze.enabled {
            return (area, Rect::ZERO);
        }
        self.split_for_banner(area)
    }

    /// Render just the debug overlay (modal + dimming)
    ///
    /// Use this with `split_area` for manual layout control.
    pub fn render_overlay(&self, frame: &mut Frame, app_area: Rect) {
        if !self.freeze.enabled {
            return;
        }

        // Dim the background
        dim_buffer(frame.buffer_mut(), self.config.style.dim_factor);

        // Render overlay if present
        if let Some(ref overlay) = self.freeze.overlay {
            match overlay {
                DebugOverlay::Inspect(table) | DebugOverlay::State(table) => {
                    self.render_table_modal(frame, app_area, table);
                }
                DebugOverlay::ActionLog(log) => {
                    self.render_action_log_modal(frame, app_area, log);
                }
            }
        }
    }

    /// Render just the debug banner
    ///
    /// Use this with `split_area` for manual layout control.
    pub fn render_banner(&self, frame: &mut Frame, banner_area: Rect) {
        if !self.freeze.enabled || banner_area.height == 0 {
            return;
        }

        let style = &self.config.style;
        let mut banner = DebugBanner::new()
            .title("DEBUG")
            .title_style(style.title_style)
            .label_style(style.label_style)
            .background(style.banner_bg);

        // Add standard debug commands with distinct colors
        let keys = &style.key_styles;
        self.add_banner_item(&mut banner, DebugAction::CMD_TOGGLE, "resume", keys.toggle);
        self.add_banner_item(
            &mut banner,
            DebugAction::CMD_TOGGLE_ACTION_LOG,
            "actions",
            keys.actions,
        );
        self.add_banner_item(
            &mut banner,
            DebugAction::CMD_TOGGLE_STATE,
            "state",
            keys.state,
        );
        self.add_banner_item(&mut banner, DebugAction::CMD_COPY_FRAME, "copy", keys.copy);

        if self.freeze.mouse_capture_enabled {
            banner = banner.item(BannerItem::new("click", "inspect", keys.mouse));
        } else {
            self.add_banner_item(
                &mut banner,
                DebugAction::CMD_TOGGLE_MOUSE,
                "mouse",
                keys.mouse,
            );
        }

        // Add message if present
        if let Some(ref msg) = self.freeze.message {
            banner = banner.item(BannerItem::new("", msg, style.value_style));
        }

        // Note: Custom status items from config.status_items() require owned strings
        // but BannerItem uses borrowed &str. For now, use render_banner_with_status()
        // if you need custom status items.

        frame.render_widget(banner, banner_area);
    }

    /// Render the debug banner with custom status items
    ///
    /// Use this if you need to add dynamic status items to the banner.
    /// The status_items slice must outlive this call.
    pub fn render_banner_with_status(
        &self,
        frame: &mut Frame,
        banner_area: Rect,
        status_items: &[(&str, &str)],
    ) {
        if !self.freeze.enabled || banner_area.height == 0 {
            return;
        }

        let style = &self.config.style;
        let mut banner = DebugBanner::new()
            .title("DEBUG")
            .title_style(style.title_style)
            .label_style(style.label_style)
            .background(style.banner_bg);

        // Add standard debug commands with distinct colors
        let keys = &style.key_styles;
        self.add_banner_item(&mut banner, DebugAction::CMD_TOGGLE, "resume", keys.toggle);
        self.add_banner_item(
            &mut banner,
            DebugAction::CMD_TOGGLE_ACTION_LOG,
            "actions",
            keys.actions,
        );
        self.add_banner_item(
            &mut banner,
            DebugAction::CMD_TOGGLE_STATE,
            "state",
            keys.state,
        );
        self.add_banner_item(&mut banner, DebugAction::CMD_COPY_FRAME, "copy", keys.copy);

        if self.freeze.mouse_capture_enabled {
            banner = banner.item(BannerItem::new("click", "inspect", keys.mouse));
        } else {
            self.add_banner_item(
                &mut banner,
                DebugAction::CMD_TOGGLE_MOUSE,
                "mouse",
                keys.mouse,
            );
        }

        // Add message if present
        if let Some(ref msg) = self.freeze.message {
            banner = banner.item(BannerItem::new("", msg, style.value_style));
        }

        // Add custom status items
        for (label, value) in status_items {
            banner = banner.item(BannerItem::new(label, value, style.value_style));
        }

        frame.render_widget(banner, banner_area);
    }

    /// Handle a debug action
    ///
    /// Returns a side effect if the app needs to take action (clipboard, mouse capture, etc).
    pub fn handle_action(&mut self, action: DebugAction) -> Option<DebugSideEffect<A>> {
        match action {
            DebugAction::Toggle => {
                if self.freeze.enabled {
                    let queued = self.freeze.take_queued();
                    self.freeze.disable();
                    if queued.is_empty() {
                        None
                    } else {
                        Some(DebugSideEffect::ProcessQueuedActions(queued))
                    }
                } else {
                    self.freeze.enable();
                    None
                }
            }
            DebugAction::CopyFrame => {
                let text = self.freeze.snapshot_text.clone();
                self.freeze.set_message("Copied to clipboard");
                Some(DebugSideEffect::CopyToClipboard(text))
            }
            DebugAction::ToggleState => {
                // Toggle between no overlay and state overlay
                if matches!(self.freeze.overlay, Some(DebugOverlay::State(_))) {
                    self.freeze.clear_overlay();
                } else {
                    // App should call show_state_overlay() with their state
                    // For now, just show a placeholder
                    let table = DebugTableBuilder::new()
                        .section("State")
                        .entry("hint", "Call show_state_overlay() with your state")
                        .finish("Application State");
                    self.freeze.set_overlay(DebugOverlay::State(table));
                }
                None
            }
            DebugAction::ToggleActionLog => {
                // Toggle between no overlay and action log overlay
                if matches!(self.freeze.overlay, Some(DebugOverlay::ActionLog(_))) {
                    self.freeze.clear_overlay();
                } else {
                    // App should call show_action_log() with their log
                    // For now, just show a placeholder
                    let overlay = ActionLogOverlay {
                        title: "Action Log".to_string(),
                        entries: vec![],
                        selected: 0,
                        scroll_offset: 0,
                    };
                    self.freeze
                        .set_message("Call show_action_log() with your ActionLog");
                    self.freeze.set_overlay(DebugOverlay::ActionLog(overlay));
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
            DebugAction::ToggleMouseCapture => {
                self.freeze.toggle_mouse_capture();
                if self.freeze.mouse_capture_enabled {
                    Some(DebugSideEffect::EnableMouseCapture)
                } else {
                    Some(DebugSideEffect::DisableMouseCapture)
                }
            }
            DebugAction::InspectCell { column, row } => {
                if let Some(ref snapshot) = self.freeze.snapshot {
                    let overlay = self.build_inspect_overlay(column, row, snapshot);
                    self.freeze.set_overlay(DebugOverlay::Inspect(overlay));
                }
                self.freeze.mouse_capture_enabled = false;
                Some(DebugSideEffect::DisableMouseCapture)
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

    /// Handle a mouse event in debug mode
    ///
    /// Returns `true` if the event was consumed by debug handling.
    pub fn handle_mouse(&mut self, mouse: MouseEvent) -> Option<DebugSideEffect<A>> {
        if !self.freeze.enabled {
            return None;
        }

        // Ignore scroll events
        if matches!(
            mouse.kind,
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
        ) {
            return None;
        }

        // Handle click in capture mode
        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
            && self.freeze.mouse_capture_enabled
        {
            return self.handle_action(DebugAction::InspectCell {
                column: mouse.column,
                row: mouse.row,
            });
        }

        None
    }

    /// Show state overlay using a DebugState implementor
    pub fn show_state_overlay<S: DebugState>(&mut self, state: &S) {
        let table = state.build_debug_table("Application State");
        self.freeze.set_overlay(DebugOverlay::State(table));
    }

    /// Show state overlay with custom title
    pub fn show_state_overlay_with_title<S: DebugState>(&mut self, state: &S, title: &str) {
        let table = state.build_debug_table(title);
        self.freeze.set_overlay(DebugOverlay::State(table));
    }

    /// Show action log overlay
    ///
    /// Displays recent actions from the provided ActionLog.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // If using ActionLoggerMiddleware with storage
    /// if let Some(log) = middleware.log() {
    ///     debug_layer.show_action_log(log);
    /// }
    /// ```
    pub fn show_action_log(&mut self, log: &ActionLog) {
        let overlay = ActionLogOverlay::from_log(log, "Action Log");
        self.freeze.set_overlay(DebugOverlay::ActionLog(overlay));
    }

    /// Show action log overlay with custom title
    pub fn show_action_log_with_title(&mut self, log: &ActionLog, title: &str) {
        let overlay = ActionLogOverlay::from_log(log, title);
        self.freeze.set_overlay(DebugOverlay::ActionLog(overlay));
    }

    /// Queue an action to be processed when debug mode is disabled
    pub fn queue_action(&mut self, action: A) {
        self.freeze.queue(action);
    }

    // --- Private helpers ---

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
        // Dim the background
        dim_buffer(frame.buffer_mut(), self.config.style.dim_factor);

        // Render overlay if present
        if let Some(ref overlay) = self.freeze.overlay {
            match overlay {
                DebugOverlay::Inspect(table) | DebugOverlay::State(table) => {
                    self.render_table_modal(frame, app_area, table);
                }
                DebugOverlay::ActionLog(log) => {
                    self.render_action_log_modal(frame, app_area, log);
                }
            }
        }

        // Render banner
        self.render_banner(frame, banner_area);
    }

    fn render_table_modal(&self, frame: &mut Frame, app_area: Rect, table: &DebugTableOverlay) {
        // Calculate modal size (80% width, 60% height, with min/max)
        let modal_width = (app_area.width * 80 / 100)
            .clamp(30, 120)
            .min(app_area.width);
        let modal_height = (app_area.height * 60 / 100)
            .clamp(8, 40)
            .min(app_area.height);

        // Center the modal
        let modal_x = app_area.x + (app_area.width.saturating_sub(modal_width)) / 2;
        let modal_y = app_area.y + (app_area.height.saturating_sub(modal_height)) / 2;

        let modal_area = Rect::new(modal_x, modal_y, modal_width, modal_height);

        // Clear and render modal background
        frame.render_widget(Clear, modal_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", table.title))
            .style(self.config.style.banner_bg);

        let inner = block.inner(modal_area);
        frame.render_widget(block, modal_area);

        // Cell preview on top (if present), table below
        if let Some(ref preview) = table.cell_preview {
            if inner.height > 3 {
                let preview_height = 2u16; // 1 line + 1 spacing
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

                // Render cell preview with neon colors
                let preview_widget = CellPreviewWidget::new(preview)
                    .label_style(Style::default().fg(DebugStyle::text_secondary()))
                    .value_style(Style::default().fg(DebugStyle::text_primary()));
                frame.render_widget(preview_widget, preview_area);

                // Render table below
                let table_widget = DebugTableWidget::new(table);
                frame.render_widget(table_widget, table_area);
                return;
            }
        }

        // No cell preview or not enough space - just render table
        let table_widget = DebugTableWidget::new(table);
        frame.render_widget(table_widget, inner);
    }

    fn render_action_log_modal(&self, frame: &mut Frame, app_area: Rect, log: &ActionLogOverlay) {
        // Calculate modal size (larger for action log - 90% width, 70% height)
        let modal_width = (app_area.width * 90 / 100)
            .clamp(40, 140)
            .min(app_area.width);
        let modal_height = (app_area.height * 70 / 100)
            .clamp(10, 50)
            .min(app_area.height);

        // Center the modal
        let modal_x = app_area.x + (app_area.width.saturating_sub(modal_width)) / 2;
        let modal_y = app_area.y + (app_area.height.saturating_sub(modal_height)) / 2;

        let modal_area = Rect::new(modal_x, modal_y, modal_width, modal_height);

        // Clear and render modal background
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
            .style(self.config.style.banner_bg);

        let inner = block.inner(modal_area);
        frame.render_widget(block, modal_area);

        // Render action log widget
        let widget = ActionLogWidget::new(log);
        frame.render_widget(widget, inner);
    }

    fn add_banner_item(
        &self,
        banner: &mut DebugBanner<'_>,
        command: &str,
        label: &'static str,
        style: Style,
    ) {
        if let Some(key) = self
            .config
            .keybindings
            .get_first_keybinding(command, self.config.debug_context)
        {
            let formatted = format_key_for_display(&key);
            // We need to leak the string for the lifetime - this is fine for debug UI
            let key_str: &'static str = Box::leak(formatted.into_boxed_str());
            *banner = std::mem::take(banner).item(BannerItem::new(key_str, label, style));
        }
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

/// Builder for DebugLayer with ergonomic configuration
pub struct DebugLayerBuilder<A, C: BindingContext> {
    config: DebugConfig<C>,
    _marker: PhantomData<A>,
}

impl<A, C: BindingContext> DebugLayerBuilder<A, C> {
    /// Create a new builder
    pub fn new(config: DebugConfig<C>) -> Self {
        Self {
            config,
            _marker: PhantomData,
        }
    }

    /// Set status provider
    pub fn with_status_provider<F>(mut self, provider: F) -> Self
    where
        F: Fn() -> Vec<StatusItem> + Send + Sync + 'static,
    {
        self.config = self.config.with_status_provider(provider);
        self
    }

    /// Build the DebugLayer
    pub fn build(self) -> DebugLayer<A, C> {
        DebugLayer::new(self.config)
    }
}

// ============================================================================
// Simple API - Zero-configuration debug layer
// ============================================================================

impl<A> DebugLayer<A, SimpleDebugContext> {
    /// Create a debug layer with sensible defaults - no configuration needed.
    ///
    /// This is the recommended way to add debug capabilities to your app.
    ///
    /// # Default Keybindings (when debug mode is active)
    ///
    /// - `F12` / `Esc`: Toggle debug mode
    /// - `S`: Show/hide state overlay
    /// - `Y`: Copy frozen frame to clipboard
    /// - `I`: Toggle mouse capture for cell inspection
    ///
    /// # Example
    ///
    /// ```ignore
    /// use tui_dispatch::debug::DebugLayer;
    ///
    /// // One line setup:
    /// let mut debug = DebugLayer::<MyAction>::simple();
    ///
    /// // In render loop:
    /// terminal.draw(|frame| {
    ///     debug.render(frame, |f, area| {
    ///         render_my_app(f, area, &state);
    ///     });
    /// })?;
    ///
    /// // Handle F12 to toggle (before normal event handling):
    /// if matches!(event, KeyEvent { code: KeyCode::F(12), .. }) {
    ///     debug.handle_action(DebugAction::Toggle);
    /// }
    /// ```
    pub fn simple() -> Self {
        let keybindings = default_debug_keybindings();
        let config = DebugConfig::new(keybindings, SimpleDebugContext::Debug);
        Self::new(config)
    }

    /// Create a debug layer with custom toggle key(s).
    ///
    /// Same as [`simple()`](Self::simple) but uses the provided key(s)
    /// for toggling debug mode instead of `F12`/`Esc`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use tui_dispatch::debug::DebugLayer;
    ///
    /// // Use F11 instead of F12:
    /// let debug = DebugLayer::<MyAction>::simple_with_toggle_key(&["F11"]);
    ///
    /// // Multiple toggle keys:
    /// let debug = DebugLayer::<MyAction>::simple_with_toggle_key(&["F11", "Ctrl+D"]);
    /// ```
    pub fn simple_with_toggle_key(keys: &[&str]) -> Self {
        let keybindings = default_debug_keybindings_with_toggle(keys);
        let config = DebugConfig::new(keybindings, SimpleDebugContext::Debug);
        Self::new(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keybindings::Keybindings;

    // Minimal test context
    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    enum TestContext {
        Debug,
    }

    impl BindingContext for TestContext {
        fn name(&self) -> &'static str {
            "debug"
        }
        fn from_name(name: &str) -> Option<Self> {
            (name == "debug").then_some(TestContext::Debug)
        }
        fn all() -> &'static [Self] {
            &[TestContext::Debug]
        }
    }

    #[derive(Debug, Clone)]
    enum TestAction {
        Foo,
        Bar,
    }

    fn make_layer() -> DebugLayer<TestAction, TestContext> {
        let config = DebugConfig::new(Keybindings::new(), TestContext::Debug);
        DebugLayer::new(config)
    }

    #[test]
    fn test_debug_layer_creation() {
        let layer = make_layer();
        assert!(!layer.is_enabled());
        assert!(layer.freeze().snapshot.is_none());
    }

    #[test]
    fn test_toggle() {
        let mut layer = make_layer();

        // Enable
        let effect = layer.handle_action(DebugAction::Toggle);
        assert!(effect.is_none());
        assert!(layer.is_enabled());

        // Disable
        let effect = layer.handle_action(DebugAction::Toggle);
        assert!(effect.is_none()); // No queued actions
        assert!(!layer.is_enabled());
    }

    #[test]
    fn test_queued_actions_returned_on_disable() {
        let mut layer = make_layer();

        layer.handle_action(DebugAction::Toggle); // Enable
        layer.queue_action(TestAction::Foo);
        layer.queue_action(TestAction::Bar);

        let effect = layer.handle_action(DebugAction::Toggle); // Disable

        match effect {
            Some(DebugSideEffect::ProcessQueuedActions(actions)) => {
                assert_eq!(actions.len(), 2);
            }
            _ => panic!("Expected ProcessQueuedActions"),
        }
    }

    #[test]
    fn test_split_area() {
        let layer = make_layer();

        // Disabled: full area returned
        let area = Rect::new(0, 0, 80, 24);
        let (app, banner) = layer.split_area(area);
        assert_eq!(app, area);
        assert_eq!(banner, Rect::ZERO);
    }

    #[test]
    fn test_split_area_enabled() {
        let mut layer = make_layer();
        layer.handle_action(DebugAction::Toggle);

        let area = Rect::new(0, 0, 80, 24);
        let (app, banner) = layer.split_area(area);

        assert_eq!(app.height, 23);
        assert_eq!(banner.height, 1);
        assert_eq!(banner.y, 23);
    }

    #[test]
    fn test_mouse_capture_toggle() {
        let mut layer = make_layer();
        layer.handle_action(DebugAction::Toggle); // Enable debug

        let effect = layer.handle_action(DebugAction::ToggleMouseCapture);
        assert!(matches!(effect, Some(DebugSideEffect::EnableMouseCapture)));
        assert!(layer.freeze().mouse_capture_enabled);

        let effect = layer.handle_action(DebugAction::ToggleMouseCapture);
        assert!(matches!(effect, Some(DebugSideEffect::DisableMouseCapture)));
        assert!(!layer.freeze().mouse_capture_enabled);
    }

    // Tests for simple() API
    #[test]
    fn test_simple_creation() {
        let layer: DebugLayer<TestAction, SimpleDebugContext> = DebugLayer::simple();
        assert!(!layer.is_enabled());
        assert!(layer.freeze().snapshot.is_none());

        // Verify config uses SimpleDebugContext::Debug
        assert_eq!(layer.config().debug_context, SimpleDebugContext::Debug);
    }

    #[test]
    fn test_simple_toggle_works() {
        let mut layer: DebugLayer<TestAction, SimpleDebugContext> = DebugLayer::simple();

        // Enable
        layer.handle_action(DebugAction::Toggle);
        assert!(layer.is_enabled());

        // Disable
        layer.handle_action(DebugAction::Toggle);
        assert!(!layer.is_enabled());
    }

    #[test]
    fn test_simple_with_toggle_key() {
        let layer: DebugLayer<TestAction, SimpleDebugContext> =
            DebugLayer::simple_with_toggle_key(&["F11"]);

        assert!(!layer.is_enabled());

        // Check that F11 is registered (by checking keybindings)
        let keybindings = &layer.config().keybindings;
        let toggle_keys = keybindings.get_context_bindings(SimpleDebugContext::Debug);
        assert!(toggle_keys.is_some());

        if let Some(bindings) = toggle_keys {
            let keys = bindings.get("debug.toggle");
            assert!(keys.is_some());
            assert!(keys.unwrap().contains(&"F11".to_string()));
        }
    }

    #[test]
    fn test_simple_has_default_keybindings() {
        let layer: DebugLayer<TestAction, SimpleDebugContext> = DebugLayer::simple();
        let keybindings = &layer.config().keybindings;

        // Check all default bindings are present
        let bindings = keybindings
            .get_context_bindings(SimpleDebugContext::Debug)
            .unwrap();

        assert!(bindings.contains_key("debug.toggle"));
        assert!(bindings.contains_key("debug.state"));
        assert!(bindings.contains_key("debug.copy"));
        assert!(bindings.contains_key("debug.mouse"));

        // Check toggle has F12 and Esc
        let toggle_keys = bindings.get("debug.toggle").unwrap();
        assert!(toggle_keys.contains(&"F12".to_string()));
        assert!(toggle_keys.contains(&"Esc".to_string()));
    }
}
