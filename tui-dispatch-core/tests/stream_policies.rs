#![cfg(feature = "subscriptions")]
use std::{
    future::Future,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    task::{Context, Poll, Waker},
    time::Duration,
};

#[cfg(feature = "testing-time")]
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_stream::StreamExt;
use tui_dispatch_core::{ActionChannel, BatchConfig, PausePolicy, Subscriptions};

#[derive(Clone, Debug, PartialEq, Eq)]
enum BatchAction {
    Batch(Vec<i32>),
    Single(i32),
}

#[tokio::test]
async fn bounded_delivery_waits_until_capacity_is_available() {
    let (tx, mut rx) = ActionChannel::bounded(1);
    tx.send(BatchAction::Single(1)).await.unwrap();
    let mut second = Box::pin(tx.send(BatchAction::Single(2)));
    let mut cx = Context::from_waker(Waker::noop());
    assert!(matches!(second.as_mut().poll(&mut cx), Poll::Pending));
    assert_eq!(rx.recv().await, Some(BatchAction::Single(1)));
    second.await.unwrap();
    assert_eq!(rx.recv().await, Some(BatchAction::Single(2)));
}

#[cfg(feature = "testing-time")]
#[tokio::test(start_paused = true)]
async fn buffering_pause_preserves_order_and_flushes_the_partial_batch() {
    let (tx, mut rx) = ActionChannel::bounded(2);
    let mut subscriptions = Subscriptions::with_channel(tx);
    subscriptions.pause();
    subscriptions.stream_batched(
        "numbers",
        tokio_stream::iter([1, 2, 3, 4, 5]),
        BatchConfig::new(3, Duration::from_millis(50)),
        PausePolicy::Buffer { capacity: 6 },
        BatchAction::Batch,
    );
    tokio::task::yield_now().await;
    assert!(rx.try_recv().is_err());
    subscriptions.resume();
    assert_eq!(rx.recv().await, Some(BatchAction::Batch(vec![1, 2, 3])));
    assert_eq!(rx.recv().await, Some(BatchAction::Batch(vec![4, 5])));
    assert!(rx.try_recv().is_err());
}

#[cfg(feature = "testing-time")]
#[tokio::test(start_paused = true)]
async fn full_pause_buffer_backpressures_the_source_without_losing_items() {
    const CAPACITY: usize = 3;
    const ITEMS: usize = 12;
    let consumed = Arc::new(AtomicUsize::new(0));
    let observed = consumed.clone();
    let source = tokio_stream::iter(0..ITEMS as i32).map(move |item| {
        observed.fetch_add(1, Ordering::SeqCst);
        item
    });
    let (tx, mut rx) = ActionChannel::bounded(1);
    let mut subscriptions = Subscriptions::with_channel(tx);
    subscriptions.pause();
    subscriptions.stream_batched(
        "numbers",
        source,
        BatchConfig::new(1, Duration::from_secs(60)),
        PausePolicy::Buffer { capacity: CAPACITY },
        BatchAction::Batch,
    );
    // On this paused, current-thread runtime, auto-advance happens once runnable
    // work parks. The source is immediately ready, so it must reach backpressure.
    tokio::time::sleep(Duration::from_millis(1)).await;
    let pulled = consumed.load(Ordering::SeqCst);
    assert!(
        (CAPACITY..=CAPACITY + 1).contains(&pulled),
        "buffer must fill and stop polling, allowing at most one in-flight item: {pulled}",
    );
    assert!(rx.try_recv().is_err());
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert_eq!(consumed.load(Ordering::SeqCst), pulled);
    subscriptions.resume();
    tokio::time::timeout(Duration::from_secs(1), async {
        for item in 0..ITEMS as i32 {
            assert_eq!(rx.recv().await, Some(BatchAction::Batch(vec![item])));
        }
    })
    .await
    .expect("resuming must release backpressure and deliver every buffered/source item");
    assert_eq!(consumed.load(Ordering::SeqCst), ITEMS);
    assert!(rx.try_recv().is_err());
}

#[cfg(feature = "testing-time")]
#[tokio::test(start_paused = true)]
async fn partial_batch_flushes_at_its_deadline_while_the_source_stays_open() {
    let (source_tx, source_rx) = tokio::sync::mpsc::unbounded_channel();
    let (consumed_tx, mut consumed_rx) = tokio::sync::mpsc::unbounded_channel();
    let (flush_tx, mut flush_rx) = tokio::sync::mpsc::unbounded_channel();
    let (tx, mut rx) = ActionChannel::bounded(2);
    let mut subscriptions = Subscriptions::with_channel(tx);
    let started = tokio::time::Instant::now();
    source_tx.send(1).unwrap();
    source_tx.send(2).unwrap();
    subscriptions.stream_batched(
        "numbers",
        UnboundedReceiverStream::new(source_rx).map(move |item| {
            consumed_tx.send(item).unwrap();
            item
        }),
        BatchConfig::new(3, Duration::from_millis(50)),
        PausePolicy::Buffer { capacity: 6 },
        move |items| {
            flush_tx.send(tokio::time::Instant::now()).unwrap();
            BatchAction::Batch(items)
        },
    );
    // Synchronize with input consumption before advancing the clock. The open
    // source then parks, so neither batch size nor EOF can trigger a flush.
    assert_eq!(consumed_rx.recv().await, Some(1));
    assert_eq!(consumed_rx.recv().await, Some(2));
    assert!(rx.try_recv().is_err());
    tokio::time::advance(Duration::from_millis(49)).await;
    assert!(
        rx.try_recv().is_err(),
        "partial batch must wait until 50 ms"
    );
    // Allow scheduler ordering at the deadline, then check the actual flush time.
    let batch = tokio::time::timeout(Duration::from_millis(2), rx.recv())
        .await
        .expect("partial batch must flush at 50 ms, without source EOF");
    assert_eq!(batch, Some(BatchAction::Batch(vec![1, 2])));
    let flushed_at = flush_rx.try_recv().unwrap() - started;
    assert!(
        (Duration::from_millis(50)..=Duration::from_millis(51)).contains(&flushed_at),
        "flush must respect the deadline (allowing Tokio's millisecond timer resolution): {flushed_at:?}",
    );
    assert!(!source_tx.is_closed(), "source must still be live");
    assert!(rx.try_recv().is_err());
    subscriptions.cancel_all();
}

impl tui_dispatch_core::Action for BatchAction {
    fn name(&self) -> &'static str {
        "BatchAction"
    }
}

async fn wait_for(mut predicate: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while !predicate() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("subscription did not settle");
}

#[tokio::test]
async fn pause_during_a_blocked_send_holds_or_discards_the_pending_action() {
    for policy in [PausePolicy::Drop, PausePolicy::Buffer { capacity: 3 }] {
        let (tx, mut rx) = ActionChannel::bounded(1);
        tx.send(BatchAction::Single(-1)).await.unwrap();
        let mut subs = Subscriptions::with_channel(tx);
        let (seen_tx, mut seen_rx) = tokio::sync::mpsc::unbounded_channel();
        subs.stream_with_policy(
            "source",
            tokio_stream::iter([BatchAction::Single(1)]).map(move |item| {
                seen_tx.send(()).unwrap();
                item
            }),
            policy,
        );
        seen_rx.recv().await.unwrap();
        let handle = subs.pause_handle();
        handle.pause();
        assert_eq!(rx.recv().await, Some(BatchAction::Single(-1)));
        // Release channel capacity while the producer is paused.
        for _ in 0..5 {
            tokio::task::yield_now().await;
        }
        assert!(rx.try_recv().is_err());
        handle.resume();
        wait_for(|| subs.is_empty()).await;
        match policy {
            PausePolicy::Buffer { .. } => {
                assert_eq!(rx.try_recv().unwrap(), BatchAction::Single(1))
            }
            PausePolicy::Drop => assert!(rx.try_recv().is_err()),
        }
        assert!(rx.try_recv().is_err());
    }
}

#[tokio::test]
async fn buffer_smaller_than_batch_resumes_without_deadlock_and_preserves_order() {
    let (tx, mut rx) = ActionChannel::bounded(1);
    let mut subs = Subscriptions::with_channel(tx);
    let consumed = Arc::new(AtomicUsize::new(0));
    let observed = consumed.clone();
    subs.pause();
    subs.stream_batched(
        "source",
        tokio_stream::iter(0..9).map(move |item| {
            observed.fetch_add(1, Ordering::SeqCst);
            item
        }),
        BatchConfig::new(4, Duration::from_secs(60)),
        PausePolicy::Buffer { capacity: 2 },
        BatchAction::Batch,
    );
    wait_for(|| consumed.load(Ordering::SeqCst) == 2).await;
    assert!(rx.try_recv().is_err());
    subs.resume();
    for expected in [vec![0, 1, 2, 3], vec![4, 5, 6, 7], vec![8]] {
        assert_eq!(rx.recv().await, Some(BatchAction::Batch(expected)));
    }
    wait_for(|| subs.is_empty()).await;
    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn saturated_channel_stops_source_polling() {
    let (tx, mut rx) = ActionChannel::bounded(2);
    let mut subs = Subscriptions::with_channel(tx);
    let consumed = Arc::new(AtomicUsize::new(0));
    let observed = consumed.clone();
    subs.stream_batched(
        "source",
        tokio_stream::iter(0..100).map(move |item| {
            observed.fetch_add(1, Ordering::SeqCst);
            item
        }),
        BatchConfig::new(4, Duration::from_secs(60)),
        PausePolicy::Buffer { capacity: 5 },
        BatchAction::Batch,
    );
    wait_for(|| consumed.load(Ordering::SeqCst) == 12).await;
    for _ in 0..5 {
        tokio::task::yield_now().await;
    }
    assert_eq!(
        consumed.load(Ordering::SeqCst),
        12,
        "two queued batches plus one in flight"
    );
    for start in (0..100).step_by(4) {
        assert_eq!(
            rx.recv().await,
            Some(BatchAction::Batch((start..start + 4).collect()))
        );
    }
}

#[tokio::test]
async fn closure_and_cancellation_release_paused_items_and_source() {
    // Test receiver close, cancellation, replacement, and manager drop separately.
    for mode in 0..4 {
        let (tx, mut rx) = ActionChannel::bounded(1);
        let mut subs = Subscriptions::with_channel(tx);
        let owner = Arc::new(());
        let weak = Arc::downgrade(&owner);
        let consumed = Arc::new(AtomicUsize::new(0));
        let observed = consumed.clone();
        subs.pause();
        subs.stream_batched(
            "source",
            tokio_stream::iter(0..20).map(move |_| {
                observed.fetch_add(1, Ordering::SeqCst);
                owner.clone()
            }),
            BatchConfig::new(2, Duration::from_secs(60)),
            PausePolicy::Buffer { capacity: 3 },
            |_| BatchAction::Single(1),
        );
        wait_for(|| consumed.load(Ordering::SeqCst) == 3).await;
        match mode {
            0 => rx.close(),
            1 => subs.cancel_all(),
            2 => {
                subs.stream("source", tokio_stream::empty());
            }
            _ => drop(subs),
        }
        wait_for(|| weak.upgrade().is_none()).await;
        assert!(rx.try_recv().is_err());
    }
}

#[tokio::test]
async fn closed_receiver_stops_an_idle_source() {
    let (tx, rx) = ActionChannel::unbounded::<BatchAction>();
    let mut subs = Subscriptions::with_channel(tx);
    subs.stream_batched(
        "source",
        tokio_stream::pending::<i32>(),
        BatchConfig::new(3, Duration::from_secs(1)),
        PausePolicy::Drop,
        BatchAction::Batch,
    );
    drop(rx);
    wait_for(|| subs.is_empty()).await;
}

#[tokio::test]
async fn unbounded_channel_and_closed_send_errors_preserve_actions() {
    let (tx, mut rx) = ActionChannel::unbounded();
    tx.send(BatchAction::Single(1)).await.unwrap();
    rx.close();
    assert_eq!(
        tx.send(BatchAction::Single(2)).await.unwrap_err().0,
        BatchAction::Single(2)
    );
    assert_eq!(rx.recv().await, Some(BatchAction::Single(1)));
    assert_eq!(rx.recv().await, None);
    let (tx, rx) = ActionChannel::bounded(1);
    drop(rx);
    assert_eq!(
        tx.send(BatchAction::Single(3)).await.unwrap_err().0,
        BatchAction::Single(3)
    );
}

#[tokio::test]
async fn mapper_can_pause_without_deadlocking() {
    let (tx, mut rx) = ActionChannel::bounded(1);
    let mut subs = Subscriptions::with_channel(tx);
    let pause = subs.pause_handle();
    let (mapped_tx, mut mapped_rx) = tokio::sync::mpsc::unbounded_channel();
    subs.stream_batched(
        "source",
        tokio_stream::iter([1]),
        BatchConfig::new(1, Duration::ZERO),
        PausePolicy::Buffer { capacity: 1 },
        move |items| {
            pause.pause();
            mapped_tx.send(()).unwrap();
            BatchAction::Batch(items)
        },
    );
    mapped_rx.recv().await.unwrap();
    assert!(rx.try_recv().is_err());
    subs.resume();
    assert_eq!(rx.recv().await, Some(BatchAction::Batch(vec![1])));
}

#[cfg(feature = "testing-time")]
#[tokio::test(start_paused = true)]
async fn pause_preserves_expired_partial_batch_until_resume() {
    let (tx, mut rx) = ActionChannel::bounded(1);
    let mut subs = Subscriptions::with_channel(tx);
    let (source_tx, source_rx) = tokio::sync::mpsc::unbounded_channel();
    let (seen_tx, mut seen_rx) = tokio::sync::mpsc::unbounded_channel();
    subs.stream_batched(
        "source",
        UnboundedReceiverStream::new(source_rx).map(move |item| {
            seen_tx.send(()).unwrap();
            item
        }),
        BatchConfig::new(3, Duration::from_millis(50)),
        PausePolicy::Buffer { capacity: 2 },
        BatchAction::Batch,
    );
    source_tx.send(1).unwrap();
    seen_rx.recv().await.unwrap();
    subs.pause();
    tokio::time::advance(Duration::from_secs(1)).await;
    assert!(rx.try_recv().is_err());
    subs.resume();
    assert_eq!(rx.recv().await, Some(BatchAction::Batch(vec![1])));
    assert!(!source_tx.is_closed());
}

#[tokio::test]
async fn empty_source_never_calls_mapper() {
    let (tx, mut rx) = ActionChannel::bounded::<BatchAction>(1);
    let mut subs = Subscriptions::with_channel(tx);
    subs.stream_batched(
        "source",
        tokio_stream::empty::<i32>(),
        BatchConfig::new(3, Duration::ZERO),
        PausePolicy::Drop,
        |_| panic!("empty batch"),
    );
    wait_for(|| subs.is_empty()).await;
    assert!(rx.try_recv().is_err());
}

#[test]
#[should_panic(expected = "batch size must be positive")]
fn zero_batch_size_is_rejected() {
    BatchConfig::new(0, Duration::ZERO);
}

#[tokio::test]
#[should_panic(expected = "pause buffer capacity must be positive")]
async fn zero_buffer_capacity_is_rejected() {
    let (tx, _rx) = ActionChannel::bounded::<BatchAction>(1);
    Subscriptions::with_channel(tx).stream_with_policy(
        "source",
        tokio_stream::empty(),
        PausePolicy::Buffer { capacity: 0 },
    );
}

#[cfg(feature = "testing-time")]
#[tokio::test(start_paused = true)]
async fn drop_policy_discards_partial_batches_and_paused_items() {
    let (tx, mut rx) = ActionChannel::unbounded();
    let mut subs = Subscriptions::with_channel(tx);
    let (source_tx, source_rx) = tokio::sync::mpsc::unbounded_channel();
    let (seen_tx, mut seen_rx) = tokio::sync::mpsc::unbounded_channel();
    subs.stream_batched(
        "source",
        UnboundedReceiverStream::new(source_rx).map(move |item| {
            seen_tx.send(item).unwrap();
            item
        }),
        BatchConfig::new(3, Duration::from_millis(50)),
        PausePolicy::Drop,
        BatchAction::Batch,
    );
    source_tx.send(1).unwrap();
    assert_eq!(seen_rx.recv().await, Some(1));
    subs.pause();
    source_tx.send(2).unwrap();
    assert_eq!(seen_rx.recv().await, Some(2));
    tokio::time::advance(Duration::from_secs(1)).await;
    assert!(rx.try_recv().is_err());
    subs.resume();
    source_tx.send(3).unwrap();
    drop(source_tx);
    assert_eq!(rx.recv().await, Some(BatchAction::Batch(vec![3])));
    wait_for(|| subs.is_empty()).await;
    assert!(rx.try_recv().is_err());
}

#[cfg(feature = "testing-time")]
#[tokio::test(start_paused = true)]
async fn later_items_do_not_extend_the_partial_batch_deadline() {
    let (tx, mut rx) = ActionChannel::bounded(1);
    let mut subs = Subscriptions::with_channel(tx);
    let (source_tx, source_rx) = tokio::sync::mpsc::unbounded_channel();
    let (seen_tx, mut seen_rx) = tokio::sync::mpsc::unbounded_channel();
    subs.stream_batched(
        "source",
        UnboundedReceiverStream::new(source_rx).map(move |item| {
            seen_tx.send(item).unwrap();
            item
        }),
        BatchConfig::new(3, Duration::from_millis(50)),
        PausePolicy::Buffer { capacity: 3 },
        BatchAction::Batch,
    );
    source_tx.send(1).unwrap();
    assert_eq!(seen_rx.recv().await, Some(1));
    tokio::time::advance(Duration::from_millis(40)).await;
    source_tx.send(2).unwrap();
    assert_eq!(seen_rx.recv().await, Some(2));
    let received = tokio::time::timeout(Duration::from_millis(12), rx.recv())
        .await
        .unwrap();
    assert_eq!(received, Some(BatchAction::Batch(vec![1, 2])));
    assert!(!source_tx.is_closed());
}
