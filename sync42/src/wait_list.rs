//! [WaitList] provides a collection for synchronizing threads.  Internally, the collection owns
//! MAX_CONCURRENCY positions of rendezvous called Waiter (private).  You can call [WaitList::link]
//! to allocate a waiter and return a [WaitGuard].  Holding a wait guard allows one to construct an
//! iterator or random-accessor over all subsequent threads in the data structure.  In this way,
//! the head of the list can iterate the list, batching operations together, and then iterate the
//! list again to distribute the batched work.
//!
//! See examples/clicker.rs for a concrete, complete example of exactly that.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard};

use biometrics::Counter;

use super::MAX_CONCURRENCY;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static NEW_WAIT_LIST: Counter = Counter::new("sync42.wait_list.new");

static NOTIFY_HEAD: Counter = Counter::new("sync42.wait_list.notify_head");
static NOTIFY_HEAD_DROPPED: Counter = Counter::new("sync42.wait_list.notify_head_dropped");
static WAITING_FOR_WAITERS: Counter = Counter::new("sync42.wait_list.waiting_for_waiters");

static LINK: Counter = Counter::new("sync42.wait_list.link");
static UNLINK: Counter = Counter::new("sync42.wait_list.unlink");

/// Register biometrics for the wait_list.
pub fn register_biometrics(collector: &biometrics::Collector) {
    collector.register_counter(&NEW_WAIT_LIST);
    collector.register_counter(&NOTIFY_HEAD);
    collector.register_counter(&NOTIFY_HEAD_DROPPED);
    collector.register_counter(&WAITING_FOR_WAITERS);
    collector.register_counter(&LINK);
    collector.register_counter(&UNLINK);
}

////////////////////////////////////////////// Waiter //////////////////////////////////////////////

#[derive(Debug)]
struct Waiter<T: Clone> {
    cond: Condvar,
    value: Mutex<Option<T>>,
    seq_no: AtomicU64,
    linked: AtomicBool,
}

impl<T: Clone> Waiter<T> {
    fn new() -> Self {
        Self {
            cond: Condvar::new(),
            value: Mutex::new(None),
            seq_no: AtomicU64::new(0),
            linked: AtomicBool::new(false),
        }
    }

    fn initialize<'a, M>(&self, mut guard: MutexGuard<'a, M>, t: T) -> MutexGuard<'a, M> {
        guard = self.store(guard, t);
        self.linked.store(true, Ordering::SeqCst);
        guard
    }

    fn deinitialize<'a, M>(&self, guard: MutexGuard<'a, M>) -> MutexGuard<'a, M> {
        self.value.lock().unwrap().take();
        guard
    }

    fn store<'a, M>(&self, guard: MutexGuard<'a, M>, t: T) -> MutexGuard<'a, M> {
        *self.value.lock().unwrap() = Some(t);
        self.seq_no.fetch_add(1, Ordering::Relaxed);
        self.cond.notify_one();
        guard
    }

    fn load<'a, M>(&self, guard: MutexGuard<'a, M>) -> (MutexGuard<'a, M>, T) {
        (guard, self.value.lock().unwrap().as_ref().unwrap().clone())
    }

    fn swap<'a, M>(&self, guard: MutexGuard<'a, M>, t: &mut T) -> MutexGuard<'a, M> {
        let mut value = self.value.lock().unwrap();
        let x: &mut T = value.as_mut().expect("should always be some when initialized");
        std::mem::swap(x, t);
        guard
    }

    fn notify(&self) {
        self.cond.notify_one()
    }

    fn naked_wait<'a, M>(&self, guard: MutexGuard<'a, M>) -> MutexGuard<'a, M> {
        self.cond.wait(guard).unwrap()
    }

    fn wait_for_store<'a, M>(&self, mut guard: MutexGuard<'a, M>) -> (MutexGuard<'a, M>, T) {
        let seq_no = self.seq_no.load(Ordering::Relaxed);
        while seq_no == self.seq_no.load(Ordering::Relaxed) {
            guard = self.cond.wait(guard).unwrap();
        }
        self.load(guard)
    }
}

/////////////////////////////////////////// WaitListState //////////////////////////////////////////

#[derive(Debug)]
struct WaitListState {
    head: u64,
    tail: u64,
    waiting_for_available: u64,
}

///////////////////////////////////////////// WaitList /////////////////////////////////////////////

/// [WaitList] provides the main collection.
#[derive(Debug)]
pub struct WaitList<T: Clone> {
    state: Mutex<WaitListState>,
    waiters: Vec<Waiter<T>>,
    wait_waiter_available: Condvar,
}

impl<T: Clone> WaitList<T> {
    pub fn new() -> Self {
        NEW_WAIT_LIST.click();
        let mut waiters: Vec<Waiter<T>> = Vec::new();
        for _ in 0..MAX_CONCURRENCY {
            waiters.push(Waiter::new());
        }
        let state = WaitListState {
            head: 0,
            tail: 0,
            waiting_for_available: 0,
        };
        Self {
            state: Mutex::new(state),
            waiters,
            wait_waiter_available: Condvar::new(),
        }
    }

    pub fn link<>(&self, t: T) -> WaitGuard<T> {
        let mut state = self.state.lock().unwrap();
        while state.head + (self.waiters.len() as u64) <= state.tail {
            state = self.assert_invariants(state);
            state.waiting_for_available += 1;
            WAITING_FOR_WAITERS.click();
            state = self.wait_waiter_available.wait(state).unwrap();
            state.waiting_for_available -= 1;
            state = self.assert_invariants(state);
        }
        let index = state.tail;
        state.tail += 1;
        state = self.index_waitlist(index).initialize(state, t);
        let _state = self.assert_invariants(state);
        LINK.click();
        WaitGuard {
            list: self,
            index,
            owned: true,
        }
    }

    pub fn unlink(&self, mut guard: WaitGuard<T>) {
        assert!(guard.owned, "must own the guard to explicitly unlink; it is safe to leave unlinking to the drop call");
        self._unlink(&mut guard);
    }

    fn _unlink(&self, guard: &mut WaitGuard<T>) {
        let index = guard.index;
        let notify = {
            let mut state = self.state.lock().unwrap();
            state = self.assert_invariants(state);
            let waiter = self.index_waitlist(index);
            assert!(waiter.linked.load(Ordering::Relaxed));
            waiter.linked.store(false, Ordering::SeqCst);
            while state.head < state.tail && !self.index_waitlist(state.head).linked.load(Ordering::SeqCst) {
                state = self.index_waitlist(state.head).deinitialize(state);
                state.head += 1;
            }
            state = self.assert_invariants(state);
            state.waiting_for_available > 0
        };
        if notify {
            self.wait_waiter_available.notify_one();
        }
        guard.owned = false;
        guard.index = u64::max_value();
        UNLINK.click();
    }

    /// Notify the first waiter in the list.  Notification is dropped if there is no waiter.
    pub fn notify_head(&self) {
        let mut state = self.state.lock().unwrap();
        state = self.assert_invariants(state);
        if state.head < state.tail {
            NOTIFY_HEAD.click();
            self.index_waitlist(state.head).cond.notify_one();
        } else {
            NOTIFY_HEAD_DROPPED.click();
        }
    }

    fn index_waitlist(&self, index: u64) -> &Waiter<T> {
        let index = index % (self.waiters.len() as u64);
        &self.waiters[index as usize]
    }

    // Call with the lock held.
    fn assert_invariants<'a>(&self, state: MutexGuard<'a, WaitListState>) -> MutexGuard<'a, WaitListState> {
        assert!(state.head == state.tail || self.index_waitlist(state.head).linked.load(Ordering::Relaxed));
        state
    }
}

impl<T: Clone> Default for WaitList<T> {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////// WaitGuard ////////////////////////////////////////////

/// Callers link a Waiter into the list and protect it with a WaitGuard.  The WaitGuard will panic
/// if the caller fails to unlink.
#[derive(Debug)]
pub struct WaitGuard<'a, T: Clone + 'a> {
    list: &'a WaitList<T>,
    index: u64,
    owned: bool,
}

impl<'a, T: Clone + 'a> WaitGuard<'a, T> {
    /// Iterate the list from our position forward.
    pub fn iter<'b: 'a>(&'b self) -> WaitIterator<'b, T> {
        let index = self.index;
        WaitIterator {
            guard: self,
            index,
        }
    }

    /// Return our index into the list.
    pub fn index(&mut self) -> u64 {
        self.index
    }

    /// Store a value for the WaitGuard thread to load later.
    pub fn store(&mut self, t: T) {
        let state = self.list.state.lock().unwrap();
        let _state = self.list.index_waitlist(self.index).store(state, t);
    }

    /// Load the value for the WaitGuard.
    pub fn load(&mut self) -> T {
        let state = self.list.state.lock().unwrap();
        let (_state, t) = self.list.index_waitlist(self.index).load(state);
        t
    }

    /// Swap with the current value.
    pub fn swap(&mut self, t: &mut T) {
        let state = self.list.state.lock().unwrap();
        let _state = self.list.index_waitlist(self.index).swap(state, t);
    }

    /// True iff the thread is the lowest-index thread in the system.
    pub fn is_head(&mut self) -> bool {
        let state = self.list.state.lock().unwrap();
        state.head == self.index
    }

    /// Count how many threads are in the list.  This should be used for debugging, not for logic.
    pub fn count(&mut self) -> u64 {
        let state = self.list.state.lock().unwrap();
        state.tail - state.head
    }

    /// Use the [WaitGuard] provided by `self` to get a wait guard to a later position in the list.
    /// It is not possible to get a wait guard to an index less than our own position.  This
    /// limitation enables us to enforce lifetimes with the borrow checker.  Returns None if the
    /// owner called unlink on the index.
    pub fn get_waiter<'c, 'b: 'c>(&'b mut self, index: u64) -> Option<WaitGuard<'c, T>> {
        let state = self.list.state.lock().unwrap();
        if index < self.index || index >= state.tail || !self.list.index_waitlist(index).linked.load(Ordering::Relaxed) {
            return None;
        }
        Some(WaitGuard {
            list: self.list,
            index,
            owned: false,
        })
    }

    /// Atomically unlock the guard and wait on the internal condition variable.
    pub fn naked_wait<'b, M>(&self, guard: MutexGuard<'b, M>) -> MutexGuard<'b, M> {
        self.list.index_waitlist(self.index).naked_wait(guard)
    }

    /// Wait until someone stores a value in the guard.  Note that you must always make sure that
    /// some other thread will call store on this wait guard's index.
    pub fn wait_for_store<'b, M>(&self, guard: MutexGuard<'b, M>) -> (MutexGuard<'b, M>, T) {
        self.list.index_waitlist(self.index).wait_for_store(guard)
    }

    /// Notify the waiter that it's time to wake up.
    pub fn notify(&self) {
        self.list.index_waitlist(self.index).notify()
    }
}

impl<'a, T: Clone + 'a> Drop for WaitGuard<'a, T> {
    fn drop(&mut self) {
        if self.owned {
            self.list._unlink(self)
        }
    }
}

/////////////////////////////////////////// WaitIterator ///////////////////////////////////////////

/// [WaitIterator] iteratres from the position of the provided guard forward.  At each step the
/// iterator will give a guard that lives at least as long as the [WaitIterator]'s lifetime.
#[derive(Debug)]
pub struct WaitIterator<'a, T: Clone + 'a>
{
    guard: &'a WaitGuard<'a, T>,
    index: u64,
}

impl<'a, T: Clone + 'a> WaitIterator<'a, T> {
}

impl<'a, T: Clone + 'a> Iterator for WaitIterator<'a, T> {
    type Item = WaitGuard<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        let state = self.guard.list.state.lock().unwrap();
        let index = self.index;
        if index >= state.tail {
            None
        } else {
            self.index += 1;
            Some(WaitGuard {
                list: self.guard.list,
                index,
                owned: false,
            })
        }
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn eight_waiters() {
        let wait_list: WaitList<u64> = WaitList::new();
        let waiter0 = wait_list.link(0);
        let waiter1 = wait_list.link(1);
        let waiter2 = wait_list.link(2);
        let waiter3 = wait_list.link(3);
        let waiter4 = wait_list.link(4);
        let waiter5 = wait_list.link(5);
        let waiter6 = wait_list.link(6);
        let waiter7 = wait_list.link(7);

        assert_eq!(0, waiter0.index);
        assert_eq!(1, waiter1.index);
        assert_eq!(2, waiter2.index);
        assert_eq!(3, waiter3.index);
        assert_eq!(4, waiter4.index);
        assert_eq!(5, waiter5.index);
        assert_eq!(6, waiter6.index);
        assert_eq!(7, waiter7.index);

        {
        // 0
        let mut iter = waiter0.iter();
        assert_eq!(0, iter.next().unwrap().index);
        assert_eq!(1, iter.next().unwrap().index);
        assert_eq!(2, iter.next().unwrap().index);
        assert_eq!(3, iter.next().unwrap().index);
        assert_eq!(4, iter.next().unwrap().index);
        assert_eq!(5, iter.next().unwrap().index);
        assert_eq!(6, iter.next().unwrap().index);
        assert_eq!(7, iter.next().unwrap().index);
        assert!(iter.next().is_none());

        // 1
        let mut iter = waiter1.iter();
        assert_eq!(1, iter.next().unwrap().index);
        assert_eq!(2, iter.next().unwrap().index);
        assert_eq!(3, iter.next().unwrap().index);
        assert_eq!(4, iter.next().unwrap().index);
        assert_eq!(5, iter.next().unwrap().index);
        assert_eq!(6, iter.next().unwrap().index);
        assert_eq!(7, iter.next().unwrap().index);
        assert!(iter.next().is_none());

        // 2
        let mut iter = waiter2.iter();
        assert_eq!(2, iter.next().unwrap().index);
        assert_eq!(3, iter.next().unwrap().index);
        assert_eq!(4, iter.next().unwrap().index);
        assert_eq!(5, iter.next().unwrap().index);
        assert_eq!(6, iter.next().unwrap().index);
        assert_eq!(7, iter.next().unwrap().index);
        assert!(iter.next().is_none());

        // 3
        let mut iter = waiter3.iter();
        assert_eq!(3, iter.next().unwrap().index);
        assert_eq!(4, iter.next().unwrap().index);
        assert_eq!(5, iter.next().unwrap().index);
        assert_eq!(6, iter.next().unwrap().index);
        assert_eq!(7, iter.next().unwrap().index);
        assert!(iter.next().is_none());

        // 4
        let mut iter = waiter4.iter();
        assert_eq!(4, iter.next().unwrap().index);
        assert_eq!(5, iter.next().unwrap().index);
        assert_eq!(6, iter.next().unwrap().index);
        assert_eq!(7, iter.next().unwrap().index);
        assert!(iter.next().is_none());

        // 5
        let mut iter = waiter5.iter();
        assert_eq!(5, iter.next().unwrap().index);
        assert_eq!(6, iter.next().unwrap().index);
        assert_eq!(7, iter.next().unwrap().index);
        assert!(iter.next().is_none());

        // 6
        let mut iter = waiter6.iter();
        assert_eq!(6, iter.next().unwrap().index);
        assert_eq!(7, iter.next().unwrap().index);
        assert!(iter.next().is_none());

        // 7
        let mut iter = waiter7.iter();
        assert_eq!(7, iter.next().unwrap().index);
        assert!(iter.next().is_none());
        }

        wait_list.unlink(waiter0);
        wait_list.unlink(waiter1);
        wait_list.unlink(waiter2);
        wait_list.unlink(waiter3);
        wait_list.unlink(waiter4);
        wait_list.unlink(waiter5);
        wait_list.unlink(waiter6);
        wait_list.unlink(waiter7);
    }

    #[test]
    fn load_store() {
        let wait_list: WaitList<Option<u64>> = WaitList::new();
        let mut waiter0 = wait_list.link(None);
        let mut waiter1 = wait_list.link(None);

        waiter0.store(Some(0));
        waiter1.store(Some(0));

        let mut iter = waiter0.iter();
        iter.next().unwrap().store(Some(42));
        iter.next().unwrap().store(Some(99));
        assert!(iter.next().is_none());

        let mut iter = waiter1.iter();
        iter.next().unwrap().store(Some(99));
        assert!(iter.next().is_none());

        let mut iter = waiter0.iter();
        assert_eq!(Some(42), iter.next().unwrap().load());
        assert_eq!(Some(99), iter.next().unwrap().load());
        assert!(iter.next().is_none());

        wait_list.unlink(waiter0);
        wait_list.unlink(waiter1);
    }

    #[test]
    fn wait_value() {
        let wait_list0: Arc<WaitList<Option<u64>>> = Arc::new(WaitList::new());
        let wait_list1 = Arc::clone(&wait_list0);
        let barrier0 = Arc::new(std::sync::Barrier::new(2));
        let barrier1 = Arc::clone(&barrier0);
        let mut waiter0 = wait_list0.link(None);
        let mtx = Mutex::new(());
        std::thread::spawn(move || {
            barrier1.wait();
            let mut waiter1 = wait_list1.link(None);
            barrier1.wait();
            barrier1.wait();
            assert_eq!(Some(1), waiter1.load());
            let guard = mtx.lock().unwrap();
            let _guard = waiter1.wait_for_store(guard).0;
            barrier1.wait();
            wait_list1.unlink(waiter1);
        });
        barrier0.wait();
        barrier0.wait();
        for (idx, mut guard) in waiter0.iter().enumerate() {
            guard.store(Some(idx as u64))
        }
        barrier0.wait();
        assert_eq!(Some(0), waiter0.load());
        std::thread::sleep(std::time::Duration::from_millis(100));
        for mut guard in waiter0.iter() {
            guard.store(Some(42));
        }
        barrier0.wait();
        wait_list0.unlink(waiter0)
    }
}
