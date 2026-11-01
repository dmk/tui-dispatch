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

use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::time::Duration;

use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tokio_stream::{Stream, StreamExt};

use crate::{Action, ActionSender};

/// Behavior when subscription delivery is paused.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PausePolicy {
    /// Consume and discard items while paused (the legacy behavior).
    #[default]
    Drop,
    /// Retain up to `capacity` source items, then stop polling the source.
    /// Capacity must be positive. An already assembled batch may also be in flight.
    Buffer { capacity: usize },
}

/// Maximum batch size and time from the first item to a partial flush.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BatchConfig {
    max_items: usize,
    max_delay: Duration,
}

impl BatchConfig {
    /// Configure batching. A zero delay flushes each available partial batch.
    ///
    /// # Panics
    /// Panics if `max_items` is zero.
    pub fn new(max_items: usize, max_delay: Duration) -> Self {
        assert!(max_items > 0, "batch size must be positive");
        Self {
            max_items,
            max_delay,
        }
    }
}

// Reserve capacity before taking the pause read lock, then commit synchronously.
// pause()/resume() take the watch write lock, so pause returning is a delivery
// boundary even on a multi-thread runtime. Never hold that lock across an await
// or user code. Watching changes also releases blocked Drop sends on pause.
async fn deliver<A>(
    tx: &ActionSender<A>,
    paused: &mut watch::Receiver<bool>,
    policy: PausePolicy,
    action: A,
) -> bool {
    // Keep legacy unbounded forwarding allocation-free. This read lock makes
    // the pause check and enqueue atomic with respect to pause().
    if let ActionSender::Unbounded(sender) = tx {
        let guard = paused.borrow();
        if !*guard {
            return sender.send(action).is_ok();
        }
        if policy == PausePolicy::Drop {
            return !sender.is_closed();
        }
    }
    loop {
        let is_paused = *paused.borrow_and_update();
        if is_paused && policy == PausePolicy::Drop {
            return true;
        }
        tokio::select! {
            biased;
            _ = tx.closed() => return false,
            changed = paused.changed() => {
                if changed.is_err() { return false; }
            }
            permit = tx.reserve(), if !is_paused => {
                let Ok(permit) = permit else { return false; };
                let guard = paused.borrow();
                if !*guard {
                    permit.send(action);
                    return true;
                }
            }
        }
    }
}

async fn forward_stream<S, A>(stream: S, tx: ActionSender<A>, mut paused: watch::Receiver<bool>)
where
    S: Stream<Item = A>,
{
    tokio::pin!(stream);
    tokio::select! {
        biased;
        _ = tx.closed() => {},
        _ = async {
            while let Some(action) = stream.next().await {
                if !deliver(&tx, &mut paused, PausePolicy::Drop, action).await { break; }
                tokio::task::consume_budget().await;
            }
        } => {},
    }
}

async fn forward_batched<S, A, F>(
    stream: S,
    tx: ActionSender<A>,
    mut paused: watch::Receiver<bool>,
    batch: BatchConfig,
    policy: PausePolicy,
    mut action_fn: F,
) where
    S: Stream,
    F: FnMut(Vec<S::Item>) -> A,
{
    tokio::pin!(stream);
    let mut items = VecDeque::new();
    let mut deadline = None;
    let mut eof = false;
    let forward = async {
        loop {
            let is_paused = *paused.borrow_and_update();
            if is_paused && policy == PausePolicy::Drop {
                items.clear();
                deadline = None;
            }
            if !is_paused
                && !items.is_empty()
                && (items.len() >= batch.max_items
                    || eof
                    || deadline.is_some_and(|at| tokio::time::Instant::now() >= at))
            {
                let count = items.len().min(batch.max_items);
                let action = action_fn(items.drain(..count).collect());
                if !deliver(&tx, &mut paused, policy, action).await {
                    break;
                }
                if items.is_empty() {
                    deadline = None;
                }
                continue;
            }
            if eof && items.is_empty() {
                break;
            }
            let can_poll = !eof
                && match (is_paused, policy) {
                    (true, PausePolicy::Buffer { capacity }) => items.len() < capacity,
                    _ => true,
                };
            tokio::select! {
                biased;
                changed = paused.changed() => {
                    if changed.is_err() { break; }
                }
                _ = async { tokio::time::sleep_until(deadline.unwrap()).await },
                    if !is_paused && deadline.is_some() => {}
                item = stream.next(), if can_poll => {
                    match item {
                        Some(item) => {
                            if !(*paused.borrow() && policy == PausePolicy::Drop) {
                                if deadline.is_none() {
                                    deadline = Some(tokio::time::Instant::now() + batch.max_delay);
                                }
                                items.push_back(item);
                            }
                        }
                        None => eof = true,
                    }
                    // Immediately-ready sources must still allow pause/cancellation
                    // to run, including Drop streams that never await channel capacity.
                    tokio::task::consume_budget().await;
                }
            }
        }
    };
    tokio::select! {
        biased;
        _ = tx.closed() => {},
        _ = forward => {},
    }
}

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

/// Handle for pausing/resuming Subscriptions.
///
/// This is a lightweight, cloneable handle that can be used to pause and resume
/// subscription emission without needing mutable access to the Subscriptions manager.
/// Useful for debug layer integration.
#[derive(Clone)]
pub struct SubPauseHandle {
    paused: watch::Sender<bool>,
}

impl SubPauseHandle {
    /// Pause all subscriptions.
    ///
    /// When paused, intervals drop ticks; streams follow their configured pause policy.
    pub fn pause(&self) {
        self.paused.send_replace(true);
    }

    /// Resume all subscriptions.
    pub fn resume(&self) {
        self.paused.send_replace(false);
    }

    /// Check if subscriptions are paused.
    pub fn is_paused(&self) -> bool {
        *self.paused.borrow()
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
/// Supports pause/resume for debug mode. Intervals and legacy streams drop
/// paused items; streams can opt into bounded, lossless buffering.
///
/// # Type Parameters
///
/// - `A`: The action type that subscriptions produce
pub struct Subscriptions<A> {
    pub(crate) spawner: crate::work::WorkSpawner,
    handles: HashMap<SubKey, JoinHandle<()>>,
    action_tx: ActionSender<A>,
    /// Synchronizes pause boundaries with delivery and wakes buffered streams.
    paused: watch::Sender<bool>,
}

impl<A> Subscriptions<A>
where
    A: Action,
{
    /// Create a new subscription manager.
    ///
    /// The `action_tx` channel is used to send actions back to the main loop.
    pub fn new(action_tx: mpsc::UnboundedSender<A>) -> Self {
        Self::with_channel(action_tx.into())
    }

    /// Create a manager with bounded or unbounded action delivery.
    /// Bounded channels stop polling sources while delivery waits for capacity.
    pub fn with_channel(action_tx: ActionSender<A>) -> Self {
        Self {
            handles: HashMap::new(),
            action_tx,
            spawner: crate::work::WorkSpawner::default(),
            paused: watch::channel(false).0,
        }
    }

    /// Pause all subscriptions.
    ///
    /// When paused, intervals drop ticks; streams follow their configured pause policy.
    pub fn pause(&self) {
        self.paused.send_replace(true);
    }

    /// Resume all subscriptions.
    pub fn resume(&self) {
        self.paused.send_replace(false);
    }

    /// Check if subscriptions are paused.
    pub fn is_paused(&self) -> bool {
        *self.paused.borrow()
    }

    /// Get a pause handle for this subscription manager.
    ///
    /// The handle can be used to pause/resume subscriptions from elsewhere
    /// (e.g., from a debug layer) without needing mutable access.
    pub fn pause_handle(&self) -> SubPauseHandle {
        SubPauseHandle {
            paused: self.paused.clone(),
        }
    }

    /// Remove finished subscriptions from the registry.
    ///
    /// This is called automatically when adding new subscriptions.
    /// You typically don't need to call this directly.
    pub fn cleanup(&mut self) {
        self.handles.retain(|_, handle| !handle.is_finished());
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

        // Clean up finished subscriptions
        self.cleanup();

        // Cancel existing subscription with this key
        self.cancel(&key);

        let tx = self.action_tx.clone();
        let mut paused = self.paused.subscribe();
        let handle = self.spawner.spawn(async move {
            let mut interval = tokio::time::interval(duration);
            // Skip the first immediate tick
            interval.tick().await;

            loop {
                interval.tick().await;
                // Skip if paused
                if *paused.borrow() {
                    continue;
                }
                let action = action_fn();
                if !deliver(&tx, &mut paused, PausePolicy::Drop, action).await {
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

        // Clean up finished subscriptions
        self.cleanup();

        // Cancel existing subscription with this key
        self.cancel(&key);

        let tx = self.action_tx.clone();
        let mut paused = self.paused.subscribe();
        let handle = self.spawner.spawn(async move {
            let mut interval = tokio::time::interval(duration);

            loop {
                interval.tick().await;
                // Skip if paused
                if *paused.borrow() {
                    continue;
                }
                let action = action_fn();
                if !deliver(&tx, &mut paused, PausePolicy::Drop, action).await {
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
        self.stream_with_policy(key, stream, PausePolicy::Drop)
    }

    /// Forward actions with an explicit pause policy.
    /// `Buffer` retains source items in order and stops polling when full.
    pub fn stream_with_policy<S>(
        &mut self,
        key: impl Into<SubKey>,
        stream: S,
        policy: PausePolicy,
    ) -> &mut Self
    where
        S: Stream<Item = A> + Send + 'static,
    {
        if policy == PausePolicy::Drop {
            let key = key.into();
            self.cleanup();
            self.cancel(&key);
            let handle = self.spawner.spawn(forward_stream(
                stream,
                self.action_tx.clone(),
                self.paused.subscribe(),
            ));
            self.handles.insert(key, handle);
            return self;
        }
        self.stream_batched(
            key,
            stream,
            BatchConfig::new(1, Duration::ZERO),
            policy,
            |mut items| items.pop().expect("nonempty batch"),
        )
    }

    /// Collect source items into actions, flushing on size, deadline, or EOF.
    ///
    /// The deadline starts at the first item and does not slide with later items.
    /// While paused, `Drop` discards items and partial batches; `Buffer` keeps
    /// polling until its source-item capacity is reached. No action is enqueued
    /// while paused, including when a bounded send was already waiting.
    /// Already queued actions remain available to the receiver.
    ///
    /// Memory is bounded by `max(policy capacity, batch size)` source items plus
    /// at most one in-flight batch/action and the output channel. A bounded output
    /// counts actions, not source items. Cancellation discards unsent work.
    ///
    /// # Panics
    /// Panics if the buffer capacity is zero.
    pub fn stream_batched<S, F>(
        &mut self,
        key: impl Into<SubKey>,
        stream: S,
        batch: BatchConfig,
        policy: PausePolicy,
        action_fn: F,
    ) -> &mut Self
    where
        S: Stream + Send + 'static,
        S::Item: Send,
        F: FnMut(Vec<S::Item>) -> A + Send + 'static,
    {
        if let PausePolicy::Buffer { capacity } = policy {
            assert!(capacity > 0, "pause buffer capacity must be positive");
        }
        let key = key.into();
        self.cleanup();
        self.cancel(&key);
        let tx = self.action_tx.clone();
        let paused = self.paused.subscribe();
        let handle = self.spawner.spawn(forward_batched(
            stream, tx, paused, batch, policy, action_fn,
        ));
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

        // Clean up finished subscriptions
        self.cleanup();

        // Cancel existing subscription with this key
        self.cancel(&key);

        let tx = self.action_tx.clone();
        let paused = self.paused.subscribe();
        let handle = self.spawner.spawn(async move {
            let stream = tokio::select! {
                biased;
                _ = tx.closed() => return,
                stream = stream_fn => stream,
            };
            forward_stream(stream, tx, paused).await;
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
    ///
    /// Returns `false` if the subscription has finished (e.g., stream ended) or was never started.
    pub fn is_active(&self, key: &SubKey) -> bool {
        self.handles
            .get(key)
            .map(|handle| !handle.is_finished())
            .unwrap_or(false)
    }

    /// Get the number of currently active subscriptions.
    ///
    /// This counts only subscriptions that haven't finished yet.
    pub fn len(&self) -> usize {
        self.handles
            .values()
            .filter(|handle| !handle.is_finished())
            .count()
    }

    /// Check if there are no active subscriptions.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the keys of all currently active subscriptions.
    ///
    /// Only includes subscriptions that haven't finished yet.
    pub fn active_keys(&self) -> impl Iterator<Item = &SubKey> {
        self.handles
            .iter()
            .filter(|(_, handle)| !handle.is_finished())
            .map(|(key, _)| key)
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

    #[test]
    fn test_pause_handle_basic() {
        let (tx, _rx) = mpsc::unbounded_channel::<TestAction>();
        let subs = Subscriptions::new(tx);
        let handle = subs.pause_handle();

        assert!(!handle.is_paused());

        handle.pause();
        assert!(handle.is_paused());

        handle.resume();
        assert!(!handle.is_paused());
    }

    #[tokio::test]
    async fn test_pause_suppresses_interval() {
        let (tx, mut rx) = mpsc::unbounded_channel::<TestAction>();
        let mut subs = Subscriptions::new(tx);
        let handle = subs.pause_handle();

        // Start interval
        subs.interval("tick", Duration::from_millis(10), || TestAction::Tick);

        // Wait for at least one tick
        let _ = tokio::time::timeout(Duration::from_millis(50), rx.recv()).await;

        // Pause
        handle.pause();

        // Allow any in-flight tick to arrive (one may have passed the check before pause)
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Clear any pending (including possible in-flight tick)
        while rx.try_recv().is_ok() {}

        // Wait and verify no new ticks come through
        let result = tokio::time::timeout(Duration::from_millis(50), rx.recv()).await;
        assert!(result.is_err(), "should timeout - subscription is paused");

        // Resume
        handle.resume();

        // Should receive ticks again
        let result = tokio::time::timeout(Duration::from_millis(50), rx.recv()).await;
        assert!(result.is_ok(), "should receive tick after resume");
    }

    #[test]
    fn test_pause_handle_clone() {
        let (tx, _rx) = mpsc::unbounded_channel::<TestAction>();
        let subs = Subscriptions::new(tx);
        let handle1 = subs.pause_handle();
        let handle2 = handle1.clone();

        // Both handles share the same state
        handle1.pause();
        assert!(handle2.is_paused());

        handle2.resume();
        assert!(!handle1.is_paused());
    }

    #[tokio::test]
    async fn test_finished_stream_cleaned_up() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut subs = Subscriptions::new(tx);

        // Create a stream that ends after 3 items
        let stream = tokio_stream::iter(vec![
            TestAction::Value(1),
            TestAction::Value(2),
            TestAction::Value(3),
        ]);

        subs.stream("finite", stream);

        // Wait for all items and stream to complete
        for _ in 0..3 {
            let _ = tokio::time::timeout(Duration::from_millis(100), rx.recv())
                .await
                .expect("timeout");
        }

        // Give the task time to finish
        tokio::time::sleep(Duration::from_millis(50)).await;

        // is_active should return false for finished stream
        assert!(!subs.is_active(&SubKey::new("finite")));

        // len() should not count finished streams
        assert_eq!(subs.len(), 0);
    }

    #[tokio::test]
    async fn test_is_active_accurate_for_running_interval() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut subs = Subscriptions::new(tx);

        subs.interval("tick", Duration::from_millis(20), || TestAction::Tick);

        // Should be active
        assert!(subs.is_active(&SubKey::new("tick")));
        assert_eq!(subs.len(), 1);

        // Wait for a tick to confirm it's running
        let _ = tokio::time::timeout(Duration::from_millis(100), rx.recv())
            .await
            .expect("timeout");

        // Still active (intervals run forever)
        assert!(subs.is_active(&SubKey::new("tick")));
        assert_eq!(subs.len(), 1);

        // Cancel it
        subs.cancel(&SubKey::new("tick"));

        // No longer active
        assert!(!subs.is_active(&SubKey::new("tick")));
        assert_eq!(subs.len(), 0);
    }
}
