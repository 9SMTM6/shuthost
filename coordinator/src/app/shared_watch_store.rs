use alloc::sync::Arc;
use tokio::sync::{Mutex as AsyncMutex, watch};

pub(crate) type SharedWatchTx<T> = watch::Sender<Arc<T>>;
pub(crate) type SharedWatchRx<T> = watch::Receiver<Arc<T>>;

/// A store for controlled shared access to its values:
/// 
/// Writes are serialized via a [`AsyncMutex`], and all
/// mutations are published to a [`watch`] channel so background tasks can
/// subscribe to changes.
pub(crate) struct SharedWatchStore<T> {
    pub(crate) inner: AsyncMutex<T>,
    pub(crate) tx: SharedWatchTx<T>,
}

impl<T> SharedWatchStore<T>
where
    T: Clone + Send + Sync + 'static,
{
    pub(crate) fn new(initial: T) -> (Arc<Self>, SharedWatchRx<T>) {
        let (tx, rx) = watch::channel(Arc::new(initial.clone()));
        (
            Arc::new(Self {
                inner: AsyncMutex::new(initial),
                tx,
            }),
            rx,
        )
    }

    pub(crate) fn borrow(&self) -> watch::Ref<'_, Arc<T>> {
        self.tx.borrow()
    }

    pub(crate) fn subscribe(&self) -> SharedWatchRx<T> {
        self.tx.subscribe()
    }

    pub(crate) fn snapshot(&self) -> Arc<T> {
        self.tx.borrow().clone()
    }
}
