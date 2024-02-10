use std::ops::Range;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use biometrics::Collector;

use sync42::work_coalescing_queue::{WorkCoalescingCore, WorkCoalescingQueue};

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static COUNT: biometrics::Counter = biometrics::Counter::new("work_coalescing_queue.count");
static CLICK: biometrics::Counter = biometrics::Counter::new("work_coalescing_queue.click");

fn register_biometrics(collector: &mut Collector) {
    sync42::register_biometrics(collector);
    collector.register_counter(&COUNT);
    collector.register_counter(&CLICK);
}

////////////////////////////////////////////// Counter /////////////////////////////////////////////

struct Counter {
    count: Mutex<u64>,
}

impl Counter {
    const fn new() -> Self {
        Self {
            count: Mutex::new(1),
        }
    }

    fn count(&self, amount: u64) -> u64 {
        let mut count = self.count.lock().unwrap();
        let alloc: u64 = *count;
        *count = count.wrapping_add(amount);
        COUNT.click();
        std::thread::sleep(std::time::Duration::from_millis(1_000));
        alloc
    }
}

//////////////////////////////////////////// ClickerCore ///////////////////////////////////////////

struct ClickerCore {
    counter: Counter,
}

impl WorkCoalescingCore<(), u64> for ClickerCore {
    type InputAccumulator = u64;
    type OutputIterator<'a> = Range<u64>;

    fn can_batch(&self, _: &u64, _: &()) -> bool {
        true
    }

    fn batch(&mut self, acc: u64, _: ()) -> Self::InputAccumulator {
        acc + 1
    }

    fn work(&mut self, taken: usize, acc: u64) -> Self::OutputIterator<'_> {
        assert_eq!(taken as u64, acc);
        let base = self.counter.count(acc);
        base..base + acc
    }
}

////////////////////////////////////////////// Clicker /////////////////////////////////////////////

struct Clicker {
    queue: WorkCoalescingQueue<(), u64, ClickerCore>,
}

impl Clicker {
    fn new(counter: Counter) -> Self {
        let queue = WorkCoalescingQueue::new(ClickerCore { counter });
        Self { queue }
    }

    fn click(&self) -> u64 {
        CLICK.click();
        self.queue.do_work(())
    }
}

/////////////////////////////////////////////// main ///////////////////////////////////////////////

fn main() {
    // Setup the environment.
    std::thread::spawn(|| {
        let mut collector = biometrics::Collector::new();
        register_biometrics(&mut collector);
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
    // Configure the experiment.
    const NUMBER_OF_THREADS: u64 = 100;
    // Setup the experiment.
    let counter = Counter::new();
    let clicker = Arc::new(Clicker::new(counter));
    // Spawn all threads.
    let mut threads = Vec::new();
    for _ in 0..NUMBER_OF_THREADS {
        let c = Arc::clone(&clicker);
        threads.push(std::thread::spawn(move || loop {
            let ticket = c.click();
            if ticket > 128_000_000 {
                break;
            }
        }));
    }
    // Join all threads.
    for thread in threads.into_iter() {
        thread.join().unwrap();
    }
}
