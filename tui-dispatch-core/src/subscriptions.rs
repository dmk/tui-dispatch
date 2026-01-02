//! Declarative subscription system for continuous action sources
//!
//! Subscriptions provide a way to declare ongoing sources of actions
//! such as timers, intervals, and streams.
//!
//! # Example
//!
//! ```ignore
//! use tui_dispatch::subscriptions::Subscriptions;
//! use std::time::Duration;
//!
//! let (action_tx, mut action_rx) = tokio::sync::mpsc::unbounded_channel();
//! let mut subs = Subscriptions::new(action_tx);
//!
//! // Tick every 100ms for animations
//! subs.interval("tick", Duration::from_millis(100), || Action::Tick);
//!
//! // Auto-refresh every 5 minutes
//! subs.interval("refresh", Duration::from_secs(300), || Action::WeatherFetch);
//!
//! // Stream from external source
//! subs.stream("events", backend.event_stream());
//!
//! // Cancel all on shutdown
//! subs.cancel_all();
//! ```

use std::collections::HashMap;
use std::future::Future;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_stream::{Stream, StreamExt};

use crate::Action;

/// Identifies a subscription for cancellation.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct SubKey(String);

impl SubKey {
    /// Create a new subscription key.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Get the key name.
    pub fn name(&self) -> &str {
        &self.0
    }
}

impl From<&'static str> for SubKey {
    fn from(s: &'static str) -> Self {
        Self::new(s)
    }
}

impl From<String> for SubKey {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Manages declarative subscriptions that continuously emit actions.
///
/// Subscriptions are long-lived sources of actions, unlike one-shot tasks.
/// Common use cases:
/// - Tick timers for animations
/// - Periodic refresh intervals
/// - External event streams (websockets, file watchers, etc.)
///
/// # Type Parameters
///
/// - `A`: The action type that subscriptions produce
pub struct Subscriptions<A> {
    handles: HashMap<SubKey, JoinHandle<()>>,
    action_tx: mpsc::UnboundedSender<A>,
}

impl<A> Subscriptions<A>
where
    A: Action,
{
    /// Create a new subscription manager.
    ///
    /// The `action_tx` channel is used to send actions back to the main loop.
    pub fn new(action_tx: mpsc::UnboundedSender<A>) -> Self {
        Self {
            handles: HashMap::new(),
            action_tx,
        }
    }

    /// Add an interval subscription that emits an action at fixed intervals.
    ///
    /// The action factory is called each interval to produce the action.
    /// If a subscription with the same key exists, it is cancelled first.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Emit Tick every 100ms
    /// subs.interval("tick", Duration::from_millis(100), || Action::Tick);
    ///
    /// // Auto-refresh every 5 minutes
    /// subs.interval("refresh", Duration::from_secs(300), || Action::DataFetch);
    /// ```
    pub fn interval<F>(
        &mut self,
        key: impl Into<SubKey>,
        duration: Duration,
        action_fn: F,
    ) -> &mut Self
    where
        F: Fn() -> A + Send + 'static,
    {
        let key = key.into();

        // Cancel existing subscription with this key
        self.cancel(&key);

        let tx = self.action_tx.clone();
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(duration);
            // Skip the first immediate tick
            interval.tick().await;

            loop {
                interval.tick().await;
                let action = action_fn();
                if tx.send(action).is_err() {
                    // Channel closed, stop the subscription
                    break;
                }
            }
        });

        self.handles.insert(key, handle);
        self
    }

    /// Add an interval subscription that emits immediately, then at fixed intervals.
    ///
    /// Unlike `interval()`, this variant emits the first action immediately
    /// without waiting for the interval duration.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Emit immediately, then every 5 seconds
    /// subs.interval_immediate("poll", Duration::from_secs(5), || Action::Poll);
    /// ```
    pub fn interval_immediate<F>(
        &mut self,
        key: impl Into<SubKey>,
        duration: Duration,
        action_fn: F,
    ) -> &mut Self
    where
        F: Fn() -> A + Send + 'static,
    {
        let key = key.into();

        // Cancel existing subscription with this key
        self.cancel(&key);

        let tx = self.action_tx.clone();
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(duration);

            loop {
                interval.tick().await;
                let action = action_fn();
                if tx.send(action).is_err() {
                    // Channel closed, stop the subscription
                    break;
                }
            }
        });

        self.handles.insert(key, handle);
        self
    }

    /// Add a stream subscription that forwards stream items as actions.
    ///
    /// The stream is consumed and each item is sent as an action.
    /// If a subscription with the same key exists, it is cancelled first.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Forward websocket messages as actions
    /// subs.stream("ws", websocket.map(|msg| Action::WsMessage(msg)));
    ///
    /// // Forward file watcher events
    /// subs.stream("files", watcher.map(|e| Action::FileChanged(e.path)));
    /// ```
    pub fn stream<S>(&mut self, key: impl Into<SubKey>, stream: S) -> &mut Self
    where
        S: Stream<Item = A> + Send + 'static,
    {
        let key = key.into();

        // Cancel existing subscription with this key
        self.cancel(&key);

        let tx = self.action_tx.clone();
        let handle = tokio::spawn(async move {
            tokio::pin!(stream);
            while let Some(action) = stream.next().await {
                if tx.send(action).is_err() {
                    // Channel closed, stop the subscription
                    break;
                }
            }
        });

        self.handles.insert(key, handle);
        self
    }

    /// Add a subscription from an async function that returns a stream.
    ///
    /// This is useful when stream creation itself is async (e.g., connecting to a service).
    ///
    /// # Example
    ///
    /// ```ignore
    /// subs.stream_async("redis", async {
    ///     let client = redis::connect().await?;
    ///     Ok(client.subscribe("events").map(|e| Action::RedisEvent(e)))
    /// });
    /// ```
    pub fn stream_async<F, S>(&mut self, key: impl Into<SubKey>, stream_fn: F) -> &mut Self
    where
        F: Future<Output = S> + Send + 'static,
        S: Stream<Item = A> + Send + 'static,
    {
        let key = key.into();

        // Cancel existing subscription with this key
        self.cancel(&key);

        let tx = self.action_tx.clone();
        let handle = tokio::spawn(async move {
            let stream = stream_fn.await;
            tokio::pin!(stream);
            while let Some(action) = stream.next().await {
                if tx.send(action).is_err() {
                    break;
                }
            }
        });

        self.handles.insert(key, handle);
        self
    }

    /// Cancel a subscription by key.
    ///
    /// If no subscription exists with the given key, this is a no-op.
    pub fn cancel(&mut self, key: &SubKey) {
        if let Some(handle) = self.handles.remove(key) {
            handle.abort();
        }
    }

    /// Cancel all subscriptions.
    ///
    /// Useful for cleanup on shutdown.
    pub fn cancel_all(&mut self) {
        for (_, handle) in self.handles.drain() {
            handle.abort();
        }
    }

    /// Check if a subscription with the given key is active.
    pub fn is_active(&self, key: &SubKey) -> bool {
        self.handles.contains_key(key)
    }

    /// Get the number of active subscriptions.
    pub fn len(&self) -> usize {
        self.handles.len()
    }

    /// Check if there are no active subscriptions.
    pub fn is_empty(&self) -> bool {
        self.handles.is_empty()
    }

    /// Get the keys of all active subscriptions.
    pub fn active_keys(&self) -> impl Iterator<Item = &SubKey> {
        self.handles.keys()
    }
}

impl<A> Drop for Subscriptions<A> {
    fn drop(&mut self) {
        // Abort all subscriptions on drop
        for (_, handle) in self.handles.drain() {
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
        Tick,
        Value(usize),
    }

    impl Action for TestAction {
        fn name(&self) -> &'static str {
            match self {
                TestAction::Tick => "Tick",
                TestAction::Value(_) => "Value",
            }
        }
    }

    #[test]
    fn test_sub_key() {
        let k1 = SubKey::new("test");
        let k2 = SubKey::from("test");
        let k3: SubKey = "test".into();

        assert_eq!(k1, k2);
        assert_eq!(k2, k3);
        assert_eq!(k1.name(), "test");
    }

    #[tokio::test]
    async fn test_interval_emits_actions() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut subs = Subscriptions::new(tx);

        subs.interval("tick", Duration::from_millis(20), || TestAction::Tick);

        // Wait for at least one tick
        let action = tokio::time::timeout(Duration::from_millis(100), rx.recv())
            .await
            .expect("timeout")
            .expect("channel closed");

        assert!(matches!(action, TestAction::Tick));

        // Should get more ticks
        let action2 = tokio::time::timeout(Duration::from_millis(50), rx.recv())
            .await
            .expect("timeout")
            .expect("channel closed");

        assert!(matches!(action2, TestAction::Tick));
    }

    #[tokio::test]
    async fn test_interval_immediate() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut subs = Subscriptions::new(tx);

        subs.interval_immediate("tick", Duration::from_millis(100), || TestAction::Tick);

        // Should receive immediately (well before the 100ms interval)
        let action = tokio::time::timeout(Duration::from_millis(20), rx.recv())
            .await
            .expect("should receive immediately")
            .expect("channel closed");

        assert!(matches!(action, TestAction::Tick));
    }

    #[tokio::test]
    async fn test_stream_forwards_items() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut subs = Subscriptions::new(tx);

        let stream = tokio_stream::iter(vec![
            TestAction::Value(1),
            TestAction::Value(2),
            TestAction::Value(3),
        ]);

        subs.stream("test", stream);

        // Collect all items
        let mut values = vec![];
        for _ in 0..3 {
            let action = tokio::time::timeout(Duration::from_millis(100), rx.recv())
                .await
                .expect("timeout")
                .expect("channel closed");
            if let TestAction::Value(v) = action {
                values.push(v);
            }
        }

        assert_eq!(values, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn test_cancel_stops_subscription() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut subs = Subscriptions::new(tx);

        subs.interval("tick", Duration::from_millis(10), || TestAction::Tick);

        assert!(subs.is_active(&SubKey::new("tick")));

        // Wait for at least one tick
        let _ = tokio::time::timeout(Duration::from_millis(50), rx.recv()).await;

        subs.cancel(&SubKey::new("tick"));

        assert!(!subs.is_active(&SubKey::new("tick")));

        // Clear any pending
        while rx.try_recv().is_ok() {}

        // Should not receive more after cancel
        let result = tokio::time::timeout(Duration::from_millis(50), rx.recv()).await;
        assert!(result.is_err(), "should timeout - no more ticks");
    }

    #[tokio::test]
    async fn test_cancel_all() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut subs = Subscriptions::new(tx);

        subs.interval("a", Duration::from_secs(10), || TestAction::Tick);
        subs.interval("b", Duration::from_secs(10), || TestAction::Tick);

        assert_eq!(subs.len(), 2);

        subs.cancel_all();

        assert!(subs.is_empty());
    }

    #[tokio::test]
    async fn test_replace_existing_subscription() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut subs = Subscriptions::new(tx);

        let counter = Arc::new(AtomicUsize::new(0));

        // First subscription
        let c1 = counter.clone();
        subs.interval("test", Duration::from_millis(10), move || {
            c1.fetch_add(1, Ordering::SeqCst);
            TestAction::Value(1)
        });

        // Replace with second subscription
        let c2 = counter.clone();
        subs.interval("test", Duration::from_millis(10), move || {
            c2.fetch_add(100, Ordering::SeqCst);
            TestAction::Value(2)
        });

        // Should only have one subscription
        assert_eq!(subs.len(), 1);

        // Wait a bit and check we only get Value(2)
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Drain and check values
        let mut got_two = false;
        while let Ok(action) = rx.try_recv() {
            if let TestAction::Value(v) = action {
                // Should only get 2s from second subscription
                assert_eq!(v, 2);
                got_two = true;
            }
        }

        assert!(got_two, "should have received Value(2)");
    }

    #[test]
    fn test_active_keys() {
        let (tx, _rx) = mpsc::unbounded_channel::<TestAction>();
        let subs = Subscriptions::new(tx);

        assert!(subs.is_empty());
        assert_eq!(subs.len(), 0);
    }
}
