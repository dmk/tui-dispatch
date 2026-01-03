//! Debug actions and side effects

/// Debug actions provided by tui-dispatch
///
/// These are framework-level debug actions that apps can map from their own
/// action types via keybindings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugAction {
    /// Toggle debug freeze mode on/off
    Toggle,
    /// Copy frozen frame to clipboard
    CopyFrame,
    /// Toggle state overlay
    ToggleState,
    /// Toggle action log overlay
    ToggleActionLog,
    /// Toggle mouse capture mode for cell inspection
    ToggleMouseCapture,
    /// Inspect cell at position (from mouse click)
    InspectCell { column: u16, row: u16 },
    /// Close current overlay
    CloseOverlay,
    /// Request a new frame capture
    RequestCapture,
    /// Scroll action log up
    ActionLogScrollUp,
    /// Scroll action log down
    ActionLogScrollDown,
    /// Scroll action log to top
    ActionLogScrollTop,
    /// Scroll action log to bottom
    ActionLogScrollBottom,
    /// Page up in action log
    ActionLogPageUp,
    /// Page down in action log
    ActionLogPageDown,
    /// Show detail for selected action
    ActionLogShowDetail,
    /// Go back from detail view to action log
    ActionLogBackToList,
}

impl DebugAction {
    /// Standard command names for keybinding lookup
    pub const CMD_TOGGLE: &'static str = "debug.toggle";
    pub const CMD_COPY_FRAME: &'static str = "debug.copy";
    pub const CMD_TOGGLE_STATE: &'static str = "debug.state";
    pub const CMD_TOGGLE_ACTION_LOG: &'static str = "debug.action_log";
    pub const CMD_TOGGLE_MOUSE: &'static str = "debug.mouse";
    pub const CMD_CLOSE_OVERLAY: &'static str = "debug.close";

    /// Try to parse a command string into a debug action
    pub fn from_command(cmd: &str) -> Option<Self> {
        match cmd {
            Self::CMD_TOGGLE => Some(Self::Toggle),
            Self::CMD_COPY_FRAME => Some(Self::CopyFrame),
            Self::CMD_TOGGLE_STATE => Some(Self::ToggleState),
            Self::CMD_TOGGLE_ACTION_LOG => Some(Self::ToggleActionLog),
            Self::CMD_TOGGLE_MOUSE => Some(Self::ToggleMouseCapture),
            Self::CMD_CLOSE_OVERLAY => Some(Self::CloseOverlay),
            _ => None,
        }
    }

    /// Get the command string for this action
    pub fn command(&self) -> Option<&'static str> {
        match self {
            Self::Toggle => Some(Self::CMD_TOGGLE),
            Self::CopyFrame => Some(Self::CMD_COPY_FRAME),
            Self::ToggleState => Some(Self::CMD_TOGGLE_STATE),
            Self::ToggleActionLog => Some(Self::CMD_TOGGLE_ACTION_LOG),
            Self::ToggleMouseCapture => Some(Self::CMD_TOGGLE_MOUSE),
            Self::CloseOverlay => Some(Self::CMD_CLOSE_OVERLAY),
            // These don't have command strings (triggered programmatically)
            Self::InspectCell { .. }
            | Self::RequestCapture
            | Self::ActionLogScrollUp
            | Self::ActionLogScrollDown
            | Self::ActionLogScrollTop
            | Self::ActionLogScrollBottom
            | Self::ActionLogPageUp
            | Self::ActionLogPageDown
            | Self::ActionLogShowDetail
            | Self::ActionLogBackToList => None,
        }
    }
}

/// Side effects that the app needs to handle after debug actions
///
/// The `DebugLayer` returns these when processing actions that require
/// app-level handling (clipboard access, queued action processing).
#[derive(Debug)]
pub enum DebugSideEffect<A> {
    /// Process queued actions (when exiting debug mode)
    ///
    /// These actions were queued while the UI was frozen and should
    /// now be dispatched through the normal action pipeline.
    ProcessQueuedActions(Vec<A>),

    /// Copy text to clipboard
    ///
    /// The app should use its preferred clipboard mechanism (OSC52, etc).
    CopyToClipboard(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_command() {
        assert_eq!(
            DebugAction::from_command("debug.toggle"),
            Some(DebugAction::Toggle)
        );
        assert_eq!(
            DebugAction::from_command("debug.copy"),
            Some(DebugAction::CopyFrame)
        );
        assert_eq!(
            DebugAction::from_command("debug.state"),
            Some(DebugAction::ToggleState)
        );
        assert_eq!(
            DebugAction::from_command("debug.action_log"),
            Some(DebugAction::ToggleActionLog)
        );
        assert_eq!(DebugAction::from_command("unknown"), None);
    }

    #[test]
    fn test_command_roundtrip() {
        let actions = [
            DebugAction::Toggle,
            DebugAction::CopyFrame,
            DebugAction::ToggleState,
            DebugAction::ToggleActionLog,
            DebugAction::ToggleMouseCapture,
            DebugAction::CloseOverlay,
        ];

        for action in actions {
            let cmd = action.command().expect("should have command");
            let parsed = DebugAction::from_command(cmd).expect("should parse");
            assert_eq!(parsed, action);
        }
    }

    #[test]
    fn test_scroll_actions_no_command() {
        // Scroll actions are triggered programmatically, not via commands
        assert!(DebugAction::ActionLogScrollUp.command().is_none());
        assert!(DebugAction::ActionLogScrollDown.command().is_none());
        assert!(DebugAction::ActionLogScrollTop.command().is_none());
        assert!(DebugAction::ActionLogScrollBottom.command().is_none());
    }
}
