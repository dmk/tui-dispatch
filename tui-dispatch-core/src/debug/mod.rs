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
//! # Quick Start (Simple API - Recommended)
//!
//! ```ignore
//! use tui_dispatch::debug::DebugLayer;
//!
//! // One line setup with sensible defaults:
//! let debug = DebugLayer::<MyAction>::simple();
//!
//! // In render loop:
//! debug.render(frame, |f, area| {
//!     render_main_ui(f, area, &state);
//! });
//!
//! // Default keybindings (when debug mode is active):
//! // - F12/Esc: Toggle debug mode
//! // - S: Show/hide state overlay
//! // - Y: Copy frozen frame to clipboard
//! // - I: Toggle mouse capture for cell inspection
//! ```
//!
//! # Custom Configuration
//!
//! ```ignore
//! use tui_dispatch::debug::{DebugLayer, DebugConfig, DebugAction};
//!
//! // Use custom toggle key:
//! let debug = DebugLayer::<MyAction>::simple_with_toggle_key(&["F11"]);
//!
//! // Or full control with custom context:
//! let config = DebugConfig::new(keybindings, MyContext::Debug);
//! let debug: DebugLayer<MyAction, MyContext> = DebugLayer::new(config);
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
//! use tui_dispatch_core::debug::ActionLoggerConfig;
//!
//! // Log only Search* and Connect* actions
//! let config = ActionLoggerConfig::new(Some("Search*,Connect*"), None);
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
pub mod layer;
pub mod state;
pub mod table;
pub mod widgets;

// Re-export commonly used types

// High-level API (recommended)
pub use actions::{DebugAction, DebugSideEffect};
pub use config::{
    default_debug_keybindings, default_debug_keybindings_with_toggle, DebugConfig, DebugStyle,
    KeyStyles, StatusItem,
};
pub use layer::DebugLayer;
pub use state::{DebugEntry, DebugSection, DebugState, DebugWrapper};

// Action logging
pub use action_logger::{
    glob_match, ActionLog, ActionLogConfig, ActionLogEntry, ActionLoggerConfig,
    ActionLoggerMiddleware,
};

// Low-level API
pub use cell::{
    format_color_compact, format_modifier_compact, inspect_cell, point_in_rect, CellPreview,
};
pub use table::{
    ActionLogDisplayEntry, ActionLogOverlay, DebugOverlay, DebugTableBuilder, DebugTableOverlay,
    DebugTableRow,
};
pub use widgets::{
    buffer_to_text, dim_buffer, paint_snapshot, ActionLogStyle, ActionLogWidget, BannerItem,
    CellPreviewWidget, DebugBanner, DebugTableStyle, DebugTableWidget,
};

use crate::keybindings::BindingContext;
use ratatui::buffer::Buffer;

// ============================================================================
// SimpleDebugContext - Built-in context for simple debug layer usage
// ============================================================================

/// Built-in context for simple debug layer usage.
///
/// Use this with [`DebugLayer::simple()`] for zero-configuration debug layer setup.
///
/// # Example
///
/// ```ignore
/// use tui_dispatch::debug::DebugLayer;
///
/// // Uses SimpleDebugContext internally:
/// let debug = DebugLayer::<MyAction>::simple();
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
/// use tui_dispatch_core::debug::DebugFreeze;
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
        use crate::keybindings::BindingContext;

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
