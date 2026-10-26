//! # Simple spinlock.

use std::{
    cell::UnsafeCell,
    error::Error,
    fmt, hint,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Contended;

impl fmt::Display for Contended {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the lock is contended")
    }
}

impl Error for Contended {}

pub struct Lock<T: ?Sized> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized> Send for Lock<T> {}
unsafe impl<T: ?Sized> Sync for Lock<T> {}

impl<T: Default> Default for Lock<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> Lock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    /// Get exclusive access, spin until it's available.
    #[allow(clippy::single_match)]
    pub fn lock(&self) -> Guard<'_, T> {
        loop {
            match self.try_lock() {
                Ok(g) => return g,
                Err(Contended) => {}
            }
            hint::spin_loop();
        }
    }

    /// Try get exclusive access without spinning.
    pub fn try_lock(&self) -> Result<Guard<'_, T>, Contended> {
        if self.locked.swap(true, Ordering::Acquire) {
            Err(Contended)
        } else {
            Ok(Guard {
                ref_: unsafe { &mut *self.data.get() },
                unlock: &self.locked,
            })
        }
    }
}

#[derive(Debug)]
pub struct Guard<'a, T: ?Sized> {
    ref_: &'a mut T,
    unlock: &'a AtomicBool,
}

impl<'a, T: ?Sized> Deref for Guard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &*self.ref_
    }
}

impl<'a, T: ?Sized> DerefMut for Guard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.ref_
    }
}

impl<'a, T: ?Sized> Drop for Guard<'a, T> {
    fn drop(&mut self) {
        self.unlock.store(false, Ordering::Release);
    }
}
