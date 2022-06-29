use std::cell::RefCell;
use std::rc::Rc;

use biometrics::{Counter,Moments};
use id::generate_id;
use util::stopwatch::Stopwatch;

////////////////////////////////////////////// TraceID /////////////////////////////////////////////

generate_id!{TraceID, "trace:"}

/////////////////////////////////////////////// Clue ///////////////////////////////////////////////

pub enum EnterOrExit {
    Enter { buffer: Vec<u8> },
    Exit { buffer: Vec<u8> },
}

pub struct Clue<'a> {
    pub id: TraceID,
    pub node: u64,
    pub file: &'a str,
    pub line: u32,
    pub what: &'a str,
    pub data: EnterOrExit,
}

////////////////////////////////////////////// Tracer //////////////////////////////////////////////

thread_local! {
    static TRACER: Rc<RefCell<Tracer>> = Rc::new(RefCell::new(Tracer::new()));
}

struct Tracer {
    emitter: Option<Rc<dyn Emitter>>,
    current_id: Option<TraceID>,
    current_depth: u64,
    current_node: u64,
}

impl Tracer {
    const fn new() -> Self {
        Self {
            emitter: None,
            current_id: None,
            current_depth: 0,
            current_node: 0,
        }
    }

    fn set_emitter(&mut self, emitter: Rc<dyn Emitter>) {
        self.emitter = Some(emitter);
    }

    fn click_enter(&mut self) -> (TraceID, u64) {
        let trace_id = if let Some(trace_id) = self.current_id {
            self.current_depth += 1;
            trace_id
        } else {
            let stopwatch = Stopwatch::new();
            let trace_id = match TraceID::generate() {
                Some(id) => id,
                None => {
                    TRACE_ID_FAIL.click();
                    TraceID::default()
                }
            };
            self.current_id = Some(trace_id.clone());
            TRACE_ID_GENERATE_LATENCY.add(stopwatch.since());
            self.current_depth = 1;
            self.current_node += 1;
            trace_id
        };
        (trace_id, self.current_node)
    }

    fn click_exit(&mut self) -> (TraceID, u64) {
        self.current_node += 1;
        self.current_depth -= 1;
        let trace_id = match self.current_id {
            Some(x) => x,
            None => {
                TRACE_ENTER_EXIT_MISMATCH.click();
                TraceID::default()
            },
        };
        let current_node = self.current_node;
        if self.current_depth <= 0 {
            self.current_id = None;
            self.current_node = 0;
        }
        (trace_id, current_node)
    }

    fn emitter(&self) -> Option<Rc<dyn Emitter>> {
        match self.emitter {
            Some(ref x) => Some(Rc::clone(x)),
            None => None,
        }
    }
}

/////////////////////////////////////////////// Trace //////////////////////////////////////////////

static TRACE_INSTANTIATIONS: Counter = Counter::new("clue.trace.instantiations");
static TRACE_ID_FAIL: Counter = Counter::new("clue.trace.id.not.random");
static TRACE_DROPPED: Counter = Counter::new("clue.trace.dropped");
static TRACE_EVAL_ENTER: Counter = Counter::new("clue.trace.enter.fn.eval");
static TRACE_EVAL_EXIT: Counter = Counter::new("clue.trace.exit.fn.eval");
static TRACE_ENTER_EXIT_MISMATCH: Counter = Counter::new("clue.trace.enter.exit.mismatch");

static TRACE_ID_GENERATE_LATENCY: Moments = Moments::new("clue.trace.id.generate.latency");
static ENTER_MOMENTS: Moments = Moments::new("clue.trace.enter.latency");
static EXIT_MOMENTS: Moments = Moments::new("clue.trace.exit.latency");

pub struct Trace<'a> {
    file: &'static str,
    line: u32,
    what: &'static str,
    tracer: Rc<RefCell<Tracer>>,
    enter: Option<Box<dyn FnOnce() -> Vec<u8> + 'a>>,
    exit: Option<Box<dyn FnOnce() -> Vec<u8> + 'a>>,
    marker: std::marker::PhantomData<&'a u8>,
}

impl<'a> Trace<'a> {
    pub fn new(file: &'static str, line: u32, what: &'static str) -> Self {
        TRACE_INSTANTIATIONS.click();
        let tracer: Rc<RefCell<Tracer>> = TRACER.with(|f| {
            Rc::clone(&f)
        });
        Self {
            file,
            line,
            what,
            tracer: Rc::clone(&tracer),
            enter: None,
            exit: None,
            marker: std::marker::PhantomData::default(),
        }
    }

    pub fn enter_with(mut self, enter: Box<dyn FnOnce() -> Vec<u8> + 'a>) -> Self {
        self.enter = Some(enter);
        self
    }

    pub fn exit_with(mut self, exit: Box<dyn FnOnce() -> Vec<u8> + 'a>) -> Self {
        self.exit = Some(exit);
        self
    }

    pub fn enter(&mut self) {
        let stopwatch = Stopwatch::new();
        let (id, node) = self.tracer.borrow_mut().click_enter();
        let mut clue = Clue {
            id,
            node,
            file: self.file,
            line: self.line,
            what: self.what,
            data: EnterOrExit::Enter { buffer: Vec::default() },
        };
        let emitter = self.tracer.borrow().emitter();
        if let Some(ref emitter) = emitter {
            if let Some(f) = self.enter.take() {
                TRACE_EVAL_ENTER.click();
                clue.data = EnterOrExit::Enter { buffer: f() };
            }
            emitter.emit(&clue);
            ENTER_MOMENTS.add(stopwatch.since());
        } else {
            TRACE_DROPPED.click();
        }
    }

    fn exit(&mut self) {
        let stopwatch = Stopwatch::new();
        let (id, node) = self.tracer.borrow_mut().click_exit();
        let mut clue = Clue {
            id,
            node,
            file: self.file,
            line: self.line,
            what: self.what,
            data: EnterOrExit::Exit { buffer: Vec::default() },
        };
        let emitter = self.tracer.borrow().emitter();
        if let Some(ref emitter) = emitter {
            if let Some(f) = self.exit.take() {
                TRACE_EVAL_EXIT.click();
                clue.data = EnterOrExit::Exit { buffer: f() };
            }
            emitter.emit(&clue);
            EXIT_MOMENTS.add(stopwatch.since());
        } else {
            TRACE_DROPPED.click();
        }
    }
}

impl<'a> Drop for Trace<'a> {
    fn drop(&mut self) {
        self.exit();
    }
}

/////////////////////////////////////////////// clue ///////////////////////////////////////////////

#[macro_export]
macro_rules! clue {
    ($name:literal) => {
        let mut t = $crate::Trace::new(file!(), line!(), $name);
        t.enter();
    };
    ($name:literal, $exit:expr) => {
        let mut t = $crate::Trace::new(file!(), line!(), $name)
            .exit_with(Box::new($exit));
        t.enter();
    };
    ($name:literal, $enter:expr, $exit:expr) => {
        let mut t = $crate::Trace::new(file!(), line!(), $name)
            .enter_with(Box::new($enter))
            .exit_with(Box::new($exit));
        t.enter();
    };
}

////////////////////////////////////////////// Emitter /////////////////////////////////////////////

pub trait Emitter {
    fn emit<'a>(&self, clue: &Clue<'a>);
}

pub fn register_emitter<E: 'static + Emitter>(emitter: E) {
    TRACER.with(|t| {
        t.borrow_mut().set_emitter(Rc::new(emitter));
    });
}

///////////////////////////////////////// PlainTextEmitter /////////////////////////////////////////

pub struct PlainTextEmitter {
}

impl Emitter for PlainTextEmitter {
    fn emit<'a>(&self, clue: &Clue<'a>) {
        let mode = match clue.data {
            EnterOrExit::Enter { buffer: _ } => { "enter" },
            EnterOrExit::Exit { buffer: _ } => { "exit" },
        };
        println!("{}:{}: {} {}", clue.file, clue.line, mode, clue.what);
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let emitter = PlainTextEmitter{};
        register_emitter(emitter);
        let mut t = Trace::new(file!(), line!(), "clue.it_works1")
            .enter_with(Box::new(|| "foo".as_bytes().to_vec()))
            .exit_with(Box::new(|| "bar".as_bytes().to_vec()));
        t.enter();
        let mut t = Trace::new(file!(), line!(), "clue.it_works2");
        t.enter();
        let mut t = Trace::new(file!(), line!(), "clue.it_works3");
        t.enter();
    }

    #[test]
    fn macro_works() {
        clue!{"macro.call1"};
        clue!{"macro.call2", || { Vec::new() }};
        clue!{"macro.call3", || { Vec::new() }, || { Vec::new() }};
    }

    #[test]
    fn macro_closure() {
        // Unfortunately we always have to move into a closure or deal with awkwardness.  Leave the
        // surrounding boiler-plate for the rest of people.
        let x = 5;
        let y = 0.0;
        clue!{"macro.closure", move || {
            format!("x={} y={}", x, y).as_bytes().to_vec()
        }};
    }

    #[test]
    fn macro_move() {
        let moved: Vec<u8> = Vec::new();
        clue!{"macro.move", move || {
            moved
        }};
    }
}
