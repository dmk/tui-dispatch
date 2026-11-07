//! JSON state differences used by the debugger's selected-action details.
//!
//! The interactive view is [`crate::debug::DebugInspector`]. Differences are
//! computed on demand, never during collection or for every history entry.
use serde::Serialize;
use serde_json::Value;

/// One added, removed, or replaced JSON value. Missing differs from JSON null.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StateChange {
    /// RFC 6901 JSON pointer; the empty string denotes the root value.
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<Value>,
}

/// Compare objects by key and arrays by index (without inferring moves).
/// Added/removed containers are reported as a single subtree. Results are sorted
/// by pointer. Only changed values are cloned; an explicit stack avoids recursive
/// traversal. This is an inspection diff, not an ordered JSON Patch program.
pub fn state_diff(before: &Value, after: &Value) -> Vec<StateChange> {
    fn child(path: &str, key: &str) -> String {
        format!("{path}/{}", key.replace('~', "~0").replace('/', "~1"))
    }
    let mut pending = vec![(String::new(), Some(before), Some(after))];
    let mut changes = Vec::new();
    while let Some((path, before, after)) = pending.pop() {
        match (before, after) {
            (Some(Value::Object(a)), Some(Value::Object(b))) => {
                for (key, value) in a {
                    pending.push((child(&path, key), Some(value), b.get(key)));
                }
                for (key, value) in b {
                    if !a.contains_key(key) {
                        pending.push((child(&path, key), None, Some(value)));
                    }
                }
            }
            (Some(Value::Array(a)), Some(Value::Array(b))) => {
                for index in 0..a.len().max(b.len()) {
                    pending.push((format!("{path}/{index}"), a.get(index), b.get(index)));
                }
            }
            (before, after) if before != after => changes.push(StateChange {
                path,
                before: before.cloned(),
                after: after.cloned(),
            }),
            _ => {}
        }
    }
    changes.sort_unstable_by(|a, b| a.path.cmp(&b.path));
    changes
}
