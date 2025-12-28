//! Debug layer configuration

use crate::keybindings::{BindingContext, Keybindings};
use ratatui::style::{Color, Modifier, Style};

/// Style configuration for debug UI
#[derive(Debug, Clone)]
pub struct DebugStyle {
    /// Background style for the banner
    pub banner_bg: Style,
    /// Title style (e.g., "DEBUG" label)
    pub title_style: Style,
    /// Key hint style (e.g., "F12")
    pub key_style: Style,
    /// Label style (e.g., "resume")
    pub label_style: Style,
    /// Value style for status items
    pub value_style: Style,
    /// Dim factor for background (0.0-1.0)
    pub dim_factor: f32,
}

impl Default for DebugStyle {
    fn default() -> Self {
        Self {
            banner_bg: Style::default().bg(Color::Rgb(20, 20, 30)),
            title_style: Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
            key_style: Style::default().fg(Color::Rgb(20, 20, 30)).bg(Color::Cyan),
            label_style: Style::default().fg(Color::Rgb(150, 150, 160)),
            value_style: Style::default().fg(Color::White),
            dim_factor: 0.7,
        }
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
}
