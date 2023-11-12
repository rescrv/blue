use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use sync42::collector::Collector;

struct Process {
    collector: Collector,
    done: AtomicBool,
    offline: AtomicU64,
}

impl Process {
    fn worker(self: Arc<Process>) {
        let mut thread_state = self.collector.register_thread().unwrap();
        let mut offline = self.offline.load(Ordering::Relaxed);
        while !self.done.load(Ordering::Relaxed) {
            for i in 0..128u64 {
                let x = Box::new(i);
                thread_state.collect(move || drop(x));
            }
            thread_state.quiescent();
            let o = self.offline.load(Ordering::Relaxed);
            if o < offline {
                offline = o;
                thread_state.offline();
                thread_state.online();
            }
        }
    }
}

fn main() {
    let collector = Collector::new(4096);
    let process = Arc::new(Process {
        collector,
        done: AtomicBool::new(false),
        offline: AtomicU64::new(0),
    });
    let mut threads = Vec::new();
    for _ in 0..16 {
        let proc = Arc::clone(&process);
        let thread = std::thread::spawn(|| proc.worker());
        threads.push(thread);
    }
    for i in 1..7 {
        std::thread::sleep(std::time::Duration::from_millis(10 * 1000));
        process.offline.store(i, Ordering::Relaxed);
    }
    process.done.store(true, Ordering::Relaxed);
    for thread in threads.into_iter() {
        thread.join().unwrap();
    }
}
