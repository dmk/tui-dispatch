//! Replay items for action replay with await markers.
//!
//! This module provides types for replay files that can include
//! `_await` markers to pause replay until async effects complete.
//!
//! # Example JSON
//! ```json
//! [
//!   {"SearchQuerySubmit": "Cabo Verde"},
//!   {"_await": "SearchDidLoad"},
//!   {"SearchSelect": 0},
//!   "SearchConfirm",
//!   {"_await_any": ["WeatherDidLoad", "WeatherDidError"]}
//! ]
//! ```

use serde::{Deserialize, Serialize};

#[cfg(feature = "json-schema")]
use schemars::JsonSchema;

/// A replay item: either an action or an await marker.
///
/// When deserializing, this uses untagged enum representation so that:
/// - `{"_await": "pattern"}` becomes `AwaitOne`
/// - `{"_await_any": [...]}` becomes `AwaitAny`
/// - Any other JSON becomes `Action(A)`
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[serde(untagged)]
pub enum ReplayItem<A> {
    /// Pause replay until an action matching the glob pattern is dispatched.
    ///
    /// Supports `*` as wildcard (e.g., `"*DidLoad"` matches `"WeatherDidLoad"`).
    #[cfg_attr(
        feature = "json-schema",
        schemars(description = "Pause until action matching pattern (supports * glob)")
    )]
    AwaitOne {
        /// Glob pattern to match against action names
        _await: String,
    },

    /// Pause replay until any of the patterns match.
    #[cfg_attr(
        feature = "json-schema",
        schemars(description = "Pause until any action matching one of the patterns")
    )]
    AwaitAny {
        /// List of glob patterns - replay continues when any matches
        _await_any: Vec<String>,
    },

    /// A regular action to dispatch.
    Action(A),
}

impl<A> ReplayItem<A> {
    /// Returns true if this is an await marker (not an action).
    pub fn is_await(&self) -> bool {
        matches!(
            self,
            ReplayItem::AwaitOne { .. } | ReplayItem::AwaitAny { .. }
        )
    }

    /// Returns the action if this is an Action variant.
    pub fn into_action(self) -> Option<A> {
        match self {
            ReplayItem::Action(a) => Some(a),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    enum TestAction {
        Fetch,
        DidLoad(String),
        Select(usize),
    }

    #[test]
    fn test_deserialize_action() {
        let json = r#""Fetch""#;
        let item: ReplayItem<TestAction> = serde_json::from_str(json).unwrap();
        assert!(matches!(item, ReplayItem::Action(TestAction::Fetch)));
    }

    #[test]
    fn test_deserialize_action_with_data() {
        let json = r#"{"DidLoad": "hello"}"#;
        let item: ReplayItem<TestAction> = serde_json::from_str(json).unwrap();
        assert!(matches!(
            item,
            ReplayItem::Action(TestAction::DidLoad(s)) if s == "hello"
        ));
    }

    #[test]
    fn test_deserialize_await_one() {
        let json = r#"{"_await": "DidLoad"}"#;
        let item: ReplayItem<TestAction> = serde_json::from_str(json).unwrap();
        assert!(matches!(
            item,
            ReplayItem::AwaitOne { _await } if _await == "DidLoad"
        ));
    }

    #[test]
    fn test_deserialize_await_any() {
        let json = r#"{"_await_any": ["DidLoad", "DidError"]}"#;
        let item: ReplayItem<TestAction> = serde_json::from_str(json).unwrap();
        match item {
            ReplayItem::AwaitAny { _await_any } => {
                assert_eq!(_await_any, vec!["DidLoad", "DidError"]);
            }
            _ => panic!("expected AwaitAny"),
        }
    }

    #[test]
    fn test_deserialize_mixed_array() {
        let json = r#"[
            "Fetch",
            {"_await": "*DidLoad"},
            {"Select": 0}
        ]"#;
        let items: Vec<ReplayItem<TestAction>> = serde_json::from_str(json).unwrap();
        assert_eq!(items.len(), 3);
        assert!(matches!(items[0], ReplayItem::Action(TestAction::Fetch)));
        assert!(matches!(items[1], ReplayItem::AwaitOne { .. }));
        assert!(matches!(
            items[2],
            ReplayItem::Action(TestAction::Select(0))
        ));
    }

    #[test]
    fn test_is_await() {
        let action: ReplayItem<TestAction> = ReplayItem::Action(TestAction::Fetch);
        let await_one: ReplayItem<TestAction> = ReplayItem::AwaitOne {
            _await: "test".into(),
        };
        let await_any: ReplayItem<TestAction> = ReplayItem::AwaitAny {
            _await_any: vec!["a".into()],
        };

        assert!(!action.is_await());
        assert!(await_one.is_await());
        assert!(await_any.is_await());
    }
}
