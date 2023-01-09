use std::backtrace::Backtrace;
use std::cell::RefCell;
use std::io::Write;
use std::fmt::{Debug, Display};
use std::rc::Rc;

use biometrics::{click, Counter};

use id::generate_id;

use util::stopwatch::Stopwatch;

use prototk::field_types::*;
use prototk::{FieldHelper, FieldType};
use prototk::Builder as ProtoTKBuilder;

use zerror::ZError;

pub const BACKTRACE_FIELD_NUMBER: u32 = prototk::LAST_FIELD_NUMBER - 1;
pub const LABEL_FIELD_NUMBER: u32 = prototk::LAST_FIELD_NUMBER - 4;
pub const STOPWATCH_FIELD_NUMBER: u32 = prototk::LAST_FIELD_NUMBER - 5;
pub const TRACE_ID_FIELD_NUMBER: u32 = prototk::LAST_FIELD_NUMBER - 6;

////////////////////////////////////////////// TraceID /////////////////////////////////////////////

generate_id!{TraceID, "trace:"}

/////////////////////////////////////////////// Trace //////////////////////////////////////////////

pub struct Trace {
    id: Option<TraceID>,
    proto: ProtoTKBuilder,
    human: String,
    stopwatch: Option<Stopwatch>,
}

impl Trace {
    pub fn from_zerr<E: Debug + Display>(label: &str, zerr: &ZError<E>) -> Self {
        let mut trace = Trace::new(label);
        trace.proto.append(&zerr.to_proto());
        trace.human += &format!("from zerror:\n{}\n", zerr);
        trace
    }

    pub fn new(label: &str) -> Self {
        click!("clue.trace.instantiations");
        // If the id is None we won't record in the finish.
        // The start call will take care of sampling.
        let mut id = None;
        TRACER.with(|t| {
            id = t.borrow_mut().start();
        });
        let trace = Self {
            id: id.clone(),
            proto: ProtoTKBuilder::default(),
            human: String::default(),
            stopwatch: None,
        };
        if let Some(trace_id) = id {
            trace.with_context::<string, LABEL_FIELD_NUMBER>("label", label)
                .with_context::<string, TRACE_ID_FIELD_NUMBER>("trace_id", &trace_id.human_readable())
        } else {
            trace.with_context::<string, LABEL_FIELD_NUMBER>("label", label)
        }
    }

    pub fn with_context<'a, F: FieldType<'a>, const N: u32>(self, field_name: &str, field_value: F::NativeType) -> Self
    where
        F: FieldType<'a> + 'a,
        F::NativeType: Clone + Display + FieldHelper<'a, F> + 'a,
    {
        if self.id.is_none() {
            click!("clue.trace.context_not_logged");
            return self
        }
        self.with_protobuf::<F, N>(field_value.clone())
            .with_human::<F::NativeType>(field_name, field_value)
    }

    pub fn with_human<'a, F: Display>(mut self, field_name: &str, field_value: F) -> Self {
        if self.id.is_none() {
            click!("clue.trace.human_not_logged");
            return self
        }
        self.human += &format!("{} = {}\n", field_name, field_value);
        self
    }

    pub fn with_protobuf<'a, F, const N: u32>(mut self, field_value: F::NativeType) -> Self
    where
        F: FieldType<'a> + 'a,
        F::NativeType: FieldHelper<'a, F> + 'a,
    {
        if self.id.is_none() {
            click!("clue.trace.protobuf_not_logged");
            return self
        }
        self.proto.push::<F, N>(field_value);
        self
    }

    pub fn with_backtrace(self) -> Self {
        if self.id.is_none() {
            click!("clue.trace.backtrace_not_logged");
            return self
        }
        click!("clue.trace.with_backtrace");
        let backtrace = format!("{}", Backtrace::force_capture());
        self.with_context::<string, BACKTRACE_FIELD_NUMBER>("backtrace", &backtrace)
    }

    pub fn with_stopwatch(mut self) -> Self {
        if self.id.is_none() {
            click!("clue.trace.stopwatch_not_logged");
            return self
        }
        click!("clue.trace.with_stopwatch");
        self.stopwatch = Some(Stopwatch::new());
        self
    }

    pub fn finish(mut self) {
        if let Some(stopwatch) = &self.stopwatch {
            let time_ms: f64 = stopwatch.since();
            self = self.with_context::<double, STOPWATCH_FIELD_NUMBER>("elapsed", time_ms);
        }
        TRACER.with(|t| {
            t.borrow_mut().finish(self);
        });
    }

    pub fn panic<S: AsRef<str>>(self, message: S) -> ! {
        let panic_extras = self.human.clone();
        self.finish();
        TRACER.with(|t| {
            t.borrow_mut().flush();
        });
        panic!("{}\n{}", message.as_ref(), panic_extras);
    }
}

////////////////////////////////////////////// Tracer //////////////////////////////////////////////

thread_local! {
    static TRACER: Rc<RefCell<Tracer>> = Rc::new(RefCell::new(Tracer::new()));
}

struct Tracer {
    emitter: Option<Rc<dyn Emitter>>,
}

impl Tracer {
    pub fn start(&mut self) -> Option<TraceID> {
        // TODO(rescrv): sampling
        match TraceID::generate() {
            Some(id) => Some(id),
            None => {
                click!("clue.trace.id_generate_failed");
                Some(TraceID::default())
            }
        }
    }

    pub fn finish(&mut self, trace: Trace) {
        if trace.id.is_none() {
            click!("clue.trace.drop");
            return;
        }
        let emitter = match &self.emitter {
            Some(e) => e,
            None => {
                click!("clue.trace.drop.no_emitter");
                return;
            }
        };
        click!("clue.trace.emit");
        emitter.emit(trace);
    }

    pub fn flush(&mut self) {
        let emitter = match &self.emitter {
            Some(e) => e,
            None => {
                click!("clue.trace.flush.no_emitter");
                return;
            }
        };
        click!("clue.trace.flush");
        emitter.flush();
    }

    const fn new() -> Self {
        Self {
            emitter: None,
        }
    }

    fn set_emitter(&mut self, emitter: Rc<dyn Emitter>) {
        self.emitter = Some(emitter);
    }
}

////////////////////////////////////////////// Emitter /////////////////////////////////////////////

pub trait Emitter {
    fn emit(&self, trace: Trace);
    fn flush(&self);
}

pub fn register_emitter<E: Emitter + 'static>(emitter: E) {
    TRACER.with(|t| {
        t.borrow_mut().set_emitter(Rc::new(emitter));
    });
}

///////////////////////////////////////// PlainTextEmitter /////////////////////////////////////////

pub struct PlainTextEmitter<W: Write> {
    fout: RefCell<W>,
}

impl<W: Write> PlainTextEmitter<W> {
    pub fn new(fout: W) -> Self {
        Self {
            fout: RefCell::new(fout),
        }
    }
}

static CLUE_PLAINTEXT_NOT_WRITTEN: Counter = Counter::new("clue.plaintext.not_written");

impl<W: Write> Emitter for PlainTextEmitter<W> {
    fn emit(&self, trace: Trace) {
        let trace_id = match trace.id {
            Some(id) => id,
            None => {
                click!("clue.plaintext.dropped");
                return;
            },
        };
        let mut fout = self.fout.borrow_mut();
        if let Err(e) = write!(fout, "TraceID: {} ===================================\n", trace_id.human_readable()) {
            CLUE_PLAINTEXT_NOT_WRITTEN.click();
            eprintln!("plaintext emitter failure: {}", e);
        }
        if let Err(e) = write!(fout, "{}\n", trace.human) {
            CLUE_PLAINTEXT_NOT_WRITTEN.click();
            eprintln!("plaintext emitter failure: {}", e);
        }
    }

    fn flush(&self) {
        // Intentionally do nothing.
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plaintext() {
        let emitter = PlainTextEmitter::new(std::io::stdout());
        register_emitter(emitter);
        let trace = Trace::new("test")
            .with_backtrace()
            .with_stopwatch()
            .with_context::<fixed64, 1>("field_one", 0x1eaff00dc0ffeeu64);
        std::thread::sleep(std::time::Duration::from_millis(250));
        trace.finish();
    }
}
