//! Test utilities for tui-dispatch applications
//!
//! This module provides helpers for testing TUI applications built with tui-dispatch:
//!
//! - [`key`]: Create `KeyEvent` from string (e.g., `key("ctrl+p")`)
//! - [`TestHarness`]: Generic test harness with action channel and state management
//! - Assertion macros for verifying emitted actions
//!
//! # Example
//!
//! ```ignore
//! use tui_dispatch::testing::{key, TestHarness};
//!
//! #[derive(Clone, Debug, PartialEq)]
//! enum Action {
//!     Increment,
//!     Decrement,
//! }
//!
//! let mut harness = TestHarness::<i32, Action>::new(0);
//!
//! // Dispatch actions
//! harness.dispatch(Action::Increment);
//!
//! // Check emitted actions
//! harness.emit(Action::Decrement);
//! let emitted = harness.drain_emitted();
//! assert!(emitted.contains(&Action::Decrement));
//! ```

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;

use crate::event::{ComponentId, Event, EventContext, EventKind};
use crate::keybindings::parse_key_string;
use crate::{Action, ActionCategory};

/// Create a `KeyEvent` from a key string.
///
/// This is a convenience wrapper around [`parse_key_string`] that panics
/// if the key string is invalid, making it suitable for use in tests.
///
/// # Examples
///
/// ```
/// use tui_dispatch_core::testing::key;
/// use crossterm::event::{KeyCode, KeyModifiers};
///
/// let k = key("q");
/// assert_eq!(k.code, KeyCode::Char('q'));
///
/// let k = key("ctrl+p");
/// assert_eq!(k.code, KeyCode::Char('p'));
/// assert!(k.modifiers.contains(KeyModifiers::CONTROL));
///
/// let k = key("shift+tab");
/// assert_eq!(k.code, KeyCode::BackTab);
/// ```
///
/// # Panics
///
/// Panics if the key string cannot be parsed.
pub fn key(s: &str) -> KeyEvent {
    parse_key_string(s).unwrap_or_else(|| panic!("Invalid key string: {:?}", s))
}

/// Create a `KeyEvent` for a character with no modifiers.
///
/// # Examples
///
/// ```
/// use tui_dispatch_core::testing::char_key;
/// use crossterm::event::KeyCode;
///
/// let k = char_key('x');
/// assert_eq!(k.code, KeyCode::Char('x'));
/// ```
pub fn char_key(c: char) -> KeyEvent {
    KeyEvent {
        code: KeyCode::Char(c),
        modifiers: KeyModifiers::empty(),
        kind: crossterm::event::KeyEventKind::Press,
        state: crossterm::event::KeyEventState::empty(),
    }
}

/// Create a `KeyEvent` for a character with Ctrl modifier.
pub fn ctrl_key(c: char) -> KeyEvent {
    KeyEvent {
        code: KeyCode::Char(c),
        modifiers: KeyModifiers::CONTROL,
        kind: crossterm::event::KeyEventKind::Press,
        state: crossterm::event::KeyEventState::empty(),
    }
}

/// Create a `KeyEvent` for a character with Alt modifier.
pub fn alt_key(c: char) -> KeyEvent {
    KeyEvent {
        code: KeyCode::Char(c),
        modifiers: KeyModifiers::ALT,
        kind: crossterm::event::KeyEventKind::Press,
        state: crossterm::event::KeyEventState::empty(),
    }
}

/// Create an `Event<C>` containing a key event from a key string.
///
/// This is useful for testing component `handle_event` methods.
///
/// # Examples
///
/// ```ignore
/// use tui_dispatch::testing::key_event;
///
/// let event = key_event::<MyComponentId>("ctrl+p");
/// let actions = component.handle_event(&event, props);
/// ```
pub fn key_event<C: ComponentId>(s: &str) -> Event<C> {
    Event {
        kind: EventKind::Key(key(s)),
        context: EventContext::default(),
    }
}

/// Create an `Event<C>` from a `KeyEvent`.
///
/// # Examples
///
/// ```ignore
/// use tui_dispatch::testing::{key, into_event};
///
/// let k = key("enter");
/// let event = into_event::<MyComponentId>(k);
/// ```
pub fn into_event<C: ComponentId>(key_event: KeyEvent) -> Event<C> {
    Event {
        kind: EventKind::Key(key_event),
        context: EventContext::default(),
    }
}

/// Generic test harness for tui-dispatch applications.
///
/// Provides:
/// - State management with a simple `state` field
/// - Action channel for capturing emitted actions
/// - Helper methods for dispatching and draining actions
///
/// # Type Parameters
///
/// - `S`: The state type
/// - `A`: The action type (must implement [`Action`])
///
/// # Example
///
/// ```ignore
/// use tui_dispatch::testing::TestHarness;
///
/// #[derive(Clone, Debug, PartialEq)]
/// enum MyAction { Foo, Bar(i32) }
///
/// let mut harness = TestHarness::<MyState, MyAction>::new(MyState::default());
///
/// // Emit actions (simulating what handlers would do)
/// harness.emit(MyAction::Foo);
/// harness.emit(MyAction::Bar(42));
///
/// // Drain and verify
/// let actions = harness.drain_emitted();
/// assert_eq!(actions.len(), 2);
/// ```
pub struct TestHarness<S, A: Action> {
    /// The application state under test
    pub state: S,
    /// Sender for emitting actions
    tx: mpsc::UnboundedSender<A>,
    /// Receiver for draining emitted actions
    rx: mpsc::UnboundedReceiver<A>,
}

impl<S, A: Action> TestHarness<S, A> {
    /// Create a new test harness with the given initial state.
    pub fn new(state: S) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self { state, tx, rx }
    }

    /// Get a clone of the action sender for passing to handlers.
    pub fn sender(&self) -> mpsc::UnboundedSender<A> {
        self.tx.clone()
    }

    /// Emit an action (simulates what a handler would do).
    pub fn emit(&self, action: A) {
        let _ = self.tx.send(action);
    }

    /// Drain all emitted actions from the channel.
    pub fn drain_emitted(&mut self) -> Vec<A> {
        let mut actions = Vec::new();
        while let Ok(action) = self.rx.try_recv() {
            actions.push(action);
        }
        actions
    }

    /// Check if any actions were emitted.
    pub fn has_emitted(&mut self) -> bool {
        !self.drain_emitted().is_empty()
    }
}

impl<S: Default, A: Action> Default for TestHarness<S, A> {
    fn default() -> Self {
        Self::new(S::default())
    }
}

/// Category-aware methods for TestHarness.
///
/// These methods are available when the action type implements [`ActionCategory`],
/// enabling filtering and assertions by action category.
impl<S, A: ActionCategory> TestHarness<S, A> {
    /// Drain all emitted actions that belong to a specific category.
    ///
    /// Actions not matching the category remain in the channel for later draining.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use tui_dispatch::testing::TestHarness;
    ///
    /// let mut harness = TestHarness::<MyState, MyAction>::new(MyState::default());
    ///
    /// // Emit various actions
    /// harness.emit(MyAction::SearchStart);
    /// harness.emit(MyAction::ConnectionFormOpen);
    /// harness.emit(MyAction::SearchClear);
    ///
    /// // Drain only search-related actions
    /// let search_actions = harness.drain_category("search");
    /// assert_eq!(search_actions.len(), 2);
    ///
    /// // Other actions remain
    /// let remaining = harness.drain_emitted();
    /// assert_eq!(remaining.len(), 1);
    /// ```
    pub fn drain_category(&mut self, category: &str) -> Vec<A> {
        let all = self.drain_emitted();
        let mut matching = Vec::new();
        let mut non_matching = Vec::new();

        for action in all {
            if action.category() == Some(category) {
                matching.push(action);
            } else {
                non_matching.push(action);
            }
        }

        // Re-emit non-matching actions
        for action in non_matching {
            let _ = self.tx.send(action);
        }

        matching
    }

    /// Check if any action of the given category was emitted.
    ///
    /// This drains only the matching category, leaving other actions in the channel.
    pub fn has_category(&mut self, category: &str) -> bool {
        !self.drain_category(category).is_empty()
    }
}

/// Assert that a specific action was emitted.
///
/// # Example
///
/// ```ignore
/// use tui_dispatch::testing::assert_emitted;
///
/// let actions = harness.drain_emitted();
/// assert_emitted!(actions, Action::Increment);
/// assert_emitted!(actions, Action::SetValue(42));
/// ```
#[macro_export]
macro_rules! assert_emitted {
    ($actions:expr, $pattern:pat $(if $guard:expr)?) => {
        assert!(
            $actions.iter().any(|a| matches!(a, $pattern $(if $guard)?)),
            "Expected action matching `{}` to be emitted, but got: {:?}",
            stringify!($pattern),
            $actions
        );
    };
}

/// Assert that a specific action was NOT emitted.
///
/// # Example
///
/// ```ignore
/// use tui_dispatch::testing::assert_not_emitted;
///
/// let actions = harness.drain_emitted();
/// assert_not_emitted!(actions, Action::Quit);
/// ```
#[macro_export]
macro_rules! assert_not_emitted {
    ($actions:expr, $pattern:pat $(if $guard:expr)?) => {
        assert!(
            !$actions.iter().any(|a| matches!(a, $pattern $(if $guard)?)),
            "Expected action matching `{}` NOT to be emitted, but it was: {:?}",
            stringify!($pattern),
            $actions
        );
    };
}

/// Find and return the first action matching a pattern.
///
/// # Example
///
/// ```ignore
/// use tui_dispatch::testing::find_emitted;
///
/// let actions = harness.drain_emitted();
/// if let Some(Action::SetValue(v)) = find_emitted!(actions, Action::SetValue(_)) {
///     assert_eq!(*v, 42);
/// }
/// ```
#[macro_export]
macro_rules! find_emitted {
    ($actions:expr, $pattern:pat $(if $guard:expr)?) => {
        $actions.iter().find(|a| matches!(a, $pattern $(if $guard)?))
    };
}

/// Count how many actions match a pattern.
///
/// # Example
///
/// ```ignore
/// use tui_dispatch::testing::count_emitted;
///
/// let actions = harness.drain_emitted();
/// assert_eq!(count_emitted!(actions, Action::Tick), 3);
/// ```
#[macro_export]
macro_rules! count_emitted {
    ($actions:expr, $pattern:pat $(if $guard:expr)?) => {
        $actions.iter().filter(|a| matches!(a, $pattern $(if $guard)?)).count()
    };
}

/// Assert that an action of a specific category was emitted.
///
/// This requires the action type to implement [`ActionCategory`].
///
/// # Example
///
/// ```ignore
/// use tui_dispatch::testing::assert_category_emitted;
///
/// let actions = harness.drain_emitted();
/// assert_category_emitted!(actions, "search");
/// assert_category_emitted!(actions, "connection_form");
/// ```
#[macro_export]
macro_rules! assert_category_emitted {
    ($actions:expr, $category:expr) => {
        assert!(
            $actions.iter().any(|a| {
                use $crate::ActionCategory;
                a.category() == Some($category)
            }),
            "Expected action with category `{}` to be emitted, but got: {:?}",
            $category,
            $actions
        );
    };
}

/// Assert that NO action of a specific category was emitted.
///
/// This requires the action type to implement [`ActionCategory`].
///
/// # Example
///
/// ```ignore
/// use tui_dispatch::testing::assert_category_not_emitted;
///
/// let actions = harness.drain_emitted();
/// assert_category_not_emitted!(actions, "search");
/// ```
#[macro_export]
macro_rules! assert_category_not_emitted {
    ($actions:expr, $category:expr) => {
        assert!(
            !$actions.iter().any(|a| {
                use $crate::ActionCategory;
                a.category() == Some($category)
            }),
            "Expected NO action with category `{}` to be emitted, but found: {:?}",
            $category,
            $actions
                .iter()
                .filter(|a| {
                    use $crate::ActionCategory;
                    a.category() == Some($category)
                })
                .collect::<Vec<_>>()
        );
    };
}

/// Count how many actions belong to a specific category.
///
/// This requires the action type to implement [`ActionCategory`].
///
/// # Example
///
/// ```ignore
/// use tui_dispatch::testing::count_category;
///
/// let actions = harness.drain_emitted();
/// assert_eq!(count_category!(actions, "search"), 3);
/// ```
#[macro_export]
macro_rules! count_category {
    ($actions:expr, $category:expr) => {{
        use $crate::ActionCategory;
        $actions
            .iter()
            .filter(|a| a.category() == Some($category))
            .count()
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_simple() {
        let k = key("q");
        assert_eq!(k.code, KeyCode::Char('q'));
        assert_eq!(k.modifiers, KeyModifiers::empty());
    }

    #[test]
    fn test_key_with_ctrl() {
        let k = key("ctrl+p");
        assert_eq!(k.code, KeyCode::Char('p'));
        assert!(k.modifiers.contains(KeyModifiers::CONTROL));
    }

    #[test]
    fn test_key_special() {
        let k = key("esc");
        assert_eq!(k.code, KeyCode::Esc);

        let k = key("enter");
        assert_eq!(k.code, KeyCode::Enter);

        let k = key("shift+tab");
        assert_eq!(k.code, KeyCode::BackTab);
    }

    #[test]
    fn test_char_key() {
        let k = char_key('x');
        assert_eq!(k.code, KeyCode::Char('x'));
        assert_eq!(k.modifiers, KeyModifiers::empty());
    }

    #[test]
    fn test_ctrl_key() {
        let k = ctrl_key('c');
        assert_eq!(k.code, KeyCode::Char('c'));
        assert!(k.modifiers.contains(KeyModifiers::CONTROL));
    }

    #[derive(Clone, Debug, PartialEq)]
    enum TestAction {
        Foo,
        Bar(i32),
    }

    impl crate::Action for TestAction {
        fn name(&self) -> &'static str {
            match self {
                TestAction::Foo => "Foo",
                TestAction::Bar(_) => "Bar",
            }
        }
    }

    #[test]
    fn test_harness_emit_and_drain() {
        let mut harness = TestHarness::<(), TestAction>::new(());

        harness.emit(TestAction::Foo);
        harness.emit(TestAction::Bar(42));

        let actions = harness.drain_emitted();
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0], TestAction::Foo);
        assert_eq!(actions[1], TestAction::Bar(42));

        // Drain again should be empty
        let actions = harness.drain_emitted();
        assert!(actions.is_empty());
    }

    #[test]
    fn test_assert_macros() {
        let actions = vec![TestAction::Foo, TestAction::Bar(42)];

        assert_emitted!(actions, TestAction::Foo);
        assert_emitted!(actions, TestAction::Bar(42));
        assert_emitted!(actions, TestAction::Bar(_));

        assert_not_emitted!(actions, TestAction::Bar(99));

        let found = find_emitted!(actions, TestAction::Bar(_));
        assert!(found.is_some());

        let count = count_emitted!(actions, TestAction::Bar(_));
        assert_eq!(count, 1);
    }
}
