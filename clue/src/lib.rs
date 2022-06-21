use std::cell::RefCell;
use std::rc::Rc;

use biometrics::{Counter,Moments};

/////////////////////////////////////////////// Clue ///////////////////////////////////////////////

pub struct Clue<'a> {
    pub file: &'a str,
    pub line: u32,
    pub what: &'a str,
    pub enter: &'a [u8],
    pub exit: &'a [u8],
}

////////////////////////////////////////////// Tracer //////////////////////////////////////////////

thread_local! {
    static TRACER: Rc<RefCell<Tracer>> = Rc::new(RefCell::new(Tracer::new()));
}

struct Tracer {
    emitter: Option<Rc<dyn Emitter>>,
}

impl Tracer {
    #[allow(dead_code)]
    const fn new() -> Self {
        Self {
            emitter: None,
        }
    }

    #[allow(dead_code)]
    fn set_emitter(&mut self, emitter: Rc<dyn Emitter>) {
        self.emitter = Some(emitter);
    }

    fn emitter(&self) -> Option<Rc<dyn Emitter>> {
        match self.emitter {
            Some(ref x) => Some(Rc::clone(x)),
            None => None,
        }
    }
}

/////////////////////////////////////////////// Trace //////////////////////////////////////////////

#[allow(dead_code)]
static TRACE_INSTANTIATIONS: Counter = Counter::new("clue.trace.instantiations");
#[allow(dead_code)]
static ENTER_MOMENTS: Moments = Moments::new("clue.trace.enter");
#[allow(dead_code)]
static EXIT_MOMENTS: Moments = Moments::new("clue.trace.exit");

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

    fn clue<'a>(&self) -> Clue<'a> {
        Clue {
            file: self.file,
            line: self.line,
            what: self.what,
            enter: &[],
            exit: &[],
        }
    }

    pub fn enter(&self) {
        let mut clue = self.clue();
        let emitter = self.tracer.borrow().emitter();
        if let Some(ref emitter) = emitter {
            let mut data = Vec::new();
            if let &Some(ref f) = &self.enter {
                clue.enter = f(&mut data);
            }
            emitter.emit(&clue)
        }
    }

    fn exit(&mut self) {
        let mut clue = self.clue();
        let emitter = self.tracer.borrow().emitter();
        if let Some(ref emitter) = emitter {
            let mut data = Vec::new();
            if let &Some(ref f) = &self.exit {
                clue.exit = f(&mut data);
            }
            emitter.emit(&clue)
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
        println!("{}:{}: {}", clue.file, clue.line, clue.what);
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
