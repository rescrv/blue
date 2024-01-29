use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use sync42::wait_list::WaitList;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static HEAD_PASSBACK_INITIATED: biometrics::Counter =
    biometrics::Counter::new("playground.clicker.head_passback_initiated");
static HEAD_PASSBACK_FINISHED: biometrics::Counter =
    biometrics::Counter::new("playground.clicker.head_passback_finished");
static INCREMENT_OUTSTANDING_CALLS: biometrics::Counter =
    biometrics::Counter::new("playground.clicker.increment_outstanding_calls");
static DECREMENT_OUTSTANDING_CALLS: biometrics::Counter =
    biometrics::Counter::new("playground.clicker.decrement_outstanding_calls");
static BLOCKING_ON_TICKET_ACQUISITION: biometrics::Counter =
    biometrics::Counter::new("playground.clicker.blocking_on_ticket_acquisition");
static ABLE_TO_ACQUIRE_TICKETS: biometrics::Counter =
    biometrics::Counter::new("playground.clicker.able_to_acquire_tickets");

pub fn register_biometrics(collector: &mut biometrics::Collector) {
    sync42::register_biometrics(collector);
    collector.register_counter(&HEAD_PASSBACK_INITIATED);
    collector.register_counter(&HEAD_PASSBACK_FINISHED);
    collector.register_counter(&INCREMENT_OUTSTANDING_CALLS);
    collector.register_counter(&DECREMENT_OUTSTANDING_CALLS);
    collector.register_counter(&BLOCKING_ON_TICKET_ACQUISITION);
    collector.register_counter(&ABLE_TO_ACQUIRE_TICKETS);
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
        std::thread::sleep(std::time::Duration::from_millis(1_000));
        alloc
    }
}

////////////////////////////////////////////// Clicker /////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
enum WaitState {
    #[default]
    Present,
    CallingCount,
    Counted(u64),
}

struct ClickerState {
    outstanding_calls: u64,
    head_passback: bool,
}

impl ClickerState {
    const fn new() -> Self {
        Self {
            outstanding_calls: 0,
            head_passback: false,
        }
    }
}

struct Clicker<'a> {
    concurrent_count_calls: u64,
    counter: &'a Counter,
    state: Mutex<ClickerState>,
    wait_list: WaitList<WaitState>,
}

impl<'a> Clicker<'a> {
    fn new(concurrent_count_calls: u64, counter: &'a Counter) -> Self {
        Self {
            concurrent_count_calls,
            counter,
            state: Mutex::new(ClickerState::new()),
            wait_list: WaitList::new(),
        }
    }

    fn click(&self) -> u64 {
        let mut waiter = self.wait_list.link(WaitState::Present);
        let tickets = {
            let mut state = self.state.lock().unwrap();
            #[allow(clippy::nonminimal_bool)]
            'conditions: while !(state.outstanding_calls < self.concurrent_count_calls)
                && !(waiter.is_head() && state.head_passback)
            {
                match waiter.load() {
                    WaitState::Present => {
                        BLOCKING_ON_TICKET_ACQUISITION.click();
                        state = waiter.naked_wait(state);
                    }
                    WaitState::CallingCount => {
                        panic!("CallingCount state achieved before allowed to call count");
                    }
                    WaitState::Counted(_) => {
                        break 'conditions;
                    }
                }
                if state.outstanding_calls < self.concurrent_count_calls && !state.head_passback {
                    HEAD_PASSBACK_INITIATED.click();
                    state.head_passback = true;
                    INCREMENT_OUTSTANDING_CALLS.click();
                    state.outstanding_calls += 1;
                    self.wait_list.notify_head();
                }
            }
            if let WaitState::Counted(x) = waiter.load() {
                self.wait_list.unlink(waiter);
                self.wait_list.notify_head();
                return x;
            }
            ABLE_TO_ACQUIRE_TICKETS.click();
            let mut count = 0;
            for mut w in waiter.iter() {
                match w.load() {
                    WaitState::Present => {
                        count += 1;
                    }
                    WaitState::CallingCount | WaitState::Counted(_) => {
                        break;
                    }
                };
            }
            waiter.store(WaitState::CallingCount);
            if waiter.is_head() && state.head_passback {
                HEAD_PASSBACK_FINISHED.click();
                state.head_passback = false;
            } else {
                INCREMENT_OUTSTANDING_CALLS.click();
                state.outstanding_calls += 1;
            }
            count
        };
        let ticket_run_start = self.counter.count(tickets);
        for (mut w, ticket) in
            std::iter::zip(waiter.iter(), ticket_run_start..ticket_run_start + tickets)
        {
            w.store(WaitState::Counted(ticket))
        }
        if let WaitState::Counted(x) = waiter.load() {
            self.wait_list.unlink(waiter);
            {
                let mut state = self.state.lock().unwrap();
                if !state.head_passback {
                    HEAD_PASSBACK_INITIATED.click();
                    state.head_passback = true;
                } else {
                    state.outstanding_calls -= 1;
                    DECREMENT_OUTSTANDING_CALLS.click();
                }
            }
            self.wait_list.notify_head();
            return x;
        }
        panic!("We gave everyone except ourselves a ticket.");
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
            let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).expect("clock should never fail").as_millis().try_into().expect("millis since epoch should fit u64");
            if let Err(e) = collector.emit(&mut emit, now) {
                eprintln!("collector error: {}", e);
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
    });
    // Configure the experiment.
    const NUMBER_OF_THREADS: u64 = sync42::MAX_CONCURRENCY as u64 / 2;
    const CONCURRENT_COUNT: u64 = 1;
    // Setup the experiment.
    static COUNTER: Counter = Counter::new();
    let clicker: Arc<Clicker> = Arc::new(Clicker::new(CONCURRENT_COUNT, &COUNTER));
    // Spawn all threads.
    let mut threads = Vec::new();
    for _ in 0..NUMBER_OF_THREADS {
        let c = Arc::clone(&clicker);
        threads.push(std::thread::spawn(move || loop {
            let ticket = c.click();
            println!("{}", ticket);
            if ticket > 1_000_000 {
                break;
            }
        }));
    }
    // Join all threads.
    for thread in threads.into_iter() {
        thread.join().unwrap();
    }
}
