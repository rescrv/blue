use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard};

use biometrics::Counter;

use super::MAX_CONCURRENCY;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static NOTIFY_HEAD: Counter = Counter::new("sync42.wait_list.notify_head");
static NOTIFY_HEAD_DROPPED: Counter = Counter::new("sync42.wait_list.notify_head_dropped");
static WAITING_FOR_WAITERS: Counter = Counter::new("sync42.wait_list.waiting_for_waiters");

static LINK: Counter = Counter::new("sync42.wait_list.link");
static UNLINK: Counter = Counter::new("sync42.wait_list.unlink");

pub fn register_biometrics(collector: &mut biometrics::Collector) {
    collector.register_counter(&NOTIFY_HEAD);
    collector.register_counter(&NOTIFY_HEAD_DROPPED);
    collector.register_counter(&WAITING_FOR_WAITERS);
    collector.register_counter(&LINK);
    collector.register_counter(&UNLINK);
}

////////////////////////////////////////////// Waiter //////////////////////////////////////////////

#[derive(Debug)]
struct Waiter<T: Clone + Send + Sync + 'static> {
    cond: Condvar,
    value: Mutex<Option<T>>,
    seq_no: AtomicU64,
    linked: AtomicBool,
}

impl<T: Clone + Send + Sync + 'static> Waiter<T> {
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

    fn load<'a, M>(&self, guard: MutexGuard<'a, M>) -> (MutexGuard<'a, M>, Option<T>) {
        (guard, self.value.lock().unwrap().clone())
    }

    fn naked_wait<'a, M>(&self, guard: MutexGuard<'a, M>) -> MutexGuard<'a, M> {
        self.cond.wait(guard).unwrap()
    }

    fn wait_for_store<'a, M>(&self, mut guard: MutexGuard<'a, M>) -> (MutexGuard<'a, M>, Option<T>) {
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
    head: usize,
    tail: usize,
    waiting_for_available: u64,
}

///////////////////////////////////////////// WaitList /////////////////////////////////////////////

#[derive(Debug)]
pub struct WaitList<T: Clone + Send + Sync + 'static> {
    state: Mutex<WaitListState>,
    waiters: Vec<Waiter<T>>,
    wait_waiter_available: Condvar,
}

impl<T: Clone + Send + Sync + 'static> WaitList<T> {
    pub fn new() -> Self {
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

    pub fn link<'a, 'b: 'a>(&'b self, t: T) -> WaitGuard<'a, T> {
        let mut state = self.state.lock().unwrap();
        while state.head + self.waiters.len() <= state.tail {
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
        guard.index = usize::max_value();
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

    fn index_waitlist(&self, index: usize) -> &Waiter<T> {
        &self.waiters[index % self.waiters.len()]
    }

    // Call with the lock held.
    fn assert_invariants<'a>(&self, state: MutexGuard<'a, WaitListState>) -> MutexGuard<'a, WaitListState> {
        assert!(state.head == state.tail || self.index_waitlist(state.head).linked.load(Ordering::Relaxed));
        state
    }
}

impl<T: Clone + Send + Sync + 'static> Default for WaitList<T> {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////// WaitGuard ////////////////////////////////////////////

#[derive(Debug)]
pub struct WaitGuard<'a, T: Clone + Send + Sync + 'static> {
    list: &'a WaitList<T>,
    index: usize,
    owned: bool,
}

impl<'a, T: Clone + Send + Sync + 'static> WaitGuard<'a, T> {
    pub fn iter<'b: 'a>(&'b self) -> WaitIterator<'b, T> {
        let index = self.index;
        WaitIterator {
            guard: self,
            index,
        }
    }

    pub fn index(&mut self) -> usize {
        self.index
    }

    pub fn store(&mut self, t: T) {
        let state = self.list.state.lock().unwrap();
        let _state = self.list.index_waitlist(self.index).store(state, t);
    }

    pub fn load(&mut self) -> Option<T> {
        let state = self.list.state.lock().unwrap();
        let (_state, t) = self.list.index_waitlist(self.index).load(state);
        t
    }

    pub fn is_head(&mut self) -> bool {
        let state = self.list.state.lock().unwrap();
        state.head == self.index
    }

    pub fn count(&mut self) -> usize {
        let state = self.list.state.lock().unwrap();
        state.tail - state.head
    }

    pub fn get_waiter<'c, 'b: 'c>(&'b mut self, index: usize) -> Option<WaitGuard<'c, T>> {
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

    pub fn naked_wait<'b, M>(&self, guard: MutexGuard<'b, M>) -> MutexGuard<'b, M> {
        self.list.index_waitlist(self.index).naked_wait(guard)
    }

    pub fn wait_for_store<'b, M>(&self, guard: MutexGuard<'b, M>) -> (MutexGuard<'b, M>, Option<T>) {
        self.list.index_waitlist(self.index).wait_for_store(guard)
    }
}

impl<'a, T: Clone + Send + Sync + 'static> Drop for WaitGuard<'a, T> {
    fn drop(&mut self) {
        assert!(!self.owned || usize::max_value() == self.index, "unlink not called on dropped guard");
    }
}

/////////////////////////////////////////// WaitIterator ///////////////////////////////////////////

#[derive(Debug)]
pub struct WaitIterator<'a, T: Clone + Send + Sync + 'static>
{
    guard: &'a WaitGuard<'a, T>,
    index: usize,
}

impl<'a, T: Clone + Send + Sync + 'static> WaitIterator<'a, T> {
}

impl<'a, T: Clone + Send + Sync + 'static> Iterator for WaitIterator<'a, T> {
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

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
        assert_eq!(Some(Some(42)), iter.next().unwrap().load());
        assert_eq!(Some(Some(99)), iter.next().unwrap().load());
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
            assert_eq!(Some(Some(1)), waiter1.load());
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
        assert_eq!(Some(Some(0)), waiter0.load());
        std::thread::sleep(std::time::Duration::from_millis(100));
        for mut guard in waiter0.iter() {
            guard.store(Some(42));
        }
        barrier0.wait();
        wait_list0.unlink(waiter0)
    }
}
