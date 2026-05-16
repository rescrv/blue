//! A RCU-like quiescent state detector.

use std::collections::{BinaryHeap, VecDeque};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

////////////////////////////////////////////// Garbage /////////////////////////////////////////////

struct Garbage {
    timestamp: u64,
    cleanup: Box<dyn FnOnce() + Send + Sync>,
}

impl Eq for Garbage {}

impl PartialEq for Garbage {
    fn eq(&self, garbage: &Garbage) -> bool {
        self.timestamp == garbage.timestamp
    }
}

impl Ord for Garbage {
    fn cmp(&self, garbage: &Garbage) -> std::cmp::Ordering {
        // NOTE(rescrv): Intentionally backwards.
        garbage.timestamp.cmp(&self.timestamp)
    }
}

impl PartialOrd for Garbage {
    fn partial_cmp(&self, garbage: &Garbage) -> Option<std::cmp::Ordering> {
        // NOTE(rescrv): Forwards to pick up the backwards.
        Some(self.cmp(garbage))
    }
}

////////////////////////////////////////// ThreadStateNode /////////////////////////////////////////

#[derive(Default)]
struct ThreadStateNode {
    quiescent_timestamp: AtomicU64,
    offline_timestamp: AtomicU64,
    collected: Mutex<BinaryHeap<Garbage>>,
    in_use: AtomicBool,
}

impl ThreadStateNode {
    fn online_timestamp(&self) -> Option<u64> {
        if !self.in_use.load(Ordering::Acquire) {
            return None;
        }
        let quiescent_timestamp = self.quiescent_timestamp.load(Ordering::Acquire);
        let offline_timestamp = self.offline_timestamp.load(Ordering::Relaxed);
        if quiescent_timestamp > offline_timestamp {
            Some(quiescent_timestamp)
        } else {
            None
        }
    }

    fn mark_offline(&self, timestamp: u64) {
        // Store offline first so an acquire read of quiescent_timestamp observes the transition.
        self.offline_timestamp.store(timestamp, Ordering::Release);
        self.quiescent_timestamp.store(timestamp, Ordering::Release);
    }

    fn mark_online(&self, timestamp: u64) {
        self.quiescent_timestamp.store(timestamp, Ordering::Release);
        self.in_use.store(true, Ordering::Release);
    }

    fn purge(&self, min_timestamp: u64) {
        let mut collected = self.collected.lock().unwrap();
        while let Some(garbage) = collected.pop() {
            if garbage.timestamp < min_timestamp {
                (garbage.cleanup)();
            } else {
                collected.push(garbage);
                return;
            }
        }
    }
}

//////////////////////////////////////////// ThreadState ///////////////////////////////////////////

/// A thread state belongs to a single thread.
pub struct ThreadState<'a> {
    collector: &'a Collector,
    index: usize,
}

impl ThreadState<'_> {
    /// Call `quiescent` regularly at a time when the thread holds no garbage-collectible pointers.
    pub fn quiescent(&mut self) {
        let (timestamp, min_timestamp) = loop {
            // This loop finds the largest timestamp that is less than each thread state's
            // quiescent_timestamp.  For every other thread, use the value that they advertise in
            // their thread_state_node.  For this thread, use the value we read from the counter.

            // read the timestamps here
            let timestamp = self.collector.read_timestamp();
            let mut min_timestamp = timestamp;
            let prev_min_timestamp = self.collector.watermark_timestamp.load(Ordering::Relaxed);

            // Find the timestamp of the last transition.  We will check this at the end and break
            // out of the loop if it remains unchanged.
            let last_online_timestamp =
                self.collector.last_online_timestamp.load(Ordering::Acquire);

            for (idx, node) in self.collector.nodes.iter().enumerate() {
                if idx != self.index {
                    if let Some(quiescent_timestamp) = node.online_timestamp() {
                        min_timestamp = std::cmp::min(quiescent_timestamp, min_timestamp);
                    } else {
                        // We purge here so that offline garbage gets collected.
                        node.purge(prev_min_timestamp);
                    }
                }
            }

            // This acts as a barrier between the read of transitions above, and the
            // read below, so that anyone who read the counter to update their
            // timestamps will show up.
            self.collector.read_timestamp();

            if last_online_timestamp == self.collector.last_online_timestamp.load(Ordering::Acquire)
            {
                break (timestamp, min_timestamp);
            }
        };

        self.collector.update_watermark(min_timestamp);
        if self.node().online_timestamp().is_some() {
            // No need to force quiescent_timestamp to be visible with a call to read_timestamp()
            // because it's strictly increasing, and seeing a lower value only delays garbage
            // collection, but cannot hurt safety.
            self.node()
                .quiescent_timestamp
                .store(timestamp, Ordering::Release);
        } else {
            self.node().mark_offline(timestamp);
        }
        self.node().purge(min_timestamp);
    }

    /// Take the thread offline for a time.  It will act as if permanently quiescent.
    pub fn offline(&mut self) {
        let timestamp = self.collector.read_timestamp();
        assert!(self.node().quiescent_timestamp.load(Ordering::Relaxed) < timestamp);
        assert!(self.node().offline_timestamp.load(Ordering::Relaxed) < timestamp);
        self.node().mark_offline(timestamp);
        self.collector.read_timestamp();
    }

    /// Take the thread online.  It will need to begin calling quiescent again.
    pub fn online(&mut self) {
        let timestamp = self.collector.read_timestamp();
        assert!(self.node().quiescent_timestamp.load(Ordering::Relaxed) < timestamp);
        assert!(self.node().offline_timestamp.load(Ordering::Relaxed) < timestamp);
        self.node().mark_online(timestamp);
        self.collector.update_last_online(timestamp);
        self.collector.read_timestamp();
    }

    /// Collect a unit of garbage once every thread calls quiescent or offline.
    pub fn collect<F: FnOnce() + Send + Sync + 'static>(&mut self, cleanup: F) {
        let timestamp = self.collector.read_timestamp();
        let cleanup = Box::new(cleanup);
        self.node()
            .collected
            .lock()
            .unwrap()
            .push(Garbage { timestamp, cleanup });
    }

    fn node(&self) -> &ThreadStateNode {
        &self.collector.nodes[self.index]
    }
}

impl Drop for ThreadState<'_> {
    fn drop(&mut self) {
        let timestamp = self.collector.read_timestamp();
        self.node().mark_offline(timestamp);
        self.collector.read_timestamp();
        self.node().in_use.store(false, Ordering::Release);
        self.collector.free.lock().unwrap().push_back(self.index);
    }
}

///////////////////////////////////////////// Collector ////////////////////////////////////////////

/// [Collector] allows for garbage collection of lock-free data structures.
pub struct Collector {
    incrementing_timestamp: AtomicU64,
    watermark_timestamp: AtomicU64,
    last_online_timestamp: AtomicU64,
    nodes: Vec<ThreadStateNode>,
    free: Mutex<VecDeque<usize>>,
}

impl Collector {
    /// Create a new collector that supports `threads` threads.
    pub fn new(threads: usize) -> Self {
        let mut nodes = Vec::new();
        let mut free = VecDeque::new();
        for i in 0..threads {
            nodes.push(ThreadStateNode::default());
            free.push_back(i);
        }
        Self {
            incrementing_timestamp: AtomicU64::new(0),
            watermark_timestamp: AtomicU64::new(0),
            last_online_timestamp: AtomicU64::new(0),
            nodes,
            free: Mutex::new(free),
        }
    }

    /// Register a thread and get the thread state.
    pub fn register_thread(&self) -> Option<ThreadState<'_>> {
        if let Some(index) = self.free.lock().unwrap().pop_front() {
            let timestamp = self.read_timestamp();
            self.nodes[index].mark_online(timestamp);
            self.update_last_online(timestamp);
            self.read_timestamp();
            let ts = ThreadState {
                collector: self,
                index,
            };
            Some(ts)
        } else {
            None
        }
    }

    fn read_timestamp(&self) -> u64 {
        self.incrementing_timestamp.fetch_add(1, Ordering::AcqRel) + 1
    }

    fn update_watermark(&self, timestamp: u64) {
        self.watermark_timestamp
            .fetch_max(timestamp, Ordering::AcqRel);
    }

    fn update_last_online(&self, timestamp: u64) {
        self.last_online_timestamp
            .fetch_max(timestamp, Ordering::AcqRel);
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn create_and_destroy() {
        let collector = Collector::new(4096);
        let thread_state = collector.register_thread();
        drop(thread_state);
    }

    #[test]
    fn collect_one_thread() {
        let collector = Collector::new(4096);
        let mut thread_state = collector.register_thread().unwrap();
        let checker = Arc::new(AtomicBool::default());
        let checkerp = Arc::clone(&checker);
        thread_state.collect(move || checkerp.store(true, Ordering::Relaxed));
        assert!(!checker.load(Ordering::Relaxed));
        thread_state.quiescent();
        assert!(checker.load(Ordering::Relaxed));
    }

    #[test]
    fn collect_two_threads() {
        let collector = Collector::new(4096);
        let mut thread_state1 = collector.register_thread().unwrap();
        let mut thread_state2 = collector.register_thread().unwrap();
        let checker = Arc::new(AtomicBool::default());
        let checkerp = Arc::clone(&checker);
        thread_state1.collect(move || checkerp.store(true, Ordering::Relaxed));
        assert!(!checker.load(Ordering::Relaxed));
        thread_state2.quiescent();
        assert!(!checker.load(Ordering::Relaxed));
        thread_state1.quiescent();
        assert!(checker.load(Ordering::Relaxed));
    }

    #[test]
    fn collection_waits_for_newly_registered_thread() {
        let collector = Collector::new(2);
        let mut thread_state1 = collector.register_thread().unwrap();
        let mut thread_state2 = collector.register_thread().unwrap();
        let checker = Arc::new(AtomicBool::default());
        let checkerp = Arc::clone(&checker);
        thread_state1.collect(move || checkerp.store(true, Ordering::Relaxed));
        thread_state1.quiescent();
        assert!(!checker.load(Ordering::Relaxed));
        thread_state2.quiescent();
        thread_state1.quiescent();
        assert!(checker.load(Ordering::Relaxed));
    }

    #[test]
    fn dropped_thread_does_not_pin_collection() {
        let collector = Collector::new(2);
        let mut thread_state1 = collector.register_thread().unwrap();
        let thread_state2 = collector.register_thread().unwrap();
        let checker = Arc::new(AtomicBool::default());
        let checkerp = Arc::clone(&checker);
        thread_state1.collect(move || checkerp.store(true, Ordering::Relaxed));
        drop(thread_state2);
        thread_state1.quiescent();
        assert!(checker.load(Ordering::Relaxed));
    }
}
