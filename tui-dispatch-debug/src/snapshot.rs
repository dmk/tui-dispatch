use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub type SnapshotResult<T> = Result<T, SnapshotError>;

#[derive(Debug)]
pub enum SnapshotError {
    Io(std::io::Error),
    Ron(ron::Error),
    RonSpanned(ron::error::SpannedError),
}

impl From<std::io::Error> for SnapshotError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<ron::Error> for SnapshotError {
    fn from(error: ron::Error) -> Self {
        Self::Ron(error)
    }
}

impl From<ron::error::SpannedError> for SnapshotError {
    fn from(error: ron::error::SpannedError) -> Self {
        Self::RonSpanned(error)
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
    pub fn save_ron<P: AsRef<Path>>(&self, path: P) -> SnapshotResult<()> {
        save_ron(path, &self.state)
    }
}

impl<S> StateSnapshot<S>
where
    S: DeserializeOwned,
{
    pub fn load_ron<P: AsRef<Path>>(path: P) -> SnapshotResult<Self> {
        let state = load_ron(path)?;
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
    pub fn save_ron<P: AsRef<Path>>(&self, path: P) -> SnapshotResult<()> {
        save_ron(path, &self.actions)
    }
}

impl<A> ActionSnapshot<A>
where
    A: DeserializeOwned,
{
    pub fn load_ron<P: AsRef<Path>>(path: P) -> SnapshotResult<Self> {
        let actions = load_ron(path)?;
        Ok(Self { actions })
    }
}

pub fn load_ron<T, P>(path: P) -> SnapshotResult<T>
where
    T: DeserializeOwned,
    P: AsRef<Path>,
{
    let contents = fs::read_to_string(path)?;
    let value = ron::from_str(&contents)?;
    Ok(value)
}

pub fn save_ron<T, P>(path: P, value: &T) -> SnapshotResult<()>
where
    T: Serialize,
    P: AsRef<Path>,
{
    let pretty = ron::ser::PrettyConfig::default();
    let data = ron::ser::to_string_pretty(value, pretty)?;
    fs::write(path, data)?;
    Ok(())
}
