//! DataResource: typed async data lifecycle
//!
//! A type representing the lifecycle of async-loaded data. Use this instead of
//! scattering `loading: bool` and `error: Option<String>` across your state.
//!
//! # Example
//!
//! ```
//! use tui_dispatch_core::DataResource;
//!
//! // In state
//! struct AppState {
//!     weather: DataResource<WeatherData>,
//! }
//!
//! # #[derive(Clone)]
//! # struct WeatherData;
//!
//! // In reducer
//! # let mut state = AppState { weather: DataResource::Empty };
//! # enum Action { FetchWeather, WeatherDidLoad(WeatherData), WeatherDidFail(String) }
//! # let action = Action::FetchWeather;
//! match action {
//!     Action::FetchWeather => {
//!         state.weather = DataResource::Loading;
//!         // return effect to fetch
//!     }
//!     Action::WeatherDidLoad(data) => {
//!         state.weather = DataResource::Loaded(data);
//!     }
//!     Action::WeatherDidFail(err) => {
//!         state.weather = DataResource::Failed(err);
//!     }
//! }
//!
//! // In render
//! match &state.weather {
//!     DataResource::Empty => { /* show placeholder */ }
//!     DataResource::Loading => { /* show spinner */ }
//!     DataResource::Loaded(data) => { /* render data */ }
//!     DataResource::Failed(err) => { /* show error */ }
//! }
//! ```

use serde::{Deserialize, Serialize};

/// Represents the lifecycle of async-loaded data.
///
/// This type captures the four states data can be in:
/// - `Empty`: No data requested yet
/// - `Loading`: Data is being fetched
/// - `Loaded(T)`: Data successfully loaded
/// - `Failed(String)`: Loading failed with error message
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum DataResource<T> {
    /// No data requested yet
    #[default]
    Empty,
    /// Data is being fetched
    Loading,
    /// Data successfully loaded
    Loaded(T),
    /// Loading failed with error message
    Failed(String),
}

impl<T> DataResource<T> {
    /// Returns `true` if this is `Empty`.
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    /// Returns `true` if this is `Loading`.
    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }

    /// Returns `true` if this is `Loaded(_)`.
    pub fn is_loaded(&self) -> bool {
        matches!(self, Self::Loaded(_))
    }

    /// Returns `true` if this is `Failed(_)`.
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }

    /// Returns a reference to the loaded data, or `None` if not loaded.
    pub fn data(&self) -> Option<&T> {
        match self {
            Self::Loaded(t) => Some(t),
            _ => None,
        }
    }

    /// Returns a mutable reference to the loaded data, or `None` if not loaded.
    pub fn data_mut(&mut self) -> Option<&mut T> {
        match self {
            Self::Loaded(t) => Some(t),
            _ => None,
        }
    }

    /// Returns the error message if failed, or `None` otherwise.
    pub fn error(&self) -> Option<&str> {
        match self {
            Self::Failed(e) => Some(e),
            _ => None,
        }
    }

    /// Maps the loaded value using the provided function.
    ///
    /// If `Loaded(t)`, returns `Loaded(f(t))`. Otherwise returns the same state.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> DataResource<U> {
        match self {
            Self::Empty => DataResource::Empty,
            Self::Loading => DataResource::Loading,
            Self::Loaded(t) => DataResource::Loaded(f(t)),
            Self::Failed(e) => DataResource::Failed(e),
        }
    }

    /// Maps a reference to the loaded value.
    pub fn map_ref<U>(&self, f: impl FnOnce(&T) -> U) -> DataResource<U> {
        match self {
            Self::Empty => DataResource::Empty,
            Self::Loading => DataResource::Loading,
            Self::Loaded(t) => DataResource::Loaded(f(t)),
            Self::Failed(e) => DataResource::Failed(e.clone()),
        }
    }

    /// Applies a function that returns a `DataResource` to the loaded value.
    ///
    /// Useful for chaining dependent async operations.
    pub fn and_then<U>(self, f: impl FnOnce(T) -> DataResource<U>) -> DataResource<U> {
        match self {
            Self::Empty => DataResource::Empty,
            Self::Loading => DataResource::Loading,
            Self::Loaded(t) => f(t),
            Self::Failed(e) => DataResource::Failed(e),
        }
    }

    /// Returns the loaded value or a default.
    pub fn unwrap_or(self, default: T) -> T {
        match self {
            Self::Loaded(t) => t,
            _ => default,
        }
    }

    /// Returns the loaded value or computes a default.
    pub fn unwrap_or_else(self, f: impl FnOnce() -> T) -> T {
        match self {
            Self::Loaded(t) => t,
            _ => f(),
        }
    }

    /// Converts from `&DataResource<T>` to `DataResource<&T>`.
    pub fn as_ref(&self) -> DataResource<&T> {
        match self {
            Self::Empty => DataResource::Empty,
            Self::Loading => DataResource::Loading,
            Self::Loaded(t) => DataResource::Loaded(t),
            Self::Failed(e) => DataResource::Failed(e.clone()),
        }
    }

    /// Returns `true` if there's either data or an error (not empty or loading).
    pub fn is_settled(&self) -> bool {
        matches!(self, Self::Loaded(_) | Self::Failed(_))
    }

    /// Returns `true` if there's no data yet (empty or loading).
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Empty | Self::Loading)
    }

    /// Transitions to `Loading` state. Returns `true` if state actually changed.
    ///
    /// Useful in reducers to start a fetch:
    /// ```
    /// # use tui_dispatch_core::DataResource;
    /// # let mut resource: DataResource<String> = DataResource::Empty;
    /// if resource.start_loading() {
    ///     // dispatch effect to fetch
    /// }
    /// ```
    pub fn start_loading(&mut self) -> bool
    where
        T: Clone,
    {
        if self.is_loading() {
            false
        } else {
            *self = Self::Loading;
            true
        }
    }
}

impl<T: Clone> DataResource<T> {
    /// Returns a clone of the loaded data, or `None` if not loaded.
    pub fn cloned(&self) -> Option<T> {
        self.data().cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_empty() {
        let resource: DataResource<String> = DataResource::default();
        assert!(resource.is_empty());
    }

    #[test]
    fn test_state_checks() {
        let empty: DataResource<i32> = DataResource::Empty;
        let loading: DataResource<i32> = DataResource::Loading;
        let loaded: DataResource<i32> = DataResource::Loaded(42);
        let failed: DataResource<i32> = DataResource::Failed("oops".to_string());

        assert!(empty.is_empty());
        assert!(!empty.is_loading());
        assert!(empty.is_pending());
        assert!(!empty.is_settled());

        assert!(!loading.is_empty());
        assert!(loading.is_loading());
        assert!(loading.is_pending());
        assert!(!loading.is_settled());

        assert!(!loaded.is_empty());
        assert!(!loaded.is_loading());
        assert!(loaded.is_loaded());
        assert!(!loaded.is_pending());
        assert!(loaded.is_settled());

        assert!(!failed.is_empty());
        assert!(failed.is_failed());
        assert!(!failed.is_pending());
        assert!(failed.is_settled());
    }

    #[test]
    fn test_data_accessors() {
        let loaded: DataResource<i32> = DataResource::Loaded(42);
        let failed: DataResource<i32> = DataResource::Failed("error".to_string());

        assert_eq!(loaded.data(), Some(&42));
        assert_eq!(failed.data(), None);
        assert_eq!(failed.error(), Some("error"));
        assert_eq!(loaded.error(), None);
    }

    #[test]
    fn test_map() {
        let loaded: DataResource<i32> = DataResource::Loaded(21);
        let doubled = loaded.map(|x| x * 2);
        assert_eq!(doubled.data(), Some(&42));

        let loading: DataResource<i32> = DataResource::Loading;
        let still_loading: DataResource<i32> = loading.map(|x| x * 2);
        assert!(still_loading.is_loading());
    }

    #[test]
    fn test_and_then() {
        let loaded: DataResource<i32> = DataResource::Loaded(42);
        let chained = loaded.and_then(|x| DataResource::Loaded(x.to_string()));
        assert_eq!(chained.data(), Some(&"42".to_string()));

        let failed: DataResource<i32> = DataResource::Failed("err".to_string());
        let still_failed: DataResource<String> =
            failed.and_then(|x| DataResource::Loaded(x.to_string()));
        assert!(still_failed.is_failed());
    }

    #[test]
    fn test_unwrap_or() {
        let loaded: DataResource<i32> = DataResource::Loaded(42);
        let empty: DataResource<i32> = DataResource::Empty;

        assert_eq!(loaded.unwrap_or(0), 42);
        assert_eq!(empty.unwrap_or(0), 0);
    }

    #[test]
    fn test_start_loading() {
        let mut resource: DataResource<i32> = DataResource::Empty;
        assert!(resource.start_loading());
        assert!(resource.is_loading());

        // Already loading, should return false
        assert!(!resource.start_loading());
        assert!(resource.is_loading());
    }

    #[test]
    fn test_serialize_deserialize() {
        let loaded: DataResource<i32> = DataResource::Loaded(42);
        let json = serde_json::to_string(&loaded).unwrap();
        let restored: DataResource<i32> = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded, restored);

        let failed: DataResource<i32> = DataResource::Failed("oops".to_string());
        let json = serde_json::to_string(&failed).unwrap();
        let restored: DataResource<i32> = serde_json::from_str(&json).unwrap();
        assert_eq!(failed, restored);
    }
}
