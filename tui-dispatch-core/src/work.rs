//! Spawn production work, with wake tracking when deterministic testing is enabled.
use std::future::Future;
use tokio::task::JoinHandle;

#[derive(Default)]
pub(crate) struct WorkSpawner {
    #[cfg(feature = "testing-time")]
    ready: Vec<std::sync::Weak<std::sync::atomic::AtomicBool>>,
}

impl WorkSpawner {
    pub(crate) fn spawn<F>(&mut self, future: F) -> JoinHandle<()>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        #[cfg(feature = "testing-time")]
        {
            let ready = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
            self.ready.retain(|entry| entry.strong_count() > 0);
            self.ready.push(std::sync::Arc::downgrade(&ready));
            tokio::spawn(tracked::Tracked {
                future: Box::pin(future),
                ready,
            })
        }
        #[cfg(not(feature = "testing-time"))]
        tokio::spawn(future)
    }

    #[cfg(feature = "testing-time")]
    pub(crate) fn has_runnable(&self) -> bool {
        self.ready.iter().any(|entry| {
            entry
                .upgrade()
                .is_some_and(|ready| ready.load(std::sync::atomic::Ordering::SeqCst))
        })
    }
}

#[cfg(feature = "testing-time")]
mod tracked {
    use std::{
        future::Future,
        pin::Pin,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Weak,
        },
        task::{Context, Poll, Wake, Waker},
    };

    pub(super) struct Tracked<F> {
        pub(super) future: Pin<Box<F>>,
        pub(super) ready: Arc<AtomicBool>,
    }

    struct TrackedWake {
        ready: Weak<AtomicBool>,
        parent: Waker,
    }

    impl Wake for TrackedWake {
        fn wake(self: Arc<Self>) {
            self.wake_by_ref();
        }

        fn wake_by_ref(self: &Arc<Self>) {
            if let Some(ready) = self.ready.upgrade() {
                ready.store(true, Ordering::SeqCst);
                self.parent.wake_by_ref();
            }
        }
    }

    impl<F: Future> Future for Tracked<F> {
        type Output = F::Output;

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            // Clear before polling, so self-wakes (including yield_now) remain visible.
            self.ready.store(false, Ordering::SeqCst);
            let waker = Waker::from(Arc::new(TrackedWake {
                ready: Arc::downgrade(&self.ready),
                parent: cx.waker().clone(),
            }));
            self.future.as_mut().poll(&mut Context::from_waker(&waker))
        }
    }
}
