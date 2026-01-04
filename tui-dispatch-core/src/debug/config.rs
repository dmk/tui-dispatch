//! Debug layer configuration

use super::SimpleDebugContext;
use crate::keybindings::{BindingContext, Keybindings};
use ratatui::style::{Color, Modifier, Style};

// Neon color palette (matches memtui theme)
const NEON_PURPLE: Color = Color::Rgb(160, 100, 220);
const NEON_PINK: Color = Color::Rgb(255, 100, 150);
const NEON_AMBER: Color = Color::Rgb(255, 191, 0);
const NEON_CYAN: Color = Color::Rgb(0, 255, 255);
const NEON_GREEN: Color = Color::Rgb(80, 255, 120);
const ELECTRIC_BLUE: Color = Color::Rgb(80, 180, 255);
const KINDA_GREEN: Color = Color::Rgb(40, 220, 80);

const BG_DEEP: Color = Color::Rgb(12, 14, 22);
const BG_PANEL: Color = Color::Rgb(18, 21, 32);
const BG_SURFACE: Color = Color::Rgb(26, 30, 44);
const BG_HIGHLIGHT: Color = Color::Rgb(45, 50, 70);

const TEXT_PRIMARY: Color = Color::Rgb(240, 240, 245);
const TEXT_SECONDARY: Color = Color::Rgb(150, 150, 160);

/// Style configuration for debug UI
#[derive(Debug, Clone)]
pub struct DebugStyle {
    /// Background style for the banner
    pub banner_bg: Style,
    /// Title style (e.g., "DEBUG" label)
    pub title_style: Style,
    /// Key styles for different actions (toggle, state, copy, mouse)
    pub key_styles: KeyStyles,
    /// Scrollbar styling for debug overlays
    pub scrollbar: ScrollbarStyle,
    /// Label style (e.g., "resume")
    pub label_style: Style,
    /// Value style for status items
    pub value_style: Style,
    /// Dim factor for background (0.0-1.0)
    pub dim_factor: f32,
}

/// Style and symbol overrides for debug scrollbars
#[derive(Debug, Clone, Default)]
pub struct ScrollbarStyle {
    /// Style for the scrollbar thumb
    pub thumb: Style,
    /// Style for the scrollbar track
    pub track: Style,
    /// Style for the begin symbol
    pub begin: Style,
    /// Style for the end symbol
    pub end: Style,
    /// Override for the thumb symbol
    pub thumb_symbol: Option<&'static str>,
    /// Override for the track symbol
    pub track_symbol: Option<&'static str>,
    /// Override for the begin symbol
    pub begin_symbol: Option<&'static str>,
    /// Override for the end symbol
    pub end_symbol: Option<&'static str>,
}

/// Styles for different debug key hints
#[derive(Debug, Clone)]
pub struct KeyStyles {
    /// Style for toggle key (F12)
    pub toggle: Style,
    /// Style for state key (S)
    pub state: Style,
    /// Style for copy key (Y)
    pub copy: Style,
    /// Style for mouse key (I)
    pub mouse: Style,
    /// Style for actions key (A)
    pub actions: Style,
}

impl Default for KeyStyles {
    fn default() -> Self {
        let key_base = |bg: Color| {
            Style::default()
                .fg(BG_DEEP)
                .bg(bg)
                .add_modifier(Modifier::BOLD)
        };
        Self {
            toggle: key_base(NEON_PINK),
            state: key_base(NEON_CYAN),
            copy: key_base(NEON_AMBER),
            mouse: key_base(ELECTRIC_BLUE),
            actions: key_base(KINDA_GREEN),
        }
    }
}

impl Default for DebugStyle {
    fn default() -> Self {
        Self {
            banner_bg: Style::default().bg(BG_DEEP),
            title_style: Style::default()
                .fg(BG_DEEP)
                .bg(NEON_PURPLE)
                .add_modifier(Modifier::BOLD),
            key_styles: KeyStyles::default(),
            scrollbar: ScrollbarStyle::default(),
            label_style: Style::default().fg(TEXT_SECONDARY),
            value_style: Style::default().fg(TEXT_PRIMARY),
            dim_factor: 0.7,
        }
    }
}

// Re-export colors for use in table styling
impl DebugStyle {
    /// Get the neon purple color
    pub const fn neon_purple() -> Color {
        NEON_PURPLE
    }
    /// Get the neon cyan color
    pub const fn neon_cyan() -> Color {
        NEON_CYAN
    }
    /// Get the neon amber color
    pub const fn neon_amber() -> Color {
        NEON_AMBER
    }
    /// Get the neon green color
    pub const fn neon_green() -> Color {
        NEON_GREEN
    }
    /// Get the deep background color
    pub const fn bg_deep() -> Color {
        BG_DEEP
    }
    /// Get the panel background color
    pub const fn bg_panel() -> Color {
        BG_PANEL
    }
    /// Get the surface background color
    pub const fn bg_surface() -> Color {
        BG_SURFACE
    }
    /// Get the highlight background color (for selected items)
    pub const fn bg_highlight() -> Color {
        BG_HIGHLIGHT
    }
    /// Get the primary text color
    pub const fn text_primary() -> Color {
        TEXT_PRIMARY
    }
    /// Get the secondary text color
    pub const fn text_secondary() -> Color {
        TEXT_SECONDARY
    }
}

/// Status item for the debug banner
#[derive(Debug, Clone)]
pub struct StatusItem {
    /// Label/key text
    pub label: String,
    /// Value text
    pub value: String,
    /// Optional custom style
    pub style: Option<Style>,
}

impl StatusItem {
    /// Create a new status item
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            style: None,
        }
    }

    /// Set custom style
    pub fn with_style(mut self, style: Style) -> Self {
        self.style = Some(style);
        self
    }
}

/// Configuration for the debug layer
#[derive(Clone)]
pub struct DebugConfig<C: BindingContext> {
    /// Keybindings for debug commands
    pub keybindings: Keybindings<C>,
    /// Context used for debug-specific bindings
    pub debug_context: C,
    /// Style configuration
    pub style: DebugStyle,
    /// Status items provider (called each render)
    status_provider: Option<StatusProvider>,
}

type StatusProvider = std::sync::Arc<dyn Fn() -> Vec<StatusItem> + Send + Sync>;

impl<C: BindingContext> std::fmt::Debug for DebugConfig<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DebugConfig")
            .field("debug_context", &self.debug_context.name())
            .field("style", &self.style)
            .field(
                "status_provider",
                &self.status_provider.as_ref().map(|_| "<fn>"),
            )
            .finish()
    }
}

impl<C: BindingContext> DebugConfig<C> {
    /// Create a new config with keybindings and debug context
    pub fn new(keybindings: Keybindings<C>, debug_context: C) -> Self {
        Self {
            keybindings,
            debug_context,
            style: DebugStyle::default(),
            status_provider: None,
        }
    }

    /// Set the style
    pub fn with_style(mut self, style: DebugStyle) -> Self {
        self.style = style;
        self
    }

    /// Set a status provider function
    ///
    /// This function is called each render to get status items for the banner.
    pub fn with_status_provider<F>(mut self, provider: F) -> Self
    where
        F: Fn() -> Vec<StatusItem> + Send + Sync + 'static,
    {
        self.status_provider = Some(std::sync::Arc::new(provider));
        self
    }

    /// Get status items from the provider (if any)
    pub fn status_items(&self) -> Vec<StatusItem> {
        self.status_provider
            .as_ref()
            .map(|f| f())
            .unwrap_or_default()
    }
}

// ============================================================================
// Default Debug Keybindings
// ============================================================================

/// Create default debug keybindings with `F12`/`Esc` to toggle.
///
/// Default bindings:
/// - `debug.toggle`: F12, Esc
/// - `debug.state`: s, S
/// - `debug.copy`: y, Y
/// - `debug.mouse`: i, I
///
/// # Example
///
/// ```
/// use tui_dispatch_core::debug::default_debug_keybindings;
///
/// let kb = default_debug_keybindings();
/// ```
pub fn default_debug_keybindings() -> Keybindings<SimpleDebugContext> {
    default_debug_keybindings_with_toggle(&["F12", "Esc"])
}

/// Create debug keybindings with custom toggle key(s).
///
/// Same as [`default_debug_keybindings`] but uses the provided key(s)
/// for toggling debug mode instead of `F12`/`Esc`.
///
/// # Example
///
/// ```
/// use tui_dispatch_core::debug::default_debug_keybindings_with_toggle;
///
/// // Use F11 instead of F12
/// let kb = default_debug_keybindings_with_toggle(&["F11"]);
///
/// // Multiple toggle keys
/// let kb = default_debug_keybindings_with_toggle(&["F11", "Ctrl+D"]);
/// ```
pub fn default_debug_keybindings_with_toggle(
    toggle_keys: &[&str],
) -> Keybindings<SimpleDebugContext> {
    let mut kb = Keybindings::new();
    kb.add(
        SimpleDebugContext::Debug,
        "debug.toggle",
        toggle_keys.iter().map(|s| (*s).into()).collect(),
    );
    kb.add(
        SimpleDebugContext::Debug,
        "debug.state",
        vec!["s".into(), "S".into()],
    );
    kb.add(
        SimpleDebugContext::Debug,
        "debug.copy",
        vec!["y".into(), "Y".into()],
    );
    kb.add(
        SimpleDebugContext::Debug,
        "debug.mouse",
        vec!["i".into(), "I".into()],
    );
    kb.add(
        SimpleDebugContext::Debug,
        "debug.action_log",
        vec!["a".into(), "A".into()],
    );
    kb
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_status_item() {
        let item = StatusItem::new("keys", "42");
        assert_eq!(item.label, "keys");
        assert_eq!(item.value, "42");
        assert!(item.style.is_none());

        let styled = item.with_style(Style::default().fg(Color::Red));
        assert!(styled.style.is_some());
    }

    #[test]
    fn test_config_with_status_provider() {
        let config = DebugConfig::new(Keybindings::new(), TestContext::Debug)
            .with_status_provider(|| vec![StatusItem::new("test", "value")]);

        let items = config.status_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "test");
    }

    #[test]
    fn test_config_without_provider() {
        let config: DebugConfig<TestContext> =
            DebugConfig::new(Keybindings::new(), TestContext::Debug);
        let items = config.status_items();
        assert!(items.is_empty());
    }

    #[test]
    fn test_default_debug_keybindings() {
        let kb = default_debug_keybindings();
        let bindings = kb.get_context_bindings(SimpleDebugContext::Debug).unwrap();

        // Check all bindings exist
        assert!(bindings.contains_key("debug.toggle"));
        assert!(bindings.contains_key("debug.state"));
        assert!(bindings.contains_key("debug.copy"));
        assert!(bindings.contains_key("debug.mouse"));

        // Check default toggle keys
        let toggle = bindings.get("debug.toggle").unwrap();
        assert!(toggle.contains(&"F12".to_string()));
        assert!(toggle.contains(&"Esc".to_string()));

        // Check state keys
        let state = bindings.get("debug.state").unwrap();
        assert!(state.contains(&"s".to_string()));
        assert!(state.contains(&"S".to_string()));

        // Check copy keys
        let copy = bindings.get("debug.copy").unwrap();
        assert!(copy.contains(&"y".to_string()));
        assert!(copy.contains(&"Y".to_string()));

        // Check mouse keys
        let mouse = bindings.get("debug.mouse").unwrap();
        assert!(mouse.contains(&"i".to_string()));
        assert!(mouse.contains(&"I".to_string()));
    }

    #[test]
    fn test_default_debug_keybindings_with_toggle_custom() {
        let kb = default_debug_keybindings_with_toggle(&["F11"]);
        let bindings = kb.get_context_bindings(SimpleDebugContext::Debug).unwrap();

        // Check custom toggle key
        let toggle = bindings.get("debug.toggle").unwrap();
        assert!(toggle.contains(&"F11".to_string()));
        assert!(!toggle.contains(&"F12".to_string())); // Should not have F12

        // Other bindings should still be default
        let state = bindings.get("debug.state").unwrap();
        assert!(state.contains(&"s".to_string()));
    }

    #[test]
    fn test_default_debug_keybindings_with_toggle_multiple() {
        let kb = default_debug_keybindings_with_toggle(&["F11", "Ctrl+D"]);
        let bindings = kb.get_context_bindings(SimpleDebugContext::Debug).unwrap();

        let toggle = bindings.get("debug.toggle").unwrap();
        assert!(toggle.contains(&"F11".to_string()));
        assert!(toggle.contains(&"Ctrl+D".to_string()));
    }
}
