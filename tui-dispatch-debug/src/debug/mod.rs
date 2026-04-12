//! Debug and inspection utilities for TUI applications
//!
//! This module provides tools for debugging TUI applications:
//!
//! - **DebugLayer**: High-level wrapper with automatic rendering (recommended)
//! - **Action Logging**: Pattern-based filtering for action logs
//! - **Frame Freeze**: Capture and inspect UI state
//! - **Cell Inspection**: Examine individual buffer cells
//! - **Debug Widgets**: Render debug overlays and tables
//!
//! # Quick Start (Recommended)
//!
//! ```ignore
//! use tui_dispatch_debug::debug::DebugLayer;
//!
//! // Minimal setup with sensible defaults (F12 toggle key)
//! let debug = DebugLayer::<MyAction>::simple().active(args.debug);
//!
//! // In render loop:
//! debug.render_state(frame, &state, |f, area| {
//!     render_main_ui(f, area, &state);
//! });
//!
//! // Built-in keybindings (when debug mode is active):
//! // - Toggle key (e.g., F12): Toggle debug mode
//! // - S: Show/hide state overlay
//! // - B: Toggle debug banner position
//! // - /: Edit action-log search query
//! // - n/N: Next/previous action-log search match
//! // - J/K, arrows, PgUp/PgDn, g/G: Scroll overlays
//! // - Y: Copy frozen frame to clipboard
//! // - W: Save state snapshot as JSON
//! // - I: Toggle mouse capture for cell inspection
//! ```
//!
//! # Customization
//!
//! ```ignore
//! use crossterm::event::KeyCode;
//! use tui_dispatch_debug::debug::{BannerPosition, DebugLayer, DebugStyle};
//!
//! let debug = DebugLayer::<MyAction>::new(KeyCode::F(11))
//!     .with_banner_position(BannerPosition::Top)
//!     .with_action_log_capacity(500)
//!     .with_style(DebugStyle::default())
//!     .active(args.debug);
//! ```
//!
//! # Manual Control (Escape Hatch)
//!
//! ```ignore
//! // Split area manually
//! let (app_area, banner_area) = debug.split_area(frame.area());
//!
//! // Custom layout
//! render_my_ui(frame, app_area);
//!
//! // Let debug layer render its parts
//! debug.render_overlay(frame, app_area);
//! debug.render_banner(frame, banner_area);
//! ```
//!
//! # State Inspection
//!
//! Implement `DebugState` for your state types:
//!
//! ```ignore
//! use tui_dispatch::debug::{DebugState, DebugSection};
//!
//! impl DebugState for AppState {
//!     fn debug_sections(&self) -> Vec<DebugSection> {
//!         vec![
//!             DebugSection::new("Connection")
//!                 .entry("host", &self.host)
//!                 .entry("status", format!("{:?}", self.status)),
//!         ]
//!     }
//! }
//!
//! // Then show it:
//! debug.show_state_overlay(&app_state);
//! ```
//!
//! # Action Logging
//!
//! Use [`ActionLoggerMiddleware`] for pattern-based action filtering:
//!
//! ```
//! use tui_dispatch_debug::debug::ActionLoggerConfig;
//!
//! // Log only Search* and Connect* actions
//! let config = ActionLoggerConfig::new(Some("Search*,Connect*"), None);
//! // Prefix filters: cat:<category>, name:<ExactActionName>
//! let config = ActionLoggerConfig::new(Some("cat:search,name:SearchSubmit"), None);
//!
//! // Log everything except Tick and Render (default excludes)
//! let config = ActionLoggerConfig::default();
//! ```
//!
//! # Low-Level API
//!
//! For full control, use [`DebugFreeze`] directly:
//!
//! ```ignore
//! use tui_dispatch::debug::{DebugFreeze, paint_snapshot, dim_buffer};
//!
//! let debug: DebugFreeze<MyAction> = DebugFreeze::default();
//!
//! // In render loop:
//! if debug.enabled {
//!     if debug.pending_capture || debug.snapshot.is_none() {
//!         render_app(f, state);
//!         debug.capture(f.buffer());
//!     } else {
//!         paint_snapshot(f, debug.snapshot.as_ref().unwrap());
//!     }
//!     dim_buffer(f.buffer_mut(), 0.7);
//!     render_debug_overlay(f, &debug);
//! }
//! ```

pub mod action_logger;
pub mod actions;
pub mod cell;
pub mod config;
pub mod format;
pub mod layer;
pub mod scroll_state;
pub mod state;
pub mod table;
pub mod widgets;

// Re-export commonly used types

// High-level API (recommended)
pub use actions::{DebugAction, DebugSideEffect};
pub use config::{
    default_debug_keybindings, default_debug_keybindings_with_toggle, DebugConfig, DebugStyle,
    KeyStyles, ScrollbarStyle, StatusItem,
};
pub use layer::{BannerPosition, DebugLayer, DebugOutcome};
pub use state::{DebugEntry, DebugSection, DebugState, DebugWrapper};

// Action logging
pub use action_logger::{
    glob_match, ActionLog, ActionLogConfig, ActionLogEntry, ActionLoggerConfig,
    ActionLoggerMiddleware,
};

// Scroll/cursor utilities
pub use scroll_state::ScrollState;

// Low-level API
pub use cell::{
    format_color_compact, format_modifier_compact, inspect_cell, point_in_rect, CellPreview,
};
pub use format::{debug_string, debug_string_compact, debug_string_pretty, pretty_reformat};
pub use table::{
    ActionLogDisplayEntry, ActionLogOverlay, ComponentDetailOverlay, ComponentSnapshot,
    ComponentsOverlay, DebugOverlay, DebugTableBuilder, DebugTableOverlay, DebugTableRow,
    StateEntryDetail,
};
pub use widgets::{
    buffer_to_text, dim_buffer, paint_snapshot, ActionLogStyle, ActionLogWidget, BannerItem,
    CellPreviewWidget, DebugBanner, DebugTableStyle, DebugTableWidget,
};

use ratatui::buffer::Buffer;
use tui_dispatch_core::keybindings::BindingContext;
use tui_dispatch_core::runtime::{DebugAdapter, DebugHooks, RenderContext};
use tui_dispatch_core::{Action, ActionParams};

// ============================================================================
// SimpleDebugContext - Built-in context for simple debug layer usage
// ============================================================================

/// Built-in context for default debug keybindings.
///
/// Use this with [`default_debug_keybindings`] or
/// [`default_debug_keybindings_with_toggle`] when wiring debug commands into
/// your own keybinding system.
///
/// # Example
///
/// ```ignore
/// use tui_dispatch_debug::debug::default_debug_keybindings;
///
/// let keybindings = default_debug_keybindings();
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum SimpleDebugContext {
    /// Normal application context (not debug mode)
    #[default]
    Normal,
    /// Debug mode context (when freeze is active)
    Debug,
}

impl BindingContext for SimpleDebugContext {
    fn name(&self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Debug => "debug",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "normal" => Some(Self::Normal),
            "debug" => Some(Self::Debug),
            _ => None,
        }
    }

    fn all() -> &'static [Self] {
        &[Self::Normal, Self::Debug]
    }
}

/// Debug freeze state for capturing and inspecting UI frames
///
/// Generic over the action type `A` to store queued actions while frozen.
///
/// # Example
///
/// ```ignore
/// use tui_dispatch_debug::debug::DebugFreeze;
///
/// // In your app state:
/// struct AppState {
///     debug: DebugFreeze<MyAction>,
///     // ... other fields
/// }
///
/// // Toggle freeze on F12:
/// fn handle_action(state: &mut AppState, action: MyAction) {
///     match action {
///         MyAction::ToggleDebug => {
///             state.debug.toggle();
///         }
///         other if state.debug.enabled => {
///             // Queue actions while frozen
///             state.debug.queue(other);
///         }
///         // ... normal handling
///     }
/// }
/// ```
#[derive(Debug)]
pub struct DebugFreeze<A> {
    /// Whether debug/freeze mode is enabled
    pub enabled: bool,
    /// Flag to capture the next frame
    pub pending_capture: bool,
    /// The captured buffer snapshot
    pub snapshot: Option<Buffer>,
    /// Plain text version of snapshot (for clipboard)
    pub snapshot_text: String,
    /// Actions queued while frozen
    pub queued_actions: Vec<A>,
    /// Feedback message to display
    pub message: Option<String>,
    /// Currently displayed overlay
    pub overlay: Option<DebugOverlay>,
    /// Whether mouse capture mode is enabled (for position inspection)
    pub mouse_capture_enabled: bool,
}

impl<A> Default for DebugFreeze<A> {
    fn default() -> Self {
        Self {
            enabled: false,
            pending_capture: false,
            snapshot: None,
            snapshot_text: String::new(),
            queued_actions: Vec::new(),
            message: None,
            overlay: None,
            mouse_capture_enabled: false,
        }
    }
}

impl<A> DebugFreeze<A> {
    /// Create a new debug freeze state
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggle freeze mode on/off
    ///
    /// When enabling, sets pending_capture to capture the next frame.
    /// When disabling, clears the snapshot and queued actions.
    pub fn toggle(&mut self) {
        if self.enabled {
            // Disable
            self.enabled = false;
            self.snapshot = None;
            self.snapshot_text.clear();
            self.overlay = None;
            self.message = None;
            // Note: queued_actions should be processed by the app before clearing
        } else {
            // Enable
            self.enabled = true;
            self.pending_capture = true;
            self.queued_actions.clear();
            self.message = None;
        }
    }

    /// Enable freeze mode
    pub fn enable(&mut self) {
        if !self.enabled {
            self.toggle();
        }
    }

    /// Disable freeze mode
    pub fn disable(&mut self) {
        if self.enabled {
            self.toggle();
        }
    }

    /// Capture the current buffer as a snapshot
    pub fn capture(&mut self, buffer: &Buffer) {
        self.snapshot = Some(buffer.clone());
        self.snapshot_text = buffer_to_text(buffer);
        self.pending_capture = false;
    }

    /// Request a new capture on the next frame
    pub fn request_capture(&mut self) {
        self.pending_capture = true;
    }

    /// Queue an action to be processed when freeze is disabled
    pub fn queue(&mut self, action: A) {
        self.queued_actions.push(action);
    }

    /// Take all queued actions, leaving the queue empty
    pub fn take_queued(&mut self) -> Vec<A> {
        std::mem::take(&mut self.queued_actions)
    }

    /// Set a feedback message
    pub fn set_message(&mut self, msg: impl Into<String>) {
        self.message = Some(msg.into());
    }

    /// Clear the feedback message
    pub fn clear_message(&mut self) {
        self.message = None;
    }

    /// Set the current overlay
    pub fn set_overlay(&mut self, overlay: DebugOverlay) {
        self.overlay = Some(overlay);
    }

    /// Clear the current overlay
    pub fn clear_overlay(&mut self) {
        self.overlay = None;
    }

    /// Toggle mouse capture mode
    pub fn toggle_mouse_capture(&mut self) {
        self.mouse_capture_enabled = !self.mouse_capture_enabled;
    }
}

impl<S, A> DebugAdapter<S, A> for DebugLayer<A>
where
    S: DebugState + 'static,
    A: Action + ActionParams,
{
    fn render(
        &mut self,
        frame: &mut ratatui::Frame,
        state: &S,
        render_ctx: RenderContext,
        render_fn: &mut dyn FnMut(&mut ratatui::Frame, ratatui::layout::Rect, &S, RenderContext),
    ) {
        self.render_state(frame, state, |f, area| {
            render_fn(f, area, state, render_ctx);
        });
    }

    fn handle_event(
        &mut self,
        event: &tui_dispatch_core::EventKind,
        state: &S,
        action_tx: &tokio::sync::mpsc::UnboundedSender<A>,
    ) -> Option<bool> {
        self.handle_event_with_state(event, state)
            .dispatch_queued(|action| {
                let _ = action_tx.send(action);
            })
    }

    fn log_action(&mut self, action: &A) {
        DebugLayer::log_action(self, action);
    }

    fn is_enabled(&self) -> bool {
        DebugLayer::is_enabled(self)
    }
}

impl<A: Action> DebugHooks<A> for DebugLayer<A> {
    #[cfg(feature = "tasks")]
    fn with_task_manager(self, tasks: &tui_dispatch_core::tasks::TaskManager<A>) -> Self {
        DebugLayer::with_task_manager(self, tasks)
    }

    #[cfg(feature = "subscriptions")]
    fn with_subscriptions(self, subs: &tui_dispatch_core::subscriptions::Subscriptions<A>) -> Self {
        DebugLayer::with_subscriptions(self, subs)
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

    #[test]
    fn test_debug_freeze_toggle() {
        let mut freeze: DebugFreeze<TestAction> = DebugFreeze::new();

        assert!(!freeze.enabled);

        freeze.toggle();
        assert!(freeze.enabled);
        assert!(freeze.pending_capture);

        freeze.toggle();
        assert!(!freeze.enabled);
        assert!(freeze.snapshot.is_none());
    }

    #[test]
    fn test_debug_freeze_queue() {
        let mut freeze: DebugFreeze<TestAction> = DebugFreeze::new();
        freeze.enable();

        freeze.queue(TestAction::Foo);
        freeze.queue(TestAction::Bar);

        assert_eq!(freeze.queued_actions.len(), 2);

        let queued = freeze.take_queued();
        assert_eq!(queued.len(), 2);
        assert!(freeze.queued_actions.is_empty());
    }

    #[test]
    fn test_debug_freeze_message() {
        let mut freeze: DebugFreeze<TestAction> = DebugFreeze::new();

        freeze.set_message("Test message");
        assert_eq!(freeze.message, Some("Test message".to_string()));

        freeze.clear_message();
        assert!(freeze.message.is_none());
    }

    #[test]
    fn test_simple_debug_context_binding_context() {
        use tui_dispatch_core::keybindings::BindingContext;

        // Test name()
        assert_eq!(SimpleDebugContext::Normal.name(), "normal");
        assert_eq!(SimpleDebugContext::Debug.name(), "debug");

        // Test from_name()
        assert_eq!(
            SimpleDebugContext::from_name("normal"),
            Some(SimpleDebugContext::Normal)
        );
        assert_eq!(
            SimpleDebugContext::from_name("debug"),
            Some(SimpleDebugContext::Debug)
        );
        assert_eq!(SimpleDebugContext::from_name("invalid"), None);

        // Test all()
        let all = SimpleDebugContext::all();
        assert_eq!(all.len(), 2);
        assert!(all.contains(&SimpleDebugContext::Normal));
        assert!(all.contains(&SimpleDebugContext::Debug));
    }

    #[test]
    fn test_simple_debug_context_default() {
        let ctx: SimpleDebugContext = Default::default();
        assert_eq!(ctx, SimpleDebugContext::Normal);
    }
}
