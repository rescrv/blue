use std::cell::UnsafeCell;
use std::fmt::{Debug, Formatter};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU64, Ordering};

///////////////////////////////////////////// SpinLock /////////////////////////////////////////////

pub struct SpinLock<T> {
    acquires: AtomicU64,
    releases: AtomicU64,
    data: UnsafeCell<T>,
}

impl<T> SpinLock<T> {
    pub fn new(t: T) -> Self {
        Self {
            acquires: AtomicU64::new(0),
            releases: AtomicU64::new(0),
            data: UnsafeCell::new(t),
        }
    }

    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        let index = self.acquires.fetch_add(1, Ordering::Relaxed);
        while index > self.releases.load(Ordering::Acquire) {
            std::hint::spin_loop();
        }
        SpinLockGuard {
            lock: self,
            index,
        }
    }

    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
}

impl<T> Debug for SpinLock<T> {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        // TODO(rescrv): Make this T the type of T.
        write!(fmt, "SpinLock<T>")
    }
}

unsafe impl<T> Send for SpinLock<T> {}
unsafe impl<T> Sync for SpinLock<T> {}

/////////////////////////////////////////// SpinLockGuard //////////////////////////////////////////

pub struct SpinLockGuard<'a, T> {
    lock: &'a SpinLock<T>,
    index: u64,
}

impl<'a, T> Drop for SpinLockGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.releases.store(self.index + 1, Ordering::Release);
    }
}

impl<'a, T> Deref for SpinLockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe {
            &*self.lock.data.get()
        }
    }
}

impl<'a, T> DerefMut for SpinLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            &mut *self.lock.data.get()
        }
    }
}
