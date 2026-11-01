//! Action delivery with optional bounded backpressure.
use tokio::sync::mpsc;

/// Factory for bounded or unbounded action channels.
pub struct ActionChannel;

impl ActionChannel {
    /// Create a channel that waits for capacity instead of dropping actions.
    ///
    /// # Panics
    /// Panics if `capacity` is zero (or exceeds Tokio's semaphore limit).
    pub fn bounded<A>(capacity: usize) -> (ActionSender<A>, ActionReceiver<A>) {
        let (tx, rx) = mpsc::channel(capacity);
        (ActionSender::Bounded(tx), ActionReceiver::Bounded(rx))
    }

    /// Create a channel without a capacity limit.
    pub fn unbounded<A>() -> (ActionSender<A>, ActionReceiver<A>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (ActionSender::Unbounded(tx), ActionReceiver::Unbounded(rx))
    }
}

/// Cloneable sender supporting bounded and unbounded action delivery.
#[derive(Debug)]
pub enum ActionSender<A> {
    /// A bounded Tokio sender.
    Bounded(mpsc::Sender<A>),
    /// An unbounded Tokio sender.
    Unbounded(mpsc::UnboundedSender<A>),
}

impl<A> Clone for ActionSender<A> {
    fn clone(&self) -> Self {
        match self {
            Self::Bounded(tx) => Self::Bounded(tx.clone()),
            Self::Unbounded(tx) => Self::Unbounded(tx.clone()),
        }
    }
}

impl<A> From<mpsc::Sender<A>> for ActionSender<A> {
    fn from(tx: mpsc::Sender<A>) -> Self {
        Self::Bounded(tx)
    }
}

impl<A> From<mpsc::UnboundedSender<A>> for ActionSender<A> {
    fn from(tx: mpsc::UnboundedSender<A>) -> Self {
        Self::Unbounded(tx)
    }
}

impl<A> ActionSender<A> {
    /// Send an action, waiting for capacity on a bounded channel.
    /// Cancelling this future drops the unsent action.
    pub async fn send(&self, action: A) -> Result<(), mpsc::error::SendError<A>> {
        match self {
            Self::Bounded(tx) => tx.send(action).await,
            Self::Unbounded(tx) => tx.send(action),
        }
    }

    /// Wait until the receiver closes or is dropped.
    pub async fn closed(&self) {
        match self {
            Self::Bounded(tx) => tx.closed().await,
            Self::Unbounded(tx) => tx.closed().await,
        }
    }

    pub(crate) async fn reserve(&self) -> Result<ActionPermit<'_, A>, ()> {
        match self {
            Self::Bounded(tx) => tx
                .reserve()
                .await
                .map(ActionPermit::Bounded)
                .map_err(|_| ()),
            Self::Unbounded(tx) if !tx.is_closed() => Ok(ActionPermit::Unbounded(tx)),
            Self::Unbounded(_) => Err(()),
        }
    }
}

pub(crate) enum ActionPermit<'a, A> {
    Bounded(mpsc::Permit<'a, A>),
    Unbounded(&'a mpsc::UnboundedSender<A>),
}

impl<A> ActionPermit<'_, A> {
    pub(crate) fn send(self, action: A) {
        match self {
            Self::Bounded(permit) => {
                permit.send(action);
            }
            Self::Unbounded(tx) => {
                let _ = tx.send(action);
            }
        }
    }
}

/// Receiving half of an action channel.
#[derive(Debug)]
pub enum ActionReceiver<A> {
    /// A bounded Tokio receiver.
    Bounded(mpsc::Receiver<A>),
    /// An unbounded Tokio receiver.
    Unbounded(mpsc::UnboundedReceiver<A>),
}

impl<A> ActionReceiver<A> {
    /// Receive the next action, or `None` once closed and drained.
    pub async fn recv(&mut self) -> Option<A> {
        match self {
            Self::Bounded(rx) => rx.recv().await,
            Self::Unbounded(rx) => rx.recv().await,
        }
    }

    /// Receive without waiting.
    pub fn try_recv(&mut self) -> Result<A, mpsc::error::TryRecvError> {
        match self {
            Self::Bounded(rx) => rx.try_recv(),
            Self::Unbounded(rx) => rx.try_recv(),
        }
    }

    /// Reject new sends while allowing already queued actions to be drained.
    pub fn close(&mut self) {
        match self {
            Self::Bounded(rx) => rx.close(),
            Self::Unbounded(rx) => rx.close(),
        }
    }
}
