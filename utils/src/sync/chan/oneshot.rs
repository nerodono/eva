//! # Oneshot channel.

use std::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    pin::Pin,
    sync::{
        Arc,
        atomic::{
            AtomicU8,
            Ordering::{Acquire, Relaxed, Release},
        },
    },
    task::{Context, Poll, Waker},
};

use crate::sync::spin;

/// No value yet, sender is alive.
const EMPTY: u8 = 0;
/// Value is written and not yet taken; `item` is initialized.
const SENT: u8 = 1;
/// Value was moved out by the receiver; `item` is uninitialized.
const TAKEN: u8 = 2;
/// Sender was dropped without sending; `item` is uninitialized.
const CLOSED: u8 = 3;

struct Cell<T> {
    item: UnsafeCell<MaybeUninit<T>>,
    /// Publishes `item`: the `Release` store of [`SENT`] pairs with the
    /// `Acquire` loads in [`Rx::poll`], making the value write visible
    /// before the state is observed.
    state: AtomicU8,
    waker: spin::Lock<Option<Waker>>,
}

// SAFETY: the value is moved between threads (needs `T: Send`), never
// referenced from two threads at once: only the receiver dereferences
// `item`, and only after observing `SENT`.
unsafe impl<T: Send> Send for Cell<T> {}
unsafe impl<T: Send> Sync for Cell<T> {}

impl<T> Cell<T> {
    fn wake_rx(&self) {
        // Contention means the receiver is mid-registration; it re-checks
        // the state after registering, so the wake-up is not lost.
        let Ok(waker) = self.waker.try_lock() else {
            return;
        };
        let Some(waker) = &*waker else {
            return;
        };
        waker.wake_by_ref();
    }
}

impl<T> Drop for Cell<T> {
    fn drop(&mut self) {
        // `&mut self` here means this is the last owner; `Arc` already
        // synchronized all writes of the other side.
        if *self.state.get_mut() == SENT {
            unsafe { (*self.item.get()).assume_init_drop() };
        }
    }
}

pub struct Rx<T>(Option<Arc<Cell<T>>>);

impl<T> Rx<T> {
    fn take_sent(cell: &Cell<T>) -> T {
        let item = unsafe { (*cell.item.get()).assume_init_read() };
        // Only the receiver reads `state` past this point (in `Cell::drop`,
        // where `Arc` provides the synchronization), so `Relaxed` is enough.
        cell.state.store(TAKEN, Relaxed);

        item
    }
}

impl<T> Future for Rx<T> {
    type Output = Option<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let Some(cell) = this.0.as_deref() else {
            return Poll::Ready(None);
        };

        match cell.state.load(Acquire) {
            SENT => {
                let item = Self::take_sent(cell);
                this.0 = None;
                return Poll::Ready(Some(item));
            }
            CLOSED => {
                this.0 = None;
                return Poll::Ready(None);
            }
            _ => {}
        }

        if let Ok(mut waker) = cell.waker.try_lock() {
            match &mut *waker {
                Some(prev) => {
                    prev.clone_from(cx.waker());
                }
                None => {
                    *waker = Some(cx.waker().clone());
                }
            }
        }
        // Lock contention means the sender is mid-wake, i.e. the state is
        // already set — the re-check below observes it.

        match cell.state.load(Acquire) {
            SENT => {
                let item = Self::take_sent(cell);
                this.0 = None;

                Poll::Ready(Some(item))
            }
            CLOSED => {
                this.0 = None;

                Poll::Ready(None)
            }
            _ => Poll::Pending,
        }
    }
}

pub struct Tx<T>(Option<Arc<Cell<T>>>);

impl<T> Tx<T> {
    pub fn send(mut self, value: T) {
        let cell = self.0.take().expect("present until sent or dropped");
        unsafe { (*cell.item.get()).write(value) };
        cell.state.store(SENT, Release);
        cell.wake_rx();
    }
}

impl<T> Drop for Tx<T> {
    fn drop(&mut self) {
        let Some(cell) = self.0.take() else {
            return;
        };

        cell.state.store(CLOSED, Release);
        cell.wake_rx();
    }
}

pub fn make<T>() -> (Tx<T>, Rx<T>) {
    let cell = Arc::new(Cell {
        item: UnsafeCell::new(MaybeUninit::uninit()),
        state: AtomicU8::new(EMPTY),
        waker: spin::Lock::new(None),
    });

    let tx = Tx(Some(Arc::clone(&cell)));
    let rx = Rx(Some(cell));

    (tx, rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::AtomicBool;

    fn poll<T>(rx: &mut Rx<T>) -> Poll<Option<T>> {
        let mut cx = Context::from_waker(Waker::noop());
        Pin::new(rx).poll(&mut cx)
    }

    struct Guard(Arc<AtomicBool>);

    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.store(true, Relaxed);
        }
    }

    #[test]
    fn send_then_recv() {
        let (tx, mut rx) = make();
        tx.send(42_u32);

        assert_eq!(poll(&mut rx), Poll::Ready(Some(42)));
        assert_eq!(poll(&mut rx), Poll::Ready(None));
    }

    #[test]
    fn pending_until_send() {
        let (tx, mut rx) = make();

        assert_eq!(poll(&mut rx), Poll::Pending);
        tx.send(1_u8);
        assert_eq!(poll(&mut rx), Poll::Ready(Some(1)));
    }

    #[test]
    fn tx_dropped_without_send() {
        let (tx, mut rx) = make::<u32>();

        assert_eq!(poll(&mut rx), Poll::Pending);
        drop(tx);
        assert_eq!(poll(&mut rx), Poll::Ready(None));
    }

    #[test]
    fn unreceived_value_is_dropped() {
        let dropped = Arc::new(AtomicBool::new(false));
        let (tx, rx) = make();

        tx.send(Guard(Arc::clone(&dropped)));
        assert!(!dropped.load(Relaxed));

        drop(rx);
        assert!(dropped.load(Relaxed));
    }

    #[test]
    fn received_value_is_not_double_dropped() {
        let dropped = Arc::new(AtomicBool::new(false));
        let (tx, mut rx) = make();

        tx.send(Guard(Arc::clone(&dropped)));
        let Poll::Ready(Some(guard)) = poll(&mut rx) else {
            panic!("must be ready");
        };
        assert!(!dropped.load(Relaxed));

        drop(guard);
        assert!(dropped.load(Relaxed));
        dropped.store(false, Relaxed);
        drop(rx);
        assert!(!dropped.load(Relaxed));
    }
}
