//! Implementation of a monitor in classic Hoare-style.
//!
//! A monitor allows for synchronization that is more concurrent than just using mutexes and
//! condition variables.  Concretely, [Monitor] breaks the task into the coordination and the
//! critical section.  Threads can concurrently coordinate the next-best thread to enter the
//! citical section without blocking the critical section from executing.

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

/// Coordination tracks acquisition and release of the monitor.  It is an error that will induce
/// panic to allow multiple threads to be between [`Coordination::acquire`] and [`Coordination::release`] calls.
pub trait Coordination<T> {
    /// Acquire the monitor.  Returns true if and only if the monitor can enter the critical
    /// section.
    fn acquire<'a: 'b, 'b>(guard: MutexGuard<'a, Self>, t: &'b mut T) -> (bool, MutexGuard<'a, Self>);
    /// Release the monitor.  In a proper monitor [`Coordination::release`] will always follow a call to
    /// acquire on the same thread.  Release is responsible for waking threads blocked in acquire.
    fn release<'a: 'b, 'b>(guard: MutexGuard<'a, Self>, t: &'b mut T) -> MutexGuard<'a, Self>;
}

/// Critical section captures the mutually-exclusive section of the monitor.  The monitor ensures
/// that there will only be one thread in the [`CriticalSection::critical_section`] at a time.
pub trait CriticalSection<T> {
    fn critical_section<'a: 'b, 'b>(&'a mut self, t: &'b mut T);
}

/// Monitor provides the monitor pattern.  It will synchronize calls to acquire and release and the
/// critical section so that at most one thread is in the critical section at once.
pub struct Monitor<T, COORD: Coordination<T>, CRIT: CriticalSection<T>> {
    coordination: Mutex<COORD>,
    synchronization: AtomicBool,
    critical_section: UnsafeCell<CRIT>,
    _t: std::marker::PhantomData::<T>,
}

impl<T, COORD: Coordination<T>, CRIT: CriticalSection<T>> Monitor<T, COORD, CRIT> {
    /// Create a new monitor with the provided coordination and critical_section.
    pub fn new(coordination: COORD, critical_section: CRIT) -> Self {
        Self {
            coordination: Mutex::new(coordination),
            synchronization: AtomicBool::new(false),
            critical_section: UnsafeCell::new(critical_section),
            _t: std::marker::PhantomData,
        }
    }

    /// Enter the monitor and call into the critical section if the call to
    /// [`Coordination::acquire`] succeeds.
    pub fn do_it(&self, t: &mut T) {
        {
            let coordination = self.coordination.lock().unwrap();
            let (acquired, _coordination) = COORD::acquire(coordination, t);
            if !acquired {
                return;
            }
            if self.synchronization.swap(true, Ordering::Acquire) {
                panic!("synchronization invariant violated: acquire should only allow one thread at a time in the critical section");
            }
        }
        let crit: &mut CRIT = unsafe { &mut *self.critical_section.get() };
        crit.critical_section(t);
        let coordination = self.coordination.lock().unwrap();
        self.synchronization.store(false, Ordering::Release);
        drop(COORD::release(coordination, t));
    }
}

unsafe impl<T, COORD: Coordination<T>, CRIT: CriticalSection<T>> Send for Monitor<T, COORD, CRIT> {}
unsafe impl<T, COORD: Coordination<T>, CRIT: CriticalSection<T>> Sync for Monitor<T, COORD, CRIT> {}
