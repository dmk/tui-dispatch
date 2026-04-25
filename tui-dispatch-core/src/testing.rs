//! Test utilities for tui-dispatch applications
//!
//! This module provides helpers for testing TUI applications built with tui-dispatch:
//!
//! - [`key`]: Create `KeyEvent` from string (e.g., `key("ctrl+p")`)
//! - [`key_events`]: Create multiple `Event`s from space-separated key string
//! - [`TestHarness`]: Generic test harness with action channel and state management
//! - [`ActionAssertions`]: Fluent assertion trait for action vectors
//! - Assertion macros for verifying emitted actions
//!
//! # Example
//!
//! ```ignore
//! use tui_dispatch::testing::{key, TestHarness, ActionAssertions};
//!
//! #[derive(Clone, Debug, PartialEq)]
//! enum Action {
//!     Increment,
//!     Decrement,
//! }
//!
//! let mut harness = TestHarness::<i32, Action>::new(0);
//!
//! // Emit and check actions with fluent API
//! harness.emit(Action::Decrement);
//! harness.emit(Action::Increment);
//! let emitted = harness.drain_emitted();
//! emitted.assert_first(Action::Decrement);
//! emitted.assert_contains(Action::Increment);
//! emitted.assert_count(2);
//! ```

use std::fmt::Debug;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;

use crate::event::{ComponentId, Event, EventContext, EventKind};
use crate::keybindings::parse_key_string;
use crate::{Action, ActionCategory};

// ============================================================================
// Fluent Action Assertions
// ============================================================================

/// Fluent assertion trait for action vectors.
///
/// This trait only requires `Debug`, making it usable with any action type.
/// For equality-based assertions (like `assert_first`), use [`ActionAssertionsEq`].
///
/// # Example
///
/// ```ignore
/// use tui_dispatch::testing::ActionAssertions;
///
/// let actions = harness.drain_emitted();
/// actions.assert_not_empty();
/// actions.assert_count(3);
/// actions.assert_any_matches(|a| matches!(a, Action::SelectKey(i) if *i > 0));
/// ```
pub trait ActionAssertions<A> {
    /// Assert that the vector is empty.
    ///
    /// # Panics
    /// Panics if the vector is not empty.
    fn assert_empty(&self);

    /// Assert that the vector is not empty.
    ///
    /// # Panics
    /// Panics if the vector is empty.
    fn assert_not_empty(&self);

    /// Assert that the vector has exactly `n` elements.
    ///
    /// # Panics
    /// Panics if the count doesn't match.
    fn assert_count(&self, n: usize);

    /// Assert that the first action matches a predicate.
    ///
    /// # Panics
    /// Panics if the vector is empty or the predicate returns false.
    fn assert_first_matches<F: Fn(&A) -> bool>(&self, f: F);

    /// Assert that any action matches a predicate.
    ///
    /// # Panics
    /// Panics if no action matches the predicate.
    fn assert_any_matches<F: Fn(&A) -> bool>(&self, f: F);

    /// Assert that all actions match a predicate.
    ///
    /// # Panics
    /// Panics if any action doesn't match the predicate.
    fn assert_all_match<F: Fn(&A) -> bool>(&self, f: F);

    /// Assert that no action matches a predicate.
    ///
    /// # Panics
    /// Panics if any action matches the predicate.
    fn assert_none_match<F: Fn(&A) -> bool>(&self, f: F);
}

/// Equality-based assertions for action vectors.
///
/// This trait requires `PartialEq` for equality comparisons.
/// For predicate-based assertions that don't need `PartialEq`, use [`ActionAssertions`].
///
/// # Example
///
/// ```ignore
/// use tui_dispatch::testing::ActionAssertionsEq;
///
/// let actions = harness.drain_emitted();
/// actions.assert_first(Action::StartSearch);
/// actions.assert_contains(Action::SelectKey(42));
/// ```
pub trait ActionAssertionsEq<A> {
    /// Assert that the first action equals the expected value.
    ///
    /// # Panics
    /// Panics if the vector is empty or the first action doesn't match.
    fn assert_first(&self, expected: A);

    /// Assert that the last action equals the expected value.
    ///
    /// # Panics
    /// Panics if the vector is empty or the last action doesn't match.
    fn assert_last(&self, expected: A);

    /// Assert that the vector contains the expected action.
    ///
    /// # Panics
    /// Panics if no action matches the expected value.
    fn assert_contains(&self, expected: A);

    /// Assert that the vector does not contain the expected action.
    ///
    /// # Panics
    /// Panics if any action matches the expected value.
    fn assert_not_contains(&self, expected: A);
}

// ActionAssertions impl for Vec - only requires Debug
impl<A: Debug> ActionAssertions<A> for Vec<A> {
    fn assert_empty(&self) {
        assert!(
            self.is_empty(),
            "Expected no actions to be emitted, but got: {:?}",
            self
        );
    }

    fn assert_not_empty(&self) {
        assert!(
            !self.is_empty(),
            "Expected actions to be emitted, but got none"
        );
    }

    fn assert_count(&self, n: usize) {
        assert_eq!(
            self.len(),
            n,
            "Expected {} action(s), got {}: {:?}",
            n,
            self.len(),
            self
        );
    }

    fn assert_first_matches<F: Fn(&A) -> bool>(&self, f: F) {
        assert!(
            !self.is_empty(),
            "Expected first action to match predicate, but no actions were emitted"
        );
        assert!(
            f(&self[0]),
            "Expected first action to match predicate, got: {:?}",
            self[0]
        );
    }

    fn assert_any_matches<F: Fn(&A) -> bool>(&self, f: F) {
        assert!(
            self.iter().any(&f),
            "Expected any action to match predicate, but none did: {:?}",
            self
        );
    }

    fn assert_all_match<F: Fn(&A) -> bool>(&self, f: F) {
        for (i, action) in self.iter().enumerate() {
            assert!(
                f(action),
                "Expected all actions to match predicate, but action at index {} didn't: {:?}",
                i,
                action
            );
        }
    }

    fn assert_none_match<F: Fn(&A) -> bool>(&self, f: F) {
        for (i, action) in self.iter().enumerate() {
            assert!(
                !f(action),
                "Expected no action to match predicate, but action at index {} matched: {:?}",
                i,
                action
            );
        }
    }
}

// ActionAssertionsEq impl for Vec - requires PartialEq + Debug
impl<A: PartialEq + Debug> ActionAssertionsEq<A> for Vec<A> {
    fn assert_first(&self, expected: A) {
        assert!(
            !self.is_empty(),
            "Expected first action to be {:?}, but no actions were emitted",
            expected
        );
        assert_eq!(
            &self[0], &expected,
            "Expected first action to be {:?}, got {:?}",
            expected, self[0]
        );
    }

    fn assert_last(&self, expected: A) {
        assert!(
            !self.is_empty(),
            "Expected last action to be {:?}, but no actions were emitted",
            expected
        );
        let last = self.last().unwrap();
        assert_eq!(
            last, &expected,
            "Expected last action to be {:?}, got {:?}",
            expected, last
        );
    }

    fn assert_contains(&self, expected: A) {
        assert!(
            self.iter().any(|a| a == &expected),
            "Expected actions to contain {:?}, but got: {:?}",
            expected,
            self
        );
    }

    fn assert_not_contains(&self, expected: A) {
        assert!(
            !self.iter().any(|a| a == &expected),
            "Expected actions NOT to contain {:?}, but it was found in: {:?}",
            expected,
            self
        );
    }
}

// ActionAssertions impl for slices - only requires Debug
impl<A: Debug> ActionAssertions<A> for [A] {
    fn assert_empty(&self) {
        assert!(
            self.is_empty(),
            "Expected no actions to be emitted, but got: {:?}",
            self
        );
    }

    fn assert_not_empty(&self) {
        assert!(
            !self.is_empty(),
            "Expected actions to be emitted, but got none"
        );
    }

    fn assert_count(&self, n: usize) {
        assert_eq!(
            self.len(),
            n,
            "Expected {} action(s), got {}: {:?}",
            n,
            self.len(),
            self
        );
    }

    fn assert_first_matches<F: Fn(&A) -> bool>(&self, f: F) {
        assert!(
            !self.is_empty(),
            "Expected first action to match predicate, but no actions were emitted"
        );
        assert!(
            f(&self[0]),
            "Expected first action to match predicate, got: {:?}",
            self[0]
        );
    }

    fn assert_any_matches<F: Fn(&A) -> bool>(&self, f: F) {
        assert!(
            self.iter().any(&f),
            "Expected any action to match predicate, but none did: {:?}",
            self
        );
    }

    fn assert_all_match<F: Fn(&A) -> bool>(&self, f: F) {
        for (i, action) in self.iter().enumerate() {
            assert!(
                f(action),
                "Expected all actions to match predicate, but action at index {} didn't: {:?}",
                i,
                action
            );
        }
    }

    fn assert_none_match<F: Fn(&A) -> bool>(&self, f: F) {
        for (i, action) in self.iter().enumerate() {
            assert!(
                !f(action),
                "Expected no action to match predicate, but action at index {} matched: {:?}",
                i,
                action
            );
        }
    }
}

// ActionAssertionsEq impl for slices - requires PartialEq + Debug
impl<A: PartialEq + Debug> ActionAssertionsEq<A> for [A] {
    fn assert_first(&self, expected: A) {
        assert!(
            !self.is_empty(),
            "Expected first action to be {:?}, but no actions were emitted",
            expected
        );
        assert_eq!(
            &self[0], &expected,
            "Expected first action to be {:?}, got {:?}",
            expected, self[0]
        );
    }

    fn assert_last(&self, expected: A) {
        assert!(
            !self.is_empty(),
            "Expected last action to be {:?}, but no actions were emitted",
            expected
        );
        let last = self.last().unwrap();
        assert_eq!(
            last, &expected,
            "Expected last action to be {:?}, got {:?}",
            expected, last
        );
    }

    fn assert_contains(&self, expected: A) {
        assert!(
            self.iter().any(|a| a == &expected),
            "Expected actions to contain {:?}, but got: {:?}",
            expected,
            self
        );
    }

    fn assert_not_contains(&self, expected: A) {
        assert!(
            !self.iter().any(|a| a == &expected),
            "Expected actions NOT to contain {:?}, but it was found in: {:?}",
            expected,
            self
        );
    }
}

// ============================================================================
// Key Event Helpers
// ============================================================================

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

/// Create multiple `Event<C>` from a space-separated key string.
///
/// This is useful for simulating key sequences in tests.
///
/// # Examples
///
/// ```ignore
/// use tui_dispatch::testing::key_events;
///
/// // Single key
/// let events = key_events::<MyComponentId>("ctrl+p");
/// assert_eq!(events.len(), 1);
///
/// // Multiple keys separated by spaces
/// let events = key_events::<MyComponentId>("ctrl+p down down enter");
/// assert_eq!(events.len(), 4);
///
/// // Type characters
/// let events = key_events::<MyComponentId>("h e l l o");
/// assert_eq!(events.len(), 5);
/// ```
///
/// # Panics
///
/// Panics if any key string in the sequence cannot be parsed.
pub fn key_events<C: ComponentId>(keys: &str) -> Vec<Event<C>> {
    keys.split_whitespace().map(|k| key_event::<C>(k)).collect()
}

/// Parse multiple key strings into `KeyEvent`s.
///
/// Similar to [`key_events`] but returns raw `KeyEvent`s instead of `Event<C>`.
///
/// # Examples
///
/// ```
/// use tui_dispatch_core::testing::keys;
/// use crossterm::event::KeyCode;
///
/// let key_events = keys("ctrl+c esc enter");
/// assert_eq!(key_events.len(), 3);
/// assert_eq!(key_events[0].code, KeyCode::Char('c'));
/// assert_eq!(key_events[1].code, KeyCode::Esc);
/// assert_eq!(key_events[2].code, KeyCode::Enter);
/// ```
pub fn keys(key_str: &str) -> Vec<KeyEvent> {
    key_str.split_whitespace().map(key).collect()
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

    /// Simulate async action completion (semantic alias for [`Self::emit`]).
    ///
    /// Use this when simulating backend responses or async operation results.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use tui_dispatch::testing::TestHarness;
    ///
    /// harness.complete_action(Action::DidConnect { id: "conn-1", .. });
    /// harness.complete_action(Action::DidScanKeys { keys: vec!["foo", "bar"] });
    /// ```
    pub fn complete_action(&self, action: A) {
        self.emit(action);
    }

    /// Simulate multiple async action completions.
    ///
    /// # Example
    ///
    /// ```ignore
    /// harness.complete_actions([
    ///     Action::DidConnect { id: "conn-1" },
    ///     Action::DidLoadValue { key: "foo", value: "bar" },
    /// ]);
    /// ```
    pub fn complete_actions(&self, actions: impl IntoIterator<Item = A>) {
        for action in actions {
            self.emit(action);
        }
    }

    /// Send a sequence of key events and collect actions from a handler.
    ///
    /// Parses the space-separated key string and calls the handler for each event,
    /// collecting all returned actions.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use tui_dispatch::testing::TestHarness;
    ///
    /// let mut harness = TestHarness::<AppState, Action>::new(AppState::default());
    ///
    /// // Send key sequence and collect actions
    /// let actions = harness.send_keys("ctrl+p down down enter", |state, event| {
    ///     component.handle_event(&event.kind, ComponentProps { state })
    /// });
    ///
    /// actions.assert_contains(Action::SelectItem(2));
    /// ```
    pub fn send_keys<C, H, I>(&mut self, keys: &str, mut handler: H) -> Vec<A>
    where
        C: ComponentId,
        I: IntoIterator<Item = A>,
        H: FnMut(&mut S, Event<C>) -> I,
    {
        let events = key_events::<C>(keys);
        let mut all_actions = Vec::new();
        for event in events {
            let actions = handler(&mut self.state, event);
            all_actions.extend(actions);
        }
        all_actions
    }

    /// Send a sequence of key events, calling handler and emitting returned actions.
    ///
    /// Unlike [`Self::send_keys`], this method emits returned actions to the harness channel,
    /// allowing you to drain them later.
    ///
    /// # Example
    ///
    /// ```ignore
    /// harness.send_keys_emit("ctrl+p down enter", |state, event| {
    ///     component.handle_event(&event.kind, props)
    /// });
    ///
    /// let actions = harness.drain_emitted();
    /// actions.assert_contains(Action::Confirm);
    /// ```
    pub fn send_keys_emit<C, H, I>(&mut self, keys: &str, mut handler: H)
    where
        C: ComponentId,
        I: IntoIterator<Item = A>,
        H: FnMut(&mut S, Event<C>) -> I,
    {
        let events = key_events::<C>(keys);
        for event in events {
            let actions = handler(&mut self.state, event);
            for action in actions {
                self.emit(action);
            }
        }
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

// ============================================================================
// State Assertions
// ============================================================================

/// Assert that a field of the harness state has an expected value.
///
/// # Example
///
/// ```ignore
/// use tui_dispatch::testing::{TestHarness, assert_state};
///
/// let harness = TestHarness::<AppState, Action>::new(AppState::default());
/// assert_state!(harness, counter, 0);
/// assert_state!(harness, ui.focused_panel, Panel::Keys);
/// ```
#[macro_export]
macro_rules! assert_state {
    ($harness:expr, $($field:tt).+, $expected:expr) => {
        assert_eq!(
            $harness.state.$($field).+,
            $expected,
            "Expected state.{} = {:?}, got {:?}",
            stringify!($($field).+),
            $expected,
            $harness.state.$($field).+
        );
    };
}

/// Assert that a field of the harness state matches a pattern.
///
/// # Example
///
/// ```ignore
/// use tui_dispatch::testing::{TestHarness, assert_state_matches};
///
/// assert_state_matches!(harness, connection_status, ConnectionStatus::Connected { .. });
/// ```
#[macro_export]
macro_rules! assert_state_matches {
    ($harness:expr, $($field:tt).+, $pattern:pat $(if $guard:expr)?) => {
        assert!(
            matches!($harness.state.$($field).+, $pattern $(if $guard)?),
            "Expected state.{} to match `{}`, got {:?}",
            stringify!($($field).+),
            stringify!($pattern),
            $harness.state.$($field).+
        );
    };
}

// ============================================================================
// Render Harness
// ============================================================================

use ratatui::backend::{Backend, TestBackend};
use ratatui::buffer::Buffer;
use ratatui::Terminal;

/// Test harness for capturing rendered output.
///
/// Provides utilities for rendering components to a test buffer and
/// converting the output to strings for snapshot testing.
///
/// # Example
///
/// ```ignore
/// use tui_dispatch::testing::RenderHarness;
///
/// let mut render = RenderHarness::new(80, 24);
///
/// // Render a component
/// let output = render.render_to_string(|frame| {
///     my_component.render(frame, frame.area(), props);
/// });
///
/// // Use with insta for snapshot testing
/// insta::assert_snapshot!(output);
/// ```
pub struct RenderHarness {
    terminal: Terminal<TestBackend>,
}

impl RenderHarness {
    /// Create a new render harness with the specified dimensions.
    pub fn new(width: u16, height: u16) -> Self {
        let backend = TestBackend::new(width, height);
        let terminal = Terminal::new(backend).expect("Failed to create test terminal");
        Self { terminal }
    }

    /// Render using the provided function and return the buffer.
    pub fn render<F>(&mut self, render_fn: F) -> &Buffer
    where
        F: FnOnce(&mut ratatui::Frame),
    {
        self.terminal
            .draw(render_fn)
            .expect("Failed to draw to test terminal");
        self.terminal.backend().buffer()
    }

    /// Render and convert the buffer to a string representation.
    ///
    /// The output includes ANSI escape codes for colors and styles.
    pub fn render_to_string<F>(&mut self, render_fn: F) -> String
    where
        F: FnOnce(&mut ratatui::Frame),
    {
        let buffer = self.render(render_fn);
        buffer_to_string(buffer)
    }

    /// Render and convert to a plain string (no ANSI codes).
    ///
    /// Useful for simple text assertions without style information.
    pub fn render_to_string_plain<F>(&mut self, render_fn: F) -> String
    where
        F: FnOnce(&mut ratatui::Frame),
    {
        let buffer = self.render(render_fn);
        buffer_to_string_plain(buffer)
    }

    /// Get the current terminal size.
    pub fn size(&self) -> (u16, u16) {
        let area = self.terminal.backend().size().unwrap_or_default();
        (area.width, area.height)
    }

    /// Resize the terminal.
    pub fn resize(&mut self, width: u16, height: u16) {
        self.terminal.backend_mut().resize(width, height);
    }
}

/// Convert a ratatui Buffer to a string with ANSI escape codes.
///
/// Each cell's foreground color, background color, and modifiers are
/// converted to ANSI escape sequences.
pub fn buffer_to_string(buffer: &Buffer) -> String {
    use ratatui::style::{Color, Modifier};
    use std::fmt::Write;

    let area = buffer.area();
    let mut result = String::new();

    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let cell = &buffer[(x, y)];

            // Start with reset
            let _ = write!(result, "\x1b[0m");

            // Foreground color
            match cell.fg {
                Color::Reset => {}
                Color::Black => result.push_str("\x1b[30m"),
                Color::Red => result.push_str("\x1b[31m"),
                Color::Green => result.push_str("\x1b[32m"),
                Color::Yellow => result.push_str("\x1b[33m"),
                Color::Blue => result.push_str("\x1b[34m"),
                Color::Magenta => result.push_str("\x1b[35m"),
                Color::Cyan => result.push_str("\x1b[36m"),
                Color::Gray => result.push_str("\x1b[37m"),
                Color::DarkGray => result.push_str("\x1b[90m"),
                Color::LightRed => result.push_str("\x1b[91m"),
                Color::LightGreen => result.push_str("\x1b[92m"),
                Color::LightYellow => result.push_str("\x1b[93m"),
                Color::LightBlue => result.push_str("\x1b[94m"),
                Color::LightMagenta => result.push_str("\x1b[95m"),
                Color::LightCyan => result.push_str("\x1b[96m"),
                Color::White => result.push_str("\x1b[97m"),
                Color::Rgb(r, g, b) => {
                    let _ = write!(result, "\x1b[38;2;{};{};{}m", r, g, b);
                }
                Color::Indexed(i) => {
                    let _ = write!(result, "\x1b[38;5;{}m", i);
                }
            }

            // Background color
            match cell.bg {
                Color::Reset => {}
                Color::Black => result.push_str("\x1b[40m"),
                Color::Red => result.push_str("\x1b[41m"),
                Color::Green => result.push_str("\x1b[42m"),
                Color::Yellow => result.push_str("\x1b[43m"),
                Color::Blue => result.push_str("\x1b[44m"),
                Color::Magenta => result.push_str("\x1b[45m"),
                Color::Cyan => result.push_str("\x1b[46m"),
                Color::Gray => result.push_str("\x1b[47m"),
                Color::DarkGray => result.push_str("\x1b[100m"),
                Color::LightRed => result.push_str("\x1b[101m"),
                Color::LightGreen => result.push_str("\x1b[102m"),
                Color::LightYellow => result.push_str("\x1b[103m"),
                Color::LightBlue => result.push_str("\x1b[104m"),
                Color::LightMagenta => result.push_str("\x1b[105m"),
                Color::LightCyan => result.push_str("\x1b[106m"),
                Color::White => result.push_str("\x1b[107m"),
                Color::Rgb(r, g, b) => {
                    let _ = write!(result, "\x1b[48;2;{};{};{}m", r, g, b);
                }
                Color::Indexed(i) => {
                    let _ = write!(result, "\x1b[48;5;{}m", i);
                }
            }

            // Modifiers
            if cell.modifier.contains(Modifier::BOLD) {
                result.push_str("\x1b[1m");
            }
            if cell.modifier.contains(Modifier::DIM) {
                result.push_str("\x1b[2m");
            }
            if cell.modifier.contains(Modifier::ITALIC) {
                result.push_str("\x1b[3m");
            }
            if cell.modifier.contains(Modifier::UNDERLINED) {
                result.push_str("\x1b[4m");
            }
            if cell.modifier.contains(Modifier::REVERSED) {
                result.push_str("\x1b[7m");
            }
            if cell.modifier.contains(Modifier::CROSSED_OUT) {
                result.push_str("\x1b[9m");
            }

            result.push_str(cell.symbol());
        }
        result.push_str("\x1b[0m\n");
    }

    result
}

/// Convert a ratatui Buffer to a plain string (no ANSI codes).
///
/// Only extracts the text content, ignoring colors and styles.
pub fn buffer_to_string_plain(buffer: &Buffer) -> String {
    let area = buffer.area();
    let mut result = String::new();

    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let cell = &buffer[(x, y)];
            result.push_str(cell.symbol());
        }
        result.push('\n');
    }

    result
}

/// Convert a specific rect of a buffer to a plain string.
///
/// Useful for testing a specific region of the rendered output.
pub fn buffer_rect_to_string_plain(buffer: &Buffer, rect: ratatui::layout::Rect) -> String {
    let mut result = String::new();

    for y in rect.top()..rect.bottom() {
        for x in rect.left()..rect.right() {
            if x < buffer.area().right() && y < buffer.area().bottom() {
                let cell = &buffer[(x, y)];
                result.push_str(cell.symbol());
            }
        }
        result.push('\n');
    }

    result
}

// ============================================================================
// StoreTestHarness - Integrated Store + Test Harness
// ============================================================================

/// Test harness combining Store + action channel + render capabilities.
///
/// Provides an integrated testing experience for applications using
/// [`Store`](crate::Store) with [`ReducerResult`](crate::ReducerResult)
/// reducers.
///
/// # Example
///
/// ```ignore
/// use tui_dispatch::testing::StoreTestHarness;
///
/// let mut harness = StoreTestHarness::new(AppState::default(), reducer);
///
/// // Dispatch and check state
/// harness.dispatch(Action::Increment);
/// harness.assert_state(|s| s.count == 1);
///
/// // Send keys through component
/// let actions = harness.send_keys::<NumericComponentId, _, _>("j j enter", |state, event| {
///     component.handle_event(&event.kind, Props { state })
/// });
/// actions.assert_contains(Action::Select(2));
///
/// // Snapshot testing
/// let output = harness.render_plain(60, 24, |f, area, state| {
///     component.render(f, area, Props { state });
/// });
/// assert!(output.contains("expected text"));
/// ```
pub struct StoreTestHarness<S, A: Action, E = crate::NoEffect> {
    store: crate::Store<S, A, E>,
    tx: mpsc::UnboundedSender<A>,
    rx: mpsc::UnboundedReceiver<A>,
    effects: Vec<E>,
    render: Option<RenderHarness>,
    default_size: (u16, u16),
}

impl<S, A: Action, E> StoreTestHarness<S, A, E> {
    /// Create a new harness with initial state and reducer.
    pub fn new(state: S, reducer: crate::Reducer<S, A, E>) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            store: crate::Store::new(state, reducer),
            tx,
            rx,
            effects: Vec::new(),
            render: None,
            default_size: (80, 24),
        }
    }

    /// Set the default terminal size for rendering.
    pub fn with_size(mut self, width: u16, height: u16) -> Self {
        self.default_size = (width, height);
        self
    }

    // === Store Operations ===

    /// Dispatch an action to the store.
    ///
    /// Returns the full reducer result.
    pub fn dispatch(&mut self, action: A) -> crate::ReducerResult<E> {
        self.store.dispatch(action)
    }

    /// Dispatch an action and automatically collect its effects.
    ///
    /// Returns `true` if state changed. Effects are collected internally
    /// and can be retrieved with [`Self::drain_effects`].
    pub fn dispatch_collect(&mut self, action: A) -> bool {
        let result = self.store.dispatch(action);
        self.effects.extend(result.effects);
        result.changed
    }

    /// Dispatch multiple actions in sequence.
    ///
    /// Returns a vector of booleans indicating which dispatches changed state.
    pub fn dispatch_all(&mut self, actions: impl IntoIterator<Item = A>) -> Vec<bool> {
        actions
            .into_iter()
            .map(|a| self.dispatch_collect(a))
            .collect()
    }

    /// Get a reference to the current state.
    pub fn state(&self) -> &S {
        self.store.state()
    }

    /// Get a mutable reference to the state for test setup.
    pub fn state_mut(&mut self) -> &mut S {
        self.store.state_mut()
    }

    // === Effect Operations ===

    /// Drain all collected effects.
    pub fn drain_effects(&mut self) -> Vec<E> {
        std::mem::take(&mut self.effects)
    }

    /// Check if any effects were collected.
    pub fn has_effects(&self) -> bool {
        !self.effects.is_empty()
    }

    /// Get the number of collected effects.
    pub fn effect_count(&self) -> usize {
        self.effects.len()
    }

    // === Action Channel (for async simulation) ===

    /// Get a sender clone for simulating async action completions.
    pub fn sender(&self) -> mpsc::UnboundedSender<A> {
        self.tx.clone()
    }

    /// Emit an action to the channel (simulates async completion).
    pub fn emit(&self, action: A) {
        let _ = self.tx.send(action);
    }

    /// Simulate async action completion (semantic alias for [`Self::emit`]).
    pub fn complete_action(&self, action: A) {
        self.emit(action);
    }

    /// Simulate multiple async action completions.
    pub fn complete_actions(&self, actions: impl IntoIterator<Item = A>) {
        for action in actions {
            self.emit(action);
        }
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

    /// Process all emitted actions through the store.
    ///
    /// Drains the channel and dispatches each action to the store.
    /// Returns `(changed_count, total_count)`.
    pub fn process_emitted(&mut self) -> (usize, usize) {
        let actions = self.drain_emitted();
        let total = actions.len();
        let changed = actions
            .into_iter()
            .filter(|a| self.dispatch_collect(a.clone()))
            .count();
        (changed, total)
    }

    // === Key/Event Handling ===

    /// Send a sequence of key events and collect actions from a handler.
    ///
    /// Parses the space-separated key string and calls the handler for each event,
    /// collecting all returned actions.
    pub fn send_keys<C, H, I>(&mut self, keys: &str, mut handler: H) -> Vec<A>
    where
        C: ComponentId,
        I: IntoIterator<Item = A>,
        H: FnMut(&mut S, Event<C>) -> I,
    {
        let events = key_events::<C>(keys);
        let mut all_actions = Vec::new();
        for event in events {
            let actions = handler(self.store.state_mut(), event);
            all_actions.extend(actions);
        }
        all_actions
    }

    /// Send keys and dispatch returned actions to the store.
    ///
    /// Returns the actions that were dispatched.
    pub fn send_keys_dispatch<C, H, I>(&mut self, keys: &str, mut handler: H) -> Vec<A>
    where
        C: ComponentId,
        I: IntoIterator<Item = A>,
        H: FnMut(&mut S, Event<C>) -> I,
    {
        let events = key_events::<C>(keys);
        let mut all_actions = Vec::new();
        for event in events {
            let actions: Vec<A> = handler(self.store.state_mut(), event).into_iter().collect();
            for action in actions {
                self.dispatch_collect(action.clone());
                all_actions.push(action);
            }
        }
        all_actions
    }

    // === State Assertions ===

    /// Assert a condition on the current state.
    ///
    /// # Panics
    ///
    /// Panics if the predicate returns `false`.
    pub fn assert_state<F>(&self, predicate: F)
    where
        F: FnOnce(&S) -> bool,
    {
        assert!(predicate(self.state()), "State assertion failed");
    }

    /// Assert a condition with a custom message.
    pub fn assert_state_msg<F>(&self, predicate: F, msg: &str)
    where
        F: FnOnce(&S) -> bool,
    {
        assert!(predicate(self.state()), "{}", msg);
    }

    // === Render Operations ===

    fn ensure_render(&mut self, width: u16, height: u16) {
        if self.render.is_none() || self.render.as_ref().map(|r| r.size()) != Some((width, height))
        {
            self.render = Some(RenderHarness::new(width, height));
        }
    }

    /// Render using the provided function, returns string with ANSI codes.
    pub fn render<F>(&mut self, width: u16, height: u16, render_fn: F) -> String
    where
        F: FnOnce(&mut ratatui::Frame, ratatui::layout::Rect, &S),
    {
        self.ensure_render(width, height);
        let store = &self.store;
        let render = self.render.as_mut().unwrap();
        render.render_to_string(|frame| {
            let area = frame.area();
            render_fn(frame, area, store.state());
        })
    }

    /// Render to plain string (no ANSI codes).
    pub fn render_plain<F>(&mut self, width: u16, height: u16, render_fn: F) -> String
    where
        F: FnOnce(&mut ratatui::Frame, ratatui::layout::Rect, &S),
    {
        self.ensure_render(width, height);
        let store = &self.store;
        let render = self.render.as_mut().unwrap();
        render.render_to_string_plain(|frame| {
            let area = frame.area();
            render_fn(frame, area, store.state());
        })
    }

    /// Render with default terminal size (set via [`Self::with_size`]).
    pub fn render_default<F>(&mut self, render_fn: F) -> String
    where
        F: FnOnce(&mut ratatui::Frame, ratatui::layout::Rect, &S),
    {
        let (w, h) = self.default_size;
        self.render(w, h, render_fn)
    }

    /// Render with default size to plain string.
    pub fn render_default_plain<F>(&mut self, render_fn: F) -> String
    where
        F: FnOnce(&mut ratatui::Frame, ratatui::layout::Rect, &S),
    {
        let (w, h) = self.default_size;
        self.render_plain(w, h, render_fn)
    }
}

impl<S: Default, A: Action> Default for StoreTestHarness<S, A> {
    fn default() -> Self {
        Self::new(S::default(), |_, _| crate::ReducerResult::unchanged())
    }
}

// ============================================================================
// EffectAssertions - Fluent assertions for effect vectors
// ============================================================================

/// Fluent assertions for effect vectors.
///
/// Method names are prefixed with `effects_` to avoid conflicts with
/// [`ActionAssertions`] when both traits are in scope.
///
/// # Example
///
/// ```ignore
/// use tui_dispatch::testing::EffectAssertions;
///
/// let effects = harness.drain_effects();
/// effects.effects_count(1);
/// effects.effects_first_matches(|e| matches!(e, Effect::FetchWeather { .. }));
/// ```
pub trait EffectAssertions<E> {
    /// Assert the effect vector is empty.
    fn effects_empty(&self);
    /// Assert the effect vector is not empty.
    fn effects_not_empty(&self);
    /// Assert the effect vector has exactly `n` elements.
    fn effects_count(&self, n: usize);
    /// Assert any effect matches the predicate.
    fn effects_any_matches<F: Fn(&E) -> bool>(&self, f: F);
    /// Assert the first effect matches the predicate.
    fn effects_first_matches<F: Fn(&E) -> bool>(&self, f: F);
    /// Assert all effects match the predicate.
    fn effects_all_match<F: Fn(&E) -> bool>(&self, f: F);
    /// Assert no effects match the predicate.
    fn effects_none_match<F: Fn(&E) -> bool>(&self, f: F);
}

impl<E: std::fmt::Debug> EffectAssertions<E> for Vec<E> {
    fn effects_empty(&self) {
        assert!(self.is_empty(), "Expected no effects, got {:?}", self);
    }

    fn effects_not_empty(&self) {
        assert!(!self.is_empty(), "Expected effects, got none");
    }

    fn effects_count(&self, n: usize) {
        assert_eq!(
            self.len(),
            n,
            "Expected {} effects, got {}: {:?}",
            n,
            self.len(),
            self
        );
    }

    fn effects_any_matches<F: Fn(&E) -> bool>(&self, f: F) {
        assert!(
            self.iter().any(&f),
            "No effect matched predicate in {:?}",
            self
        );
    }

    fn effects_first_matches<F: Fn(&E) -> bool>(&self, f: F) {
        let first = self.first().expect("Expected at least one effect");
        assert!(f(first), "First effect {:?} did not match predicate", first);
    }

    fn effects_all_match<F: Fn(&E) -> bool>(&self, f: F) {
        for (i, e) in self.iter().enumerate() {
            assert!(f(e), "Effect at index {} did not match: {:?}", i, e);
        }
    }

    fn effects_none_match<F: Fn(&E) -> bool>(&self, f: F) {
        for (i, e) in self.iter().enumerate() {
            assert!(!f(e), "Effect at index {} unexpectedly matched: {:?}", i, e);
        }
    }
}

/// Equality-based assertions for effect vectors.
///
/// Method names are prefixed with `effects_` to avoid conflicts with
/// [`ActionAssertionsEq`] when both traits are in scope.
pub trait EffectAssertionsEq<E> {
    /// Assert the vector contains the expected effect.
    fn effects_contains(&self, expected: E);
    /// Assert the first effect equals the expected.
    fn effects_first_eq(&self, expected: E);
    /// Assert the last effect equals the expected.
    fn effects_last_eq(&self, expected: E);
}

impl<E: PartialEq + std::fmt::Debug> EffectAssertionsEq<E> for Vec<E> {
    fn effects_contains(&self, expected: E) {
        assert!(
            self.contains(&expected),
            "Expected to contain {:?}, got {:?}",
            expected,
            self
        );
    }

    fn effects_first_eq(&self, expected: E) {
        let first = self.first().expect("Expected at least one effect");
        assert_eq!(first, &expected, "First effect mismatch");
    }

    fn effects_last_eq(&self, expected: E) {
        let last = self.last().expect("Expected at least one effect");
        assert_eq!(last, &expected, "Last effect mismatch");
    }
}

// ============================================================================
// Time Control (Feature-gated)
// ============================================================================

/// Time control utilities for testing debounced actions.
///
/// These functions require the `testing-time` feature and must be used
/// within a `#[tokio::test]` context.
///
/// # Example
///
/// ```ignore
/// use tui_dispatch::testing::{pause_time, advance_time};
/// use std::time::Duration;
///
/// #[tokio::test]
/// async fn test_debounce() {
///     pause_time();
///
///     // Simulate typing with debounce
///     harness.send_keys("a b c", handler);
///
///     // Advance past debounce threshold
///     advance_time(Duration::from_millis(300)).await;
///
///     let actions = harness.drain_emitted();
///     assert_emitted!(actions, Action::DebouncedSearch { .. });
/// }
/// ```
#[cfg(feature = "testing-time")]
pub fn pause_time() {
    tokio::time::pause();
}

/// Resume real-time execution after pausing.
#[cfg(feature = "testing-time")]
pub fn resume_time() {
    tokio::time::resume();
}

/// Advance the paused clock by the specified duration.
///
/// Must be called after [`pause_time`].
#[cfg(feature = "testing-time")]
pub async fn advance_time(duration: std::time::Duration) {
    tokio::time::advance(duration).await;
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

    // ActionAssertions trait tests
    #[test]
    fn test_action_assertions_first_last() {
        let actions = vec![TestAction::Foo, TestAction::Bar(42), TestAction::Bar(99)];

        actions.assert_first(TestAction::Foo);
        actions.assert_last(TestAction::Bar(99));
    }

    #[test]
    fn test_action_assertions_contains() {
        let actions = vec![TestAction::Foo, TestAction::Bar(42)];

        actions.assert_contains(TestAction::Foo);
        actions.assert_contains(TestAction::Bar(42));
        actions.assert_not_contains(TestAction::Bar(99));
    }

    #[test]
    fn test_action_assertions_empty() {
        let empty: Vec<TestAction> = vec![];
        let non_empty = vec![TestAction::Foo];

        empty.assert_empty();
        non_empty.assert_not_empty();
    }

    #[test]
    fn test_action_assertions_count() {
        let actions = vec![TestAction::Foo, TestAction::Bar(1), TestAction::Bar(2)];
        actions.assert_count(3);
    }

    #[test]
    fn test_action_assertions_matches() {
        let actions = vec![TestAction::Foo, TestAction::Bar(42), TestAction::Bar(99)];

        actions.assert_first_matches(|a| matches!(a, TestAction::Foo));
        actions.assert_any_matches(|a| matches!(a, TestAction::Bar(x) if *x > 50));
        actions.assert_all_match(|a| matches!(a, TestAction::Foo | TestAction::Bar(_)));
        actions.assert_none_match(|a| matches!(a, TestAction::Bar(0)));
    }

    // key_events / keys tests
    #[test]
    fn test_keys_multiple() {
        let k = keys("a b c");
        assert_eq!(k.len(), 3);
        assert_eq!(k[0].code, KeyCode::Char('a'));
        assert_eq!(k[1].code, KeyCode::Char('b'));
        assert_eq!(k[2].code, KeyCode::Char('c'));
    }

    #[test]
    fn test_keys_with_modifiers() {
        let k = keys("ctrl+c esc enter");
        assert_eq!(k.len(), 3);
        assert_eq!(k[0].code, KeyCode::Char('c'));
        assert!(k[0].modifiers.contains(KeyModifiers::CONTROL));
        assert_eq!(k[1].code, KeyCode::Esc);
        assert_eq!(k[2].code, KeyCode::Enter);
    }

    // RenderHarness tests
    #[test]
    fn test_render_harness_new() {
        let harness = RenderHarness::new(80, 24);
        assert_eq!(harness.size(), (80, 24));
    }

    #[test]
    fn test_render_harness_render_plain() {
        let mut harness = RenderHarness::new(10, 2);
        let output = harness.render_to_string_plain(|frame| {
            use ratatui::widgets::Paragraph;
            let p = Paragraph::new("Hello");
            frame.render_widget(p, frame.area());
        });

        // Should contain "Hello" followed by spaces to fill the width
        assert!(output.starts_with("Hello"));
    }

    #[test]
    fn test_render_harness_resize() {
        let mut harness = RenderHarness::new(80, 24);
        assert_eq!(harness.size(), (80, 24));

        harness.resize(100, 30);
        assert_eq!(harness.size(), (100, 30));
    }

    // complete_action tests
    #[test]
    fn test_complete_action() {
        let mut harness = TestHarness::<(), TestAction>::new(());

        harness.complete_action(TestAction::Foo);
        harness.complete_actions([TestAction::Bar(1), TestAction::Bar(2)]);

        let actions = harness.drain_emitted();
        assert_eq!(actions.len(), 3);
        actions.assert_first(TestAction::Foo);
    }

    // assert_state! macro test
    #[derive(Default, Debug, PartialEq)]
    struct TestState {
        count: i32,
        name: String,
    }

    #[test]
    fn test_assert_state_macro() {
        let harness = TestHarness::<TestState, TestAction>::new(TestState {
            count: 42,
            name: "test".to_string(),
        });

        assert_state!(harness, count, 42);
        assert_state!(harness, name, "test".to_string());
    }
}
