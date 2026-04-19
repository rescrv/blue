//! An efficient FIFO spin lock.  Threads will contend in much the same way as an MCS lock, relying
//! on the caching system to shoot down the cache line when it gets written.

use std::cell::UnsafeCell;
use std::fmt::{Debug, Formatter};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU64, Ordering};

/////////////////////////////////////////// CacheAligned ///////////////////////////////////////////

#[repr(align(64))]
struct CacheAligned<T>(T);

impl<T> Deref for CacheAligned<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> DerefMut for CacheAligned<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

///////////////////////////////////////////// SpinLock /////////////////////////////////////////////

/// [SpinLock] provides fast synchronization where blocking threads would be too costly.  Use only
/// for situations where a small, countable number of operations happen under lock.
pub struct SpinLock<T> {
    acquires: CacheAligned<AtomicU64>,
    releases: AtomicU64,
    data: CacheAligned<UnsafeCell<T>>,
}

impl<T> SpinLock<T> {
    /// Create a new [SpinLock] that protects a `T`.
    pub fn new(t: T) -> Self {
        Self {
            acquires: CacheAligned(AtomicU64::new(0)),
            releases: AtomicU64::new(0),
            data: CacheAligned(UnsafeCell::new(t)),
        }
    }

    /// Lock until the guard is dropped.
    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        let index = self.acquires.fetch_add(1, Ordering::Relaxed);
        while index > self.releases.load(Ordering::Acquire) {
            std::hint::spin_loop();
        }
        SpinLockGuard { lock: self, index }
    }

    /// Consume the spinlock and return its data.
    pub fn into_inner(self) -> T {
        self.data.0.into_inner()
    }
}

impl<T> Debug for SpinLock<T> {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        // TODO(rescrv): Make this T the type of T.
        write!(fmt, "SpinLock<T>")
    }
}

unsafe impl<T: Send> Send for SpinLock<T> {}
unsafe impl<T: Send> Sync for SpinLock<T> {}

/////////////////////////////////////////// SpinLockGuard //////////////////////////////////////////

/// A guard on an active spinlock.
pub struct SpinLockGuard<'a, T> {
    lock: &'a SpinLock<T>,
    index: u64,
}

impl<T> Drop for SpinLockGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.releases.store(self.index + 1, Ordering::Release);
    }
}

impl<T> Deref for SpinLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> DerefMut for SpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.data.get() }
    }
}
