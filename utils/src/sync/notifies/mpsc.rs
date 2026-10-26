//! # Multi producer, single consumer notifier.

use crate::sync::spin;

use std::{
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering::Relaxed},
    },
    task::{Context, Poll, Waker},
};

/// Raw primitive, which is used internally by the notify.
pub struct Cell {
    should_wake_up: AtomicBool,
    waker: spin::Lock<Option<Waker>>,
}

impl Default for Cell {
    fn default() -> Self {
        Self::new()
    }
}

impl Cell {
    pub const fn new() -> Self {
        Self {
            should_wake_up: AtomicBool::new(false),
            waker: spin::Lock::new(None),
        }
    }

    /// Set the wake-up flag and wake the registered waker, if any.
    ///
    /// If the waker slot is contended (consumer is mid-registration),
    /// only the flag is set — the consumer re-checks it after
    /// registering, so the wake-up is not lost.
    ///
    /// The flag is `Relaxed`: it carries no happens-before for any
    /// associated data, only a "please poll again" signal.
    pub fn wake(&self) {
        self.should_wake_up.store(true, Relaxed);
        let Ok(waker) = self.waker.try_lock() else {
            return;
        };
        let Some(waker) = &*waker else {
            return;
        };
        waker.wake_by_ref();
    }

    /// Poll the wake-up flag without consuming it: once set, every
    /// subsequent call returns [`Poll::Ready`].
    ///
    /// May spuriously return [`Poll::Ready`] when the waker slot is
    /// contended; the caller must tolerate polls with the flag unset.
    pub fn poll_peek(&self, new_waker: &Waker) -> Poll<()> {
        use Poll::*;

        if self.should_wake_up.load(Relaxed) {
            return Ready(());
        }

        {
            let Ok(mut waker) = self.waker.try_lock() else {
                return Ready(());
            };
            match &mut *waker {
                Some(prev) => {
                    prev.clone_from(new_waker);
                }
                None => {
                    *waker = Some(new_waker.clone());
                }
            }
        }

        if self.should_wake_up.load(Relaxed) {
            Ready(())
        } else {
            Pending
        }
    }

    /// Poll the wake-up flag, consuming it on [`Poll::Ready`].
    ///
    /// May spuriously return [`Poll::Ready`] when the waker slot is
    /// contended — including consuming a flag set for a later wake —
    /// so wake-ups may coalesce; the caller must tolerate both.
    pub fn poll_once(&self, new_waker: &Waker) -> Poll<()> {
        use Poll::*;

        if self.should_wake_up.swap(false, Relaxed) {
            return Ready(());
        }

        {
            let Ok(mut waker) = self.waker.try_lock() else {
                self.should_wake_up.store(false, Relaxed);
                return Ready(());
            };

            match &mut *waker {
                Some(prev) => {
                    prev.clone_from(new_waker);
                }
                None => {
                    *waker = Some(new_waker.clone());
                }
            }
        }

        if self.should_wake_up.swap(false, Relaxed) {
            Ready(())
        } else {
            Pending
        }
    }
}

/// Receiving part of the mpsc notify.
pub struct Rx(Arc<Cell>);

impl Rx {
    /// Wait for the wake-up flag.
    pub async fn wait(&mut self) {
        Pin::new(self).await;
    }
}

impl Future for Rx {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.0.poll_once(cx.waker())
    }
}

/// Sending part of the mpsc notify.
#[derive(Clone)]
pub struct Tx(Arc<Cell>);

impl Tx {
    /// Set wake-up flag to associated [`Rx`], if [`Rx`] is currently waiting -
    /// it would be woken up.
    pub fn wake(&self) {
        let cell = &*self.0;
        cell.wake();
    }
}

/// Make rx/tx notifier pair.
pub fn make() -> (Tx, Rx) {
    let cell = Arc::new(Cell {
        should_wake_up: AtomicBool::new(false),
        waker: spin::Lock::new(None),
    });
    let tx = Tx(Arc::clone(&cell));
    let rx = Rx(cell);

    (tx, rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::{
        sync::atomic::{AtomicUsize, Ordering::SeqCst},
        task::Wake,
    };

    struct CountingWake(AtomicUsize);

    impl Wake for CountingWake {
        fn wake(self: Arc<Self>) {
            self.wake_by_ref();
        }

        fn wake_by_ref(self: &Arc<Self>) {
            self.0.fetch_add(1, SeqCst);
        }
    }

    fn counting_waker() -> (Arc<CountingWake>, Waker) {
        let count = Arc::new(CountingWake(AtomicUsize::new(0)));
        let waker = Waker::from(Arc::clone(&count));

        (count, waker)
    }

    #[test]
    fn peek_does_not_consume() {
        let cell = Cell::new();
        let (_, waker) = counting_waker();

        assert_eq!(cell.poll_peek(&waker), Poll::Pending);
        cell.wake();
        assert_eq!(cell.poll_peek(&waker), Poll::Ready(()));
        assert_eq!(cell.poll_peek(&waker), Poll::Ready(()));
    }

    #[test]
    fn once_consumes() {
        let cell = Cell::new();
        let (_, waker) = counting_waker();

        cell.wake();
        assert_eq!(cell.poll_once(&waker), Poll::Ready(()));
        assert_eq!(cell.poll_once(&waker), Poll::Pending);
    }

    #[test]
    fn wake_after_registration_calls_waker() {
        let cell = Cell::new();
        let (count, waker) = counting_waker();

        assert_eq!(cell.poll_once(&waker), Poll::Pending);
        assert_eq!(count.0.load(SeqCst), 0);

        cell.wake();
        assert_eq!(count.0.load(SeqCst), 1);
        assert_eq!(cell.poll_once(&waker), Poll::Ready(()));
    }

    #[test]
    fn rx_future_wakes_up() {
        let (tx, mut rx) = make();
        let (count, waker) = counting_waker();
        let mut cx = Context::from_waker(&waker);

        assert_eq!(Pin::new(&mut rx).poll(&mut cx), Poll::Pending);
        tx.wake();
        assert_eq!(count.0.load(SeqCst), 1);
        assert_eq!(Pin::new(&mut rx).poll(&mut cx), Poll::Ready(()));
        assert_eq!(Pin::new(&mut rx).poll(&mut cx), Poll::Pending);
    }

    #[test]
    fn wakes_coalesce() {
        let (tx, mut rx) = make();
        let (_, waker) = counting_waker();
        let mut cx = Context::from_waker(&waker);

        tx.wake();
        tx.wake();
        assert_eq!(Pin::new(&mut rx).poll(&mut cx), Poll::Ready(()));
        assert_eq!(Pin::new(&mut rx).poll(&mut cx), Poll::Pending);
    }
}
