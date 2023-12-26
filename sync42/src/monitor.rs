//! Implementation of a monitor in classic Hoare-style.
//!
//! A monitor allows for synchronization that is more concurrent than just using mutexes and
//! condition variables.  Concretely, [MonitorCore] breaks the task into the coordination and the
//! critical section.  Threads can concurrently coordinate the next-best thread to enter the
//! citical section without blocking the critical section from executing.

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

/// The core traits of the monitor.  Acquire the monitor, enter the critical section, release the
/// monitor.
pub trait MonitorCore<COORD, CRIT, WS> {
    /// Acquire the monitor.  Returns true iff the thread can enter the critical section.
    fn acquire<'a: 'b, 'b>(
        &self,
        guard: MutexGuard<'a, COORD>,
        t: &'b mut WS,
    ) -> (bool, MutexGuard<'a, COORD>);

    /// Release the monitor.  In a proper monitor release will always follow a call to acquire by
    /// the same thread.  Release is responsible for waking threads blocked in acquire.
    fn release<'a: 'b, 'b>(
        &self,
        guard: MutexGuard<'a, COORD>,
        t: &'b mut WS,
    ) -> MutexGuard<'a, COORD>;

    /// Critical section captures the mutually-exclusive section of the monitor.  The monitor
    /// ensures that there will only be one thread in the at a time, even in the case of
    /// acquire/release bugs.
    fn critical_section<'a: 'b, 'b>(&self, crit: &'a mut CRIT, t: &'b mut WS);
}

pub struct Monitor<COORD, CRIT, WS, M: MonitorCore<COORD, CRIT, WS>> {
    core: M,
    coordination: Mutex<COORD>,
    synchronization: AtomicBool,
    critical_section: UnsafeCell<CRIT>,
    _phantom_ws: std::marker::PhantomData<WS>,
}

impl<COORD, CRIT, WS, M: MonitorCore<COORD, CRIT, WS>> Monitor<COORD, CRIT, WS, M> {
    /// Create a new monitor with the provided coordination and critical_section.
    pub fn new(core: M, coordination: COORD, critical_section: CRIT) -> Self {
        Self {
            core,
            coordination: Mutex::new(coordination),
            synchronization: AtomicBool::new(false),
            critical_section: UnsafeCell::new(critical_section),
            _phantom_ws: std::marker::PhantomData,
        }
    }

    /// Enter the monitor and call into the critical section if the call to
    /// [`MonitorCore::acquire`] succeeds.
    pub fn do_it<'a: 'a, 'b>(&'a self, t: &'b mut WS) {
        {
            let coordination = self.coordination.lock().unwrap();
            let (acquired, _coordination) = self.core.acquire(coordination, t);
            if !acquired {
                return;
            }
            if self.synchronization.swap(true, Ordering::Acquire) {
                panic!("synchronization invariant violated: acquire should only allow one thread at a time in the critical section");
            }
        }
        let crit: &mut CRIT = unsafe { &mut *self.critical_section.get() };
        self.core.critical_section(crit, t);
        let coordination = self.coordination.lock().unwrap();
        self.synchronization.store(false, Ordering::Release);
        drop(self.core.release(coordination, t));
    }

    /// Decompose the monitor into its coordinator and critical section respectively.
    pub fn decompose(self) -> (M, COORD, CRIT) {
        (
            self.core,
            self.coordination.into_inner().unwrap(),
            self.critical_section.into_inner(),
        )
    }
}

unsafe impl<COORD, CRIT, WS, M: MonitorCore<COORD, CRIT, WS>> Send for Monitor<COORD, CRIT, WS, M> {}
unsafe impl<COORD, CRIT, WS, M: MonitorCore<COORD, CRIT, WS>> Sync for Monitor<COORD, CRIT, WS, M> {}
