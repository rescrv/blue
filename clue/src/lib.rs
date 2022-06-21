#[macro_use]
extern crate util;

use std::cell::RefCell;
use std::rc::Rc;

use biometrics::{Counter,Moments};
use util::stopwatch::Stopwatch;

////////////////////////////////////////////// TraceID /////////////////////////////////////////////

util::generate_id!{TraceID, "trace:"}

/////////////////////////////////////////////// Clue ///////////////////////////////////////////////

pub enum EnterOrExit<'a> {
    Enter { buffer: &'a [u8] },
    Exit { buffer: &'a [u8] },
}

pub struct Clue<'a> {
    pub id: TraceID,
    pub node: u64,
    pub file: &'a str,
    pub line: u32,
    pub what: &'a str,
    pub data: EnterOrExit<'a>,
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

pub struct Trace {
    file: &'static str,
    line: u32,
    what: &'static str,
    tracer: Rc<RefCell<Tracer>>,
    enter: Option<Box<dyn for<'b> Fn(&'b mut Vec<u8>) -> &'b [u8]>>,
    exit: Option<Box<dyn for<'b> Fn(&'b mut Vec<u8>) -> &'b [u8]>>,
}

impl Trace {
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
        }
    }

    pub fn enter_with(mut self, enter: Box<dyn for<'b> Fn(&'b mut Vec<u8>) -> &'b [u8]>) -> Self {
        self.enter = Some(enter);
        self
    }

    pub fn exit_with(mut self, exit: Box<dyn for<'b> Fn(&'b mut Vec<u8>) -> &'b [u8]>) -> Self {
        self.exit = Some(exit);
        self
    }

    pub fn enter(&self) {
        let stopwatch = Stopwatch::new();
        let (id, node) = self.tracer.borrow_mut().click_enter();
        let mut clue = Clue {
            id,
            node,
            file: self.file,
            line: self.line,
            what: self.what,
            data: EnterOrExit::Enter { buffer: &[] },
        };
        let emitter = self.tracer.borrow().emitter();
        if let Some(ref emitter) = emitter {
            let mut data = Vec::new();
            if let &Some(ref f) = &self.enter {
                TRACE_EVAL_ENTER.click();
                clue.data = EnterOrExit::Enter { buffer: f(&mut data) };
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
            data: EnterOrExit::Exit { buffer: &[] },
        };
        let emitter = self.tracer.borrow().emitter();
        if let Some(ref emitter) = emitter {
            let mut data = Vec::new();
            if let &Some(ref f) = &self.exit {
                TRACE_EVAL_EXIT.click();
                clue.data = EnterOrExit::Exit { buffer: f(&mut data) };
            }
            emitter.emit(&clue);
            EXIT_MOMENTS.add(stopwatch.since());
        } else {
            TRACE_DROPPED.click();
        }
    }
}

impl Drop for Trace {
    fn drop(&mut self) {
        self.exit();
    }
}

////////////////////////////////////////////// Emitter /////////////////////////////////////////////

pub trait Emitter {
    fn emit<'a>(&self, clue: &Clue<'a>);
}

///////////////////////////////////////// PlainTextEmitter /////////////////////////////////////////

pub struct PlainTextEmitter {
}

impl Emitter for PlainTextEmitter {
    fn emit<'a>(&self, clue: &Clue<'a>) {
        let mode = match clue.data {
            EnterOrExit::Enter { buffer } => { "enter" },
            EnterOrExit::Exit { buffer } => { "exit" },
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
        TRACER.with(|t| {
            t.borrow_mut().emitter = Some(Rc::new(emitter));
        });
        let t = Trace::new(file!(), line!(), "clue.it_works1")
            .enter_with(Box::new(|_v| "foo".as_bytes()))
            .exit_with(Box::new(|_v| "bar".as_bytes()));
        t.enter();
        let t = Trace::new(file!(), line!(), "clue.it_works2");
        t.enter();
        let t = Trace::new(file!(), line!(), "clue.it_works3");
        t.enter();
    }
}
