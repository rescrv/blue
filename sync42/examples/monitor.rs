use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, MutexGuard};

use biometrics::{Collector, Counter};

use sync42::monitor::{Coordination, CriticalSection, Monitor};

//////////////////////////////////////////// Biomentrics ///////////////////////////////////////////

static WRITER_TAKES_IT: Counter = Counter::new("monitor.writer_takes_it");
static WRITER_LEAVES_IT: Counter = Counter::new("monitor.writer_leaves_it");
static WRITER_WAITING: Counter = Counter::new("monitor.writer_waiting");
static RELEASE_NOTIFIES: Counter = Counter::new("monitor.release_notifies");

fn register_biometrics(collector: &mut Collector) {
    collector.register_counter(&WRITER_TAKES_IT);
    collector.register_counter(&WRITER_LEAVES_IT);
    collector.register_counter(&WRITER_WAITING);
    collector.register_counter(&RELEASE_NOTIFIES);
}

////////////////////////////////////////////// Monitor /////////////////////////////////////////////

struct WriteState<'a> {
    wait_to_write: &'a Condvar,
    to_write: Option<String>,
    writes: Vec<String>,
}

struct CoalesceWrites {
    writer_waiting: bool,
    writer_has_it: bool,
    writes: Vec<String>,
}

impl Coordination<WriteState<'_>> for CoalesceWrites {
    fn acquire<'a: 'b, 'b>(
        mut guard: MutexGuard<'a, Self>,
        ws: &'b mut WriteState<'_>,
    ) -> (bool, MutexGuard<'a, Self>) {
        loop {
            if !guard.writer_has_it {
                WRITER_TAKES_IT.click();
                guard.writer_has_it = true;
                std::mem::swap(&mut guard.writes, &mut ws.writes);
                ws.writes.push(ws.to_write.take().unwrap());
                return (true, guard);
            } else if guard.writer_waiting {
                WRITER_LEAVES_IT.click();
                guard.writes.push(ws.to_write.take().unwrap());
                return (false, guard);
            } else {
                WRITER_WAITING.click();
                guard.writer_waiting = true;
                guard = ws.wait_to_write.wait(guard).unwrap();
                guard.writer_waiting = false;
            }
        }
    }

    fn release<'a: 'b, 'b>(
        mut guard: MutexGuard<'a, Self>,
        ws: &'b mut WriteState<'_>,
    ) -> MutexGuard<'a, Self> {
        guard.writer_has_it = false;
        if guard.writer_waiting {
            RELEASE_NOTIFIES.click();
            ws.wait_to_write.notify_one();
        }
        guard
    }
}

struct WriteWithMutualExclusion {}

impl WriteWithMutualExclusion {
    fn do_write(&mut self, write: String) {
        drop(write);
    }
}

impl CriticalSection<WriteState<'_>> for WriteWithMutualExclusion {
    fn critical_section<'a: 'b, 'b>(&'a mut self, ws: &'b mut WriteState) {
        let mut writes = Vec::new();
        std::mem::swap(&mut ws.writes, &mut writes);
        for write in writes.into_iter() {
            self.do_write(write);
        }
    }
}

////////////////////////////////////////////// Threads /////////////////////////////////////////////

static COUNTER: AtomicU64 = AtomicU64::new(1);
static DONE: AtomicBool = AtomicBool::new(false);
static WAIT_TO_WRITE: Condvar = Condvar::new();

fn worker_thread(monitor: Arc<Monitor<WriteState<'_>, CoalesceWrites, WriteWithMutualExclusion>>) {
    while !DONE.load(Ordering::Relaxed) {
        let num = COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut ws = WriteState {
            wait_to_write: &WAIT_TO_WRITE,
            to_write: Some(format!("seq_no={}", num)),
            writes: Vec::new(),
        };
        monitor.do_it(&mut ws);
    }
}

fn main() {
    // Setup the environment.
    std::thread::spawn(|| {
        let mut collector = biometrics::Collector::new();
        register_biometrics(&mut collector);
        sync42::register_biometrics(&mut collector);
        let fout = std::fs::File::create("/dev/stdout").unwrap();
        let mut emit = biometrics::PlainTextEmitter::new(fout);
        loop {
            if let Err(e) = collector.emit(&mut emit) {
                eprintln!("collector error: {}", e);
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
    });
    // Build the monitor.
    let coordination = CoalesceWrites {
        writer_waiting: false,
        writer_has_it: false,
        writes: Vec::new(),
    };
    let critical_section = WriteWithMutualExclusion {};
    let monitor = Arc::new(Monitor::new(coordination, critical_section));
    // Spawn the theads.
    let mut threads = Vec::new();
    for _ in 0..64 {
        let m = Arc::clone(&monitor);
        threads.push(std::thread::spawn(move || {
            worker_thread(m);
        }));
    }
    std::thread::sleep(std::time::Duration::from_millis(60_000));
    DONE.store(true, Ordering::Relaxed);
    for thread in threads.into_iter() {
        let _ = thread.join();
    }
}
