use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub type SnapshotResult<T> = Result<T, SnapshotError>;

#[derive(Debug)]
pub enum SnapshotError {
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl From<std::io::Error> for SnapshotError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for SnapshotError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StateSnapshot<S> {
    pub state: S,
}

impl<S> StateSnapshot<S> {
    pub fn new(state: S) -> Self {
        Self { state }
    }

    pub fn state(&self) -> &S {
        &self.state
    }

    pub fn into_state(self) -> S {
        self.state
    }
}

impl<S> StateSnapshot<S>
where
    S: Serialize,
{
    pub fn save_json<P: AsRef<Path>>(&self, path: P) -> SnapshotResult<()> {
        save_json(path, &self.state)
    }
}

impl<S> StateSnapshot<S>
where
    S: DeserializeOwned,
{
    pub fn load_json<P: AsRef<Path>>(path: P) -> SnapshotResult<Self> {
        let state = load_json(path)?;
        Ok(Self { state })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ActionSnapshot<A> {
    pub actions: Vec<A>,
}

impl<A> ActionSnapshot<A> {
    pub fn new(actions: Vec<A>) -> Self {
        Self { actions }
    }

    pub fn actions(&self) -> &[A] {
        &self.actions
    }

    pub fn into_actions(self) -> Vec<A> {
        self.actions
    }
}

impl<A> ActionSnapshot<A>
where
    A: Serialize,
{
    pub fn save_json<P: AsRef<Path>>(&self, path: P) -> SnapshotResult<()> {
        save_json(path, &self.actions)
    }
}

impl<A> ActionSnapshot<A>
where
    A: DeserializeOwned,
{
    pub fn load_json<P: AsRef<Path>>(path: P) -> SnapshotResult<Self> {
        let actions = load_json(path)?;
        Ok(Self { actions })
    }
}

pub fn load_json<T, P>(path: P) -> SnapshotResult<T>
where
    T: DeserializeOwned,
    P: AsRef<Path>,
{
    let contents = fs::read_to_string(path)?;
    let value = serde_json::from_str(&contents)?;
    Ok(value)
}

pub fn save_json<T, P>(path: P, value: &T) -> SnapshotResult<()>
where
    T: Serialize,
    P: AsRef<Path>,
{
    let data = serde_json::to_string_pretty(value)?;
    fs::write(path, data)?;
    Ok(())
}

// JSON Schema generation (requires "json-schema" feature)

#[cfg(feature = "json-schema")]
pub use schemars::JsonSchema;

/// Generate a JSON schema for type T.
#[cfg(feature = "json-schema")]
pub fn generate_schema<T: schemars::JsonSchema>() -> schemars::schema::RootSchema {
    schemars::schema_for!(T)
}

/// Generate a JSON schema as a pretty-printed JSON string.
#[cfg(feature = "json-schema")]
pub fn schema_json<T: schemars::JsonSchema>() -> String {
    let schema = generate_schema::<T>();
    serde_json::to_string_pretty(&schema).unwrap_or_else(|_| "{}".to_string())
}

/// Save a JSON schema to a file.
#[cfg(feature = "json-schema")]
pub fn save_schema<T, P>(path: P) -> SnapshotResult<()>
where
    T: schemars::JsonSchema,
    P: AsRef<Path>,
{
    let json = schema_json::<T>();
    fs::write(path, json)?;
    Ok(())
}

/// Generate and save a replay schema for `Vec<ReplayItem<A>>`.
///
/// This includes:
/// - The full schema for replay items (actions + await markers)
/// - An `awaitable_actions` list extracted from Did* action names
#[cfg(feature = "json-schema")]
pub fn save_replay_schema<A, P>(path: P) -> SnapshotResult<()>
where
    A: schemars::JsonSchema,
    P: AsRef<Path>,
{
    use crate::replay::ReplayItem;

    // Generate schema for Vec<ReplayItem<A>>
    let mut schema = schemars::schema_for!(Vec<ReplayItem<A>>);

    // Extract Did* action names from the schema definitions
    let awaitable = extract_awaitable_actions(&schema);

    // Add awaitable_actions to the schema's extensions
    if let Some(metadata) = schema.schema.metadata.as_mut() {
        metadata.description = Some(
            "Replay items: actions and await markers for async coordination.\n\n\
             Use `_await` or `_await_any` to pause replay until async effects complete.\n\
             Only actions listed in `awaitable_actions` should be awaited (Did* pattern)."
                .to_string(),
        );
    }

    // Add awaitable_actions as a custom extension
    schema.schema.extensions.insert(
        "awaitable_actions".to_string(),
        serde_json::Value::Array(
            awaitable
                .into_iter()
                .map(serde_json::Value::String)
                .collect(),
        ),
    );

    let json = serde_json::to_string_pretty(&schema).unwrap_or_else(|_| "{}".to_string());
    fs::write(path, json)?;
    Ok(())
}

/// Extract action names containing "Did" from a schema's definitions.
#[cfg(feature = "json-schema")]
fn extract_awaitable_actions(schema: &schemars::schema::RootSchema) -> Vec<String> {
    let mut awaitable = Vec::new();

    // Look through definitions for action enum variants
    for (name, def) in &schema.definitions {
        // Skip non-action definitions
        if !name.ends_with("Action") && name != "Action" {
            continue;
        }

        // Extract enum variant names from oneOf
        if let Some(subschemas) = def.clone().into_object().subschemas {
            if let Some(one_of) = subschemas.one_of {
                for variant_schema in one_of {
                    extract_did_variants(&variant_schema.into_object(), &mut awaitable);
                }
            }
        }
    }

    awaitable.sort();
    awaitable.dedup();
    awaitable
}

#[cfg(feature = "json-schema")]
fn extract_did_variants(schema: &schemars::schema::SchemaObject, awaitable: &mut Vec<String>) {
    // Check for enum values (unit variants like "WeatherDidLoad")
    if let Some(enum_values) = &schema.enum_values {
        for val in enum_values {
            if let Some(name) = val.as_str() {
                if name.contains("Did") {
                    awaitable.push(name.to_string());
                }
            }
        }
    }

    // Check for object properties (struct variants like {"WeatherDidLoad": data})
    if let Some(obj) = &schema.object {
        for prop_name in obj.properties.keys() {
            if prop_name.contains("Did") {
                awaitable.push(prop_name.clone());
            }
        }
        for prop_name in obj.required.iter() {
            if prop_name.contains("Did") && !awaitable.contains(prop_name) {
                awaitable.push(prop_name.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(label: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("tui-dispatch-debug-{label}-{nanos}.json"));
        path
    }

    #[test]
    fn test_state_snapshot_round_trip() {
        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
        struct TestState {
            name: String,
            count: usize,
            flags: Vec<bool>,
        }

        let state = TestState {
            name: "alpha".to_string(),
            count: 42,
            flags: vec![true, false, true],
        };

        let path = temp_path("state");
        StateSnapshot::new(state.clone())
            .save_json(&path)
            .expect("save state snapshot");

        let loaded = StateSnapshot::<TestState>::load_json(&path)
            .expect("load state snapshot")
            .into_state();

        assert_eq!(loaded, state);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_action_snapshot_round_trip() {
        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
        enum TestAction {
            Tick,
            Set { value: i32 },
        }

        let actions = vec![TestAction::Tick, TestAction::Set { value: 7 }];
        let path = temp_path("actions");

        ActionSnapshot::new(actions.clone())
            .save_json(&path)
            .expect("save action snapshot");

        let loaded = ActionSnapshot::<TestAction>::load_json(&path)
            .expect("load action snapshot")
            .into_actions();

        assert_eq!(loaded, actions);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_load_json_missing_file() {
        let path = temp_path("missing");
        let _ = std::fs::remove_file(&path);

        match load_json::<u32, _>(&path) {
            Err(SnapshotError::Io(_)) => {}
            other => panic!("expected io error, got {other:?}"),
        }
    }
}
