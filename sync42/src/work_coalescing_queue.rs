use std::sync::{Mutex, MutexGuard};

use biometrics::{Collector, Counter};

use crate::wait_list::WaitList;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static AWAIT_INPUT: Counter = Counter::new("sync42.work_coalescing_queue.await_input");
static AWAIT_STOLEN: Counter = Counter::new("sync42.work_coalescing_queue.await_stolen");
static SAW_OUTPUT: Counter = Counter::new("sync42.work_coalescing_queue.saw_output");
static BREAK_EARLY: Counter = Counter::new("sync42.work_coalescing_queue.break_early");

pub fn register_biometrics(collector: &Collector) {
    collector.register_counter(&AWAIT_INPUT);
    collector.register_counter(&AWAIT_STOLEN);
    collector.register_counter(&SAW_OUTPUT);
    collector.register_counter(&BREAK_EARLY);
}

///////////////////////////////////////////// WaitState ////////////////////////////////////////////

#[derive(Clone)]
enum WaitState<I: Clone, O: Clone> {
    Input(I),
    Stolen,
    Output(O),
}

////////////////////////////////////////// ConcurrentState /////////////////////////////////////////

#[derive(Default)]
struct ConcurrentState {
    doing_work: bool,
}

//////////////////////////////////////// WorkCoalescingCore ////////////////////////////////////////

pub trait WorkCoalescingCore<I: Clone, O: Clone> {
    type InputAccumulator: Default;
    type OutputIterator<'a>: Iterator<Item = O>
    where
        Self: 'a;

    fn can_batch(&self, acc: &Self::InputAccumulator, other: &I) -> bool;
    fn batch(&mut self, acc: Self::InputAccumulator, other: I) -> Self::InputAccumulator;
    fn work(&mut self, taken: usize, acc: Self::InputAccumulator) -> Self::OutputIterator<'_>;
}

//////////////////////////////////////// WorkCoalescingQueue ///////////////////////////////////////

pub struct WorkCoalescingQueue<I: Clone, O: Clone, C: WorkCoalescingCore<I, O>> {
    wait_list: WaitList<WaitState<I, O>>,
    state: Mutex<ConcurrentState>,
    core: Mutex<C>,
    _phantom_i: std::marker::PhantomData<I>,
    _phantom_o: std::marker::PhantomData<O>,
    _phantom_c: std::marker::PhantomData<C>,
}

impl<I: Clone, O: Clone, C: WorkCoalescingCore<I, O>> WorkCoalescingQueue<I, O, C> {
    pub fn new(core: C) -> Self {
        let wait_list = WaitList::new();
        let state = Mutex::default();
        let core = Mutex::new(core);
        let _phantom_i = std::marker::PhantomData;
        let _phantom_o = std::marker::PhantomData;
        let _phantom_c = std::marker::PhantomData;
        Self {
            wait_list,
            state,
            core,
            _phantom_i,
            _phantom_o,
            _phantom_c,
        }
    }

    pub fn get_core(&self) -> MutexGuard<'_, C> {
        self.core.lock().unwrap()
    }

    pub fn do_work(&self, input: I) -> O {
        let mut waiter = self.wait_list.link(WaitState::Input(input));
        let (work, mut core, taken) = {
            let mut state = self.state.lock().unwrap();
            while state.doing_work || !waiter.is_head() {
                match waiter.load() {
                    WaitState::Input(_) => {
                        AWAIT_INPUT.click();
                        state = waiter.naked_wait(state);
                    }
                    WaitState::Stolen => {
                        AWAIT_STOLEN.click();
                        state = waiter.naked_wait(state);
                    }
                    WaitState::Output(o) => {
                        SAW_OUTPUT.click();
                        self.wait_list.unlink(waiter);
                        self.wait_list.notify_head();
                        return o;
                    }
                }
            }
            assert!(!state.doing_work);
            assert!(waiter.is_head());
            match waiter.load() {
                WaitState::Input(_) => {}
                WaitState::Stolen => {
                    panic!("stolen at head of line");
                }
                WaitState::Output(o) => {
                    SAW_OUTPUT.click();
                    self.wait_list.unlink(waiter);
                    self.wait_list.notify_head();
                    return o;
                }
            }
            state.doing_work = true;
            let mut work = C::InputAccumulator::default();
            let mut core = self.core.lock().unwrap();
            let mut taken = 0;
            'waiters: for mut w in waiter.iter() {
                match w.load() {
                    WaitState::Input(input) => {
                        if core.can_batch(&work, &input) {
                            work = core.batch(work, input);
                            w.store(WaitState::Stolen);
                            taken += 1;
                        } else {
                            break 'waiters;
                        }
                    }
                    WaitState::Stolen | WaitState::Output(_) => {
                        panic!("head should never witness stolen or output");
                    }
                };
            }
            (work, core, taken)
        };
        let outputs = core.work(taken, work);
        for (mut w, out) in std::iter::zip(waiter.iter().take(taken), outputs) {
            w.store(WaitState::Output(out));
            w.notify();
        }
        if let WaitState::Output(o) = waiter.load() {
            self.wait_list.unlink(waiter);
            {
                let mut state = self.state.lock().unwrap();
                state.doing_work = false;
            }
            self.wait_list.notify_head();
            o
        } else {
            panic!("Thread gave everyone except itself an output.");
        }
    }

    pub fn into_inner(self) -> C {
        self.core.into_inner().unwrap()
    }
}
