use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::rc::Rc;
use std::cell::RefCell;

use util::generate_id;

pub const MILLIS: u64 = 1_000;
pub const SECONDS: u64 = 1_000_000;

/////////////////////////////////////////////// Event //////////////////////////////////////////////

#[derive(Clone,Debug,Eq,Ord,PartialEq,PartialOrd)]
enum Event {
    NOP,
    WatchDog { who: ProcessID },
}

impl Default for Event {
    fn default() -> Self {
        Self::NOP
    }
}

//////////////////////////////////////////// EventState ////////////////////////////////////////////

#[derive(Clone,Debug,Default,Eq,Ord,PartialEq,PartialOrd)]
struct EventState {
    when: u64,
    what: Event,
}

///////////////////////////////////////////// EventHeap ////////////////////////////////////////////

struct EventHeap {
    clock: u64,
    events: BinaryHeap<Reverse<EventState>>,
}

impl EventHeap {
    fn new() -> Self {
        Self {
            clock: 0,
            events: BinaryHeap::new(),
        }
    }

    fn push(&mut self, what: Event, how_far_in_the_future: u64) {
        let when = self.clock + how_far_in_the_future;
        let state = EventState {
            when,
            what,
        };
        self.events.push(Reverse(state));
    }

    fn pop(&mut self) -> Option<Event> {
        let Reverse(ev) = self.events.pop()?;
        self.clock = ev.when;
        Some(ev.what)
    }
}

////////////////////////////////////////////// Process /////////////////////////////////////////////

pub trait Process {
    fn pid(&self) -> ProcessID;
    fn watch_dog(&mut self, env: &mut Environment);
}

///////////////////////////////////////////// ProcessID ////////////////////////////////////////////

generate_id!{ProcessID, "net:"}

/////////////////////////////////////////// NetworkSwitch //////////////////////////////////////////

pub struct NetworkSwitch {
    links: Vec<ProcessID>,
}

impl NetworkSwitch {
    pub fn new() -> Self {
        Self {
            links: Vec::new(),
        }
    }

    pub fn connect(&mut self, who: ProcessID) {
        self.links.push(who);
    }
}

//////////////////////////////////////////// Environment ///////////////////////////////////////////

#[derive(Clone,Debug,Default)]
pub struct Environment {
    watch_dog: Option<u64>,
}

impl Environment {
    pub fn set_watch_dog(&mut self, micros: u64) {
        self.watch_dog = Some(micros);
    }
}

///////////////////////////////////////////// Simulator ////////////////////////////////////////////

pub struct Simulator {
    events: EventHeap,
    processes: Vec<Rc<RefCell<dyn Process>>>,
    switches: Vec<NetworkSwitch>,
}

impl Simulator {
    pub fn new() -> Self {
        Self {
            events: EventHeap::new(),
            processes: Vec::new(),
            switches: Vec::new(),
        }
    }

    pub fn add_process<P: 'static + Process>(&mut self, proc: P) {
        self.events.push(Event::WatchDog { who: proc.pid() }, 0);
        self.processes.push(Rc::new(RefCell::new(proc)));
    }

    pub fn add_switch(&mut self, switch: NetworkSwitch) {
        self.switches.push(switch);
    }

    pub fn run(&mut self) {
        loop {
            let ev = match self.events.pop() {
                Some(ev) => ev,
                None => {
                    break;
                }
            };
            match ev {
                Event::NOP => { self.nop(); },
                Event::WatchDog { who } => { self.watch_dog(who); },
            };
        }
    }

    pub fn nop(&mut self) {
    }

    pub fn watch_dog(&mut self, who: ProcessID) {
        let proc = self.get_process(who);
        let proc: &mut dyn Process = &mut *proc.borrow_mut();
        let mut env = self.environment();
        proc.watch_dog(&mut env);
        self.integrate(proc, env);
    }

    fn get_process(&self, who: ProcessID) -> Rc<RefCell<dyn Process>> {
        for proc in self.processes.iter() {
            if proc.borrow().pid() == who {
                return Rc::clone(proc);
            }
        }
        panic!("do not know {who}");
    }

    fn environment(&self) -> Environment {
        Environment::default()
    }

    fn integrate(&mut self, proc: &mut dyn Process, env: Environment) {
        if let Some(micros) = env.watch_dog {
            self.events.push(Event::WatchDog { who: proc.pid() }, micros);
        }
    }
}
