use std::sync::Arc;

use sync42::spin_lock::SpinLock;

fn worker(counter: Arc<SpinLock<u64>>) {
    loop {
        let mut x = counter.lock();
        if *x > 100000000 {
            return;
        }
        *x += 1;
    }
}

fn main() {
    // Configure the experiment.
    const NUMBER_OF_THREADS: u64 = 4;
    // Spawn all threads.
    let mut threads = Vec::new();
    let counter = Arc::new(SpinLock::new(0u64));
    for _ in 0..NUMBER_OF_THREADS {
        let c = Arc::clone(&counter);
        threads.push(std::thread::spawn(move || {
            worker(c);
        }));
    }
    // Join all threads.
    for thread in threads.into_iter() {
        thread.join().unwrap();
    }
}
