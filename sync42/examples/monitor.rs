use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, MutexGuard};
use std::time::SystemTime;

use biometrics::{Collector, Counter};

use sync42::monitor::{Monitor, MonitorCore};

//////////////////////////////////////////// Biomentrics ///////////////////////////////////////////

static WRITER_TAKES_IT: Counter = Counter::new("monitor.writer_takes_it");
static WRITER_LEAVES_IT: Counter = Counter::new("monitor.writer_leaves_it");
static WRITER_WAITING: Counter = Counter::new("monitor.writer_waiting");
static RELEASE_NOTIFIES: Counter = Counter::new("monitor.release_notifies");

fn register_biometrics(collector: &Collector) {
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

struct WriteWithMutualExclusion {}

impl WriteWithMutualExclusion {
    fn do_write(&mut self, write: String) {
        drop(write);
    }
}

#[derive(Default)]
struct CoalescingMonitor;

impl MonitorCore<CoalesceWrites, WriteWithMutualExclusion, WriteState<'_>> for CoalescingMonitor {
    fn acquire<'a: 'b, 'b>(
        &self,
        mut guard: MutexGuard<'a, CoalesceWrites>,
        ws: &'b mut WriteState<'_>,
    ) -> (bool, MutexGuard<'a, CoalesceWrites>) {
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
        &self,
        mut guard: MutexGuard<'a, CoalesceWrites>,
        ws: &'b mut WriteState<'_>,
    ) -> MutexGuard<'a, CoalesceWrites> {
        guard.writer_has_it = false;
        if guard.writer_waiting {
            RELEASE_NOTIFIES.click();
            ws.wait_to_write.notify_one();
        }
        guard
    }

    fn critical_section<'a: 'b, 'b>(
        &self,
        crit: &'a mut WriteWithMutualExclusion,
        ws: &'b mut WriteState,
    ) {
        let mut writes = Vec::new();
        std::mem::swap(&mut ws.writes, &mut writes);
        for write in writes.into_iter() {
            crit.do_write(write);
        }
    }
}

////////////////////////////////////////////// Threads /////////////////////////////////////////////

static COUNTER: AtomicU64 = AtomicU64::new(1);
static DONE: AtomicBool = AtomicBool::new(false);
static WAIT_TO_WRITE: Condvar = Condvar::new();

fn worker_thread(
    monitor: Arc<
        Monitor<CoalesceWrites, WriteWithMutualExclusion, WriteState<'_>, CoalescingMonitor>,
    >,
) {
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
        let collector = biometrics::Collector::new();
        register_biometrics(&collector);
        sync42::register_biometrics(&collector);
        let fout = std::fs::File::create("/dev/stdout").unwrap();
        let mut emit = biometrics::PlainTextEmitter::new(fout);
        loop {
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("clock should never fail")
                .as_millis()
                .try_into()
                .expect("millis since epoch should fit u64");
            if let Err(e) = collector.emit(&mut emit, now) {
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
    let core = CoalescingMonitor;
    let monitor = Arc::new(Monitor::new(core, coordination, critical_section));
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
