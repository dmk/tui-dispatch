//! Task manager for async operations
//!
//! Provides lifecycle management for async tasks with support for:
//! - Automatic cancellation when spawning with same key
//! - Debounced execution
//! - Manual cancellation
//!
//! # Example
//!
//! ```ignore
//! use tui_dispatch::tasks::{TaskManager, TaskKey};
//! use std::time::Duration;
//!
//! let (action_tx, mut action_rx) = tokio::sync::mpsc::unbounded_channel();
//! let mut tasks = TaskManager::new(action_tx);
//!
//! // Spawn a task - any existing task with same key is cancelled
//! tasks.spawn(TaskKey::new("fetch"), async {
//!     let data = fetch_data().await;
//!     Action::DidFetch(data)
//! });
//!
//! // Debounced task - waits before executing, resets on each call
//! tasks.debounce(TaskKey::new("search"), Duration::from_millis(200), async {
//!     let results = search(query).await;
//!     Action::DidSearch(results)
//! });
//!
//! // Cancel a specific task
//! tasks.cancel(&TaskKey::new("fetch"));
//!
//! // Cancel all tasks (e.g., on shutdown)
//! tasks.cancel_all();
//! ```

use std::collections::HashMap;
use std::future::Future;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::task::{AbortHandle, JoinHandle};

use crate::Action;

/// Identifies a task for cancellation and replacement.
///
/// Tasks with the same key are mutually exclusive - spawning a new task
/// with a key that's already running will cancel the existing task.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct TaskKey(String);

impl TaskKey {
    /// Create a new task key.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Get the key name.
    pub fn name(&self) -> &str {
        &self.0
    }
}

impl From<&'static str> for TaskKey {
    fn from(s: &'static str) -> Self {
        Self::new(s)
    }
}

impl From<String> for TaskKey {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Manages async task lifecycle with automatic cancellation.
///
/// The task manager maintains a registry of running tasks by key.
/// When a new task is spawned with a key that already exists,
/// the existing task is automatically cancelled before the new one starts.
///
/// # Type Parameters
///
/// - `A`: The action type that tasks produce
pub struct TaskManager<A> {
    tasks: HashMap<TaskKey, AbortHandle>,
    action_tx: mpsc::UnboundedSender<A>,
}

impl<A> TaskManager<A>
where
    A: Action,
{
    /// Create a new task manager.
    ///
    /// The `action_tx` channel is used to send actions back to the main loop
    /// when tasks complete.
    pub fn new(action_tx: mpsc::UnboundedSender<A>) -> Self {
        Self {
            tasks: HashMap::new(),
            action_tx,
        }
    }

    /// Spawn a task, cancelling any existing task with the same key.
    ///
    /// The future should return an action that will be sent to the action channel
    /// when the task completes. If the task is cancelled before completion,
    /// no action is sent.
    ///
    /// # Example
    ///
    /// ```ignore
    /// tasks.spawn(TaskKey::new("weather"), async move {
    ///     match api::fetch_weather(lat, lon).await {
    ///         Ok(data) => Action::WeatherDidLoad(data),
    ///         Err(e) => Action::WeatherDidError(e.to_string()),
    ///     }
    /// });
    /// ```
    pub fn spawn<F>(&mut self, key: impl Into<TaskKey>, future: F) -> &mut Self
    where
        F: Future<Output = A> + Send + 'static,
    {
        let key = key.into();

        // Cancel existing task with this key
        self.cancel(&key);

        let tx = self.action_tx.clone();
        let handle: JoinHandle<()> = tokio::spawn(async move {
            let action = future.await;
            let _ = tx.send(action);
        });

        self.tasks.insert(key, handle.abort_handle());
        self
    }

    /// Spawn a task with debounce - waits for duration before executing.
    ///
    /// If called again with the same key before the duration expires,
    /// the previous task is cancelled and the timer resets.
    ///
    /// Useful for search-as-you-type, auto-save, and similar patterns.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Only executes if no new input for 200ms
    /// tasks.debounce(TaskKey::new("search"), Duration::from_millis(200), async move {
    ///     let results = backend.search(&query).await;
    ///     Action::DidSearch(results)
    /// });
    /// ```
    pub fn debounce<F>(
        &mut self,
        key: impl Into<TaskKey>,
        duration: Duration,
        future: F,
    ) -> &mut Self
    where
        F: Future<Output = A> + Send + 'static,
    {
        let key = key.into();

        // Cancel existing task with this key
        self.cancel(&key);

        let tx = self.action_tx.clone();
        let handle: JoinHandle<()> = tokio::spawn(async move {
            tokio::time::sleep(duration).await;
            let action = future.await;
            let _ = tx.send(action);
        });

        self.tasks.insert(key, handle.abort_handle());
        self
    }

    /// Cancel a task by key.
    ///
    /// If no task exists with the given key, this is a no-op.
    pub fn cancel(&mut self, key: &TaskKey) {
        if let Some(handle) = self.tasks.remove(key) {
            handle.abort();
        }
    }

    /// Cancel all running tasks.
    ///
    /// Useful for cleanup on shutdown.
    pub fn cancel_all(&mut self) {
        for (_, handle) in self.tasks.drain() {
            handle.abort();
        }
    }

    /// Check if a task with the given key is currently running.
    pub fn is_running(&self, key: &TaskKey) -> bool {
        self.tasks.contains_key(key)
    }

    /// Get the number of running tasks.
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    /// Check if there are no running tasks.
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Get the keys of all running tasks.
    pub fn running_keys(&self) -> impl Iterator<Item = &TaskKey> {
        self.tasks.keys()
    }
}

impl<A> Drop for TaskManager<A> {
    fn drop(&mut self) {
        // Abort all running tasks on drop
        for (_, handle) in self.tasks.drain() {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[derive(Clone, Debug)]
    enum TestAction {
        Done(usize),
    }

    impl Action for TestAction {
        fn name(&self) -> &'static str {
            "Done"
        }
    }

    #[test]
    fn test_task_key() {
        let k1 = TaskKey::new("test");
        let k2 = TaskKey::from("test");
        let k3: TaskKey = "test".into();

        assert_eq!(k1, k2);
        assert_eq!(k2, k3);
        assert_eq!(k1.name(), "test");
    }

    #[tokio::test]
    async fn test_spawn_sends_action() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut tasks = TaskManager::new(tx);

        tasks.spawn("test", async { TestAction::Done(42) });

        let action = tokio::time::timeout(Duration::from_millis(100), rx.recv())
            .await
            .expect("timeout")
            .expect("channel closed");

        assert!(matches!(action, TestAction::Done(42)));
    }

    #[tokio::test]
    async fn test_spawn_cancels_previous() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut tasks = TaskManager::new(tx);

        let counter = Arc::new(AtomicUsize::new(0));

        // Spawn first task that takes a while
        let c1 = counter.clone();
        tasks.spawn("test", async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            c1.fetch_add(1, Ordering::SeqCst);
            TestAction::Done(1)
        });

        // Immediately spawn second task with same key
        let c2 = counter.clone();
        tasks.spawn("test", async move {
            c2.fetch_add(10, Ordering::SeqCst);
            TestAction::Done(2)
        });

        // Only second task should complete
        let action = tokio::time::timeout(Duration::from_millis(200), rx.recv())
            .await
            .expect("timeout")
            .expect("channel closed");

        assert!(matches!(action, TestAction::Done(2)));
        assert_eq!(counter.load(Ordering::SeqCst), 10);
    }

    #[tokio::test]
    async fn test_debounce() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut tasks = TaskManager::new(tx);

        tasks.debounce("test", Duration::from_millis(50), async {
            TestAction::Done(1)
        });

        // Should not receive yet
        let result = tokio::time::timeout(Duration::from_millis(30), rx.recv()).await;
        assert!(result.is_err());

        // Should receive after debounce period
        let action = tokio::time::timeout(Duration::from_millis(100), rx.recv())
            .await
            .expect("timeout")
            .expect("channel closed");

        assert!(matches!(action, TestAction::Done(1)));
    }

    #[tokio::test]
    async fn test_debounce_resets() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut tasks = TaskManager::new(tx);

        // First debounce
        tasks.debounce("test", Duration::from_millis(50), async {
            TestAction::Done(1)
        });

        // Wait a bit, then debounce again (should reset timer)
        tokio::time::sleep(Duration::from_millis(30)).await;
        tasks.debounce("test", Duration::from_millis(50), async {
            TestAction::Done(2)
        });

        // Should not receive first task
        let action = tokio::time::timeout(Duration::from_millis(100), rx.recv())
            .await
            .expect("timeout")
            .expect("channel closed");

        // Should only get second task
        assert!(matches!(action, TestAction::Done(2)));
    }

    #[tokio::test]
    async fn test_cancel() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut tasks = TaskManager::new(tx);

        tasks.spawn("test", async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            TestAction::Done(1)
        });

        assert!(tasks.is_running(&TaskKey::new("test")));

        tasks.cancel(&TaskKey::new("test"));

        assert!(!tasks.is_running(&TaskKey::new("test")));

        // Should not receive action
        let result = tokio::time::timeout(Duration::from_millis(150), rx.recv()).await;
        assert!(result.is_err() || result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_cancel_all() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut tasks = TaskManager::new(tx);

        tasks.spawn("a", async {
            tokio::time::sleep(Duration::from_secs(10)).await;
            TestAction::Done(1)
        });
        tasks.spawn("b", async {
            tokio::time::sleep(Duration::from_secs(10)).await;
            TestAction::Done(2)
        });

        assert_eq!(tasks.len(), 2);

        tasks.cancel_all();

        assert!(tasks.is_empty());
    }

    #[test]
    fn test_running_keys() {
        let (tx, _rx) = mpsc::unbounded_channel::<TestAction>();
        let tasks = TaskManager::new(tx);

        // Can't spawn without runtime, but can test the structure
        assert!(tasks.is_empty());
        assert_eq!(tasks.len(), 0);
    }
}
