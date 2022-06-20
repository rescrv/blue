use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::{Rc,Weak};

use biometrics::{Counter,Moments};
use prototk::Message;

////////////////////////////////////////////// Tracer //////////////////////////////////////////////

thread_local! {
    pub static TRACER: Rc<RefCell<Tracer>> = Rc::new(RefCell::new(Tracer::new()));
}

pub struct Tracer {
    stack: Vec<Weak<Trace>>,
}

impl Tracer {
    const fn new() -> Self {
        Self {
            stack: Vec::new(),
        }
    }
}

/////////////////////////////////////////////// Trace //////////////////////////////////////////////

pub static TRACE_INSTANTIATIONS: Counter = Counter::new("clue.trace.instantiations");
pub static ENTER_MOMENTS: Moments = Moments::new("clue.trace.enter");
pub static EXIT_MOMENTS: Moments = Moments::new("clue.trace.exit");

struct Trace {
    file: &'static str,
    line: u32,
    what: &'static str,
    tracer: Rc<RefCell<Tracer>>,
    enter: Option<Box<dyn for<'b> Fn(&'b mut Vec<u8>) -> &'b [u8]>>,
    enter_vec: Vec<u8>,
    exit: Option<Box<dyn for<'b> Fn(&'b mut Vec<u8>) -> &'b [u8]>>,
    exit_vec: Vec<u8>,
}

impl Trace {
    pub fn new(file: &'static str, line: u32, what: &'static str) -> Rc<Self> {
        let tracer: Rc<RefCell<Tracer>> = TRACER.with(|f| {
            Rc::clone(&f)
        });
        let trace = Rc::new(Self {
            file,
            line,
            what,
            tracer: Rc::clone(&tracer),
            enter: None,
            enter_vec: Vec::new(),
            exit: None,
            exit_vec: Vec::new(),
        });
        tracer.borrow_mut().stack.push(Rc::downgrade(&trace));
        trace
    }

    pub fn enter_with(mut self, enter: Box<dyn for<'b> Fn(&'b mut Vec<u8>) -> &'b [u8]>) {
        self.enter = Some(enter);
    }

    pub fn exit_with(mut self, exit: Box<dyn for<'b> Fn(&'b mut Vec<u8>) -> &'b [u8]>) {
        self.exit = Some(exit);
    }

    fn enter(&mut self) {
        // XXX
    }

    fn exit(&mut self) {
        // XXX
    }
}

impl Drop for Trace {
    fn drop(&mut self) {
        self.exit();
        self.tracer.borrow_mut().stack.pop();
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let t = Trace::new(file!(), line!(), "clue.it_works1");
        let t = Trace::new(file!(), line!(), "clue.it_works2");
        let t = Trace::new(file!(), line!(), "clue.it_works3");
    }
}
