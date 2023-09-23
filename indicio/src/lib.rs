use std::backtrace::Backtrace;
use std::fmt::Display;
use std::sync::{Arc, Mutex};

use biometrics::{Collector, Counter};

use buffertk::stack_pack;

use one_two_eight::{generate_id, generate_id_prototk, generate_id_tuple_element};

use prototk::field_types::*;
use prototk::{FieldNumber, FieldPackHelper, FieldType};

use utilz::stopwatch::Stopwatch;

///////////////////////////////////////////// Constants ////////////////////////////////////////////

pub const LABEL_FIELD_NUMBER: u32 = prototk::LAST_FIELD_NUMBER;
pub const TRACE_ID_FIELD_NUMBER: u32 = prototk::LAST_FIELD_NUMBER - 1;
pub const BACKTRACE_FIELD_NUMBER: u32 = prototk::LAST_FIELD_NUMBER - 8;
pub const STOPWATCH_FIELD_NUMBER: u32 = prototk::LAST_FIELD_NUMBER - 9;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static TRACE_INSTANTIATIONS: Counter = Counter::new("indicio.trace.instantiation");
static TRACE_ID_GENERATE_FAILED: Counter = Counter::new("indicio.trace.id_generate_failed");
static TRACE_NOT_SAMPLED: Counter = Counter::new("indicio.trace.not_sampled");
static TRACE_DROPPED: Counter = Counter::new("indicio.trace.dropped");
static TRACE_EMIT: Counter = Counter::new("indicio.trace.emit");
static TRACE_FLUSH: Counter = Counter::new("indicio.trace.flush");
static TRACE_WITH_VALUE: Counter = Counter::new("indicio.trace.with_value");
static TRACE_WITH_BACKTRACE: Counter = Counter::new("indicio.trace.with_backtrace");
static TRACE_WITH_STOPWATCH: Counter = Counter::new("indicio.trace.with_stopwatch");

pub fn register_biometrics(collector: &mut Collector) {
    collector.register_counter(&TRACE_INSTANTIATIONS);
    collector.register_counter(&TRACE_ID_GENERATE_FAILED);
    collector.register_counter(&TRACE_NOT_SAMPLED);
    collector.register_counter(&TRACE_DROPPED);
    collector.register_counter(&TRACE_EMIT);
    collector.register_counter(&TRACE_FLUSH);
    collector.register_counter(&TRACE_WITH_VALUE);
    collector.register_counter(&TRACE_WITH_BACKTRACE);
    collector.register_counter(&TRACE_WITH_STOPWATCH);
}

////////////////////////////////////////////// TraceID /////////////////////////////////////////////

generate_id! {TraceID, "trace:"}
generate_id_prototk! {TraceID}
generate_id_tuple_element! {TraceID}

/////////////////////////////////////////////// Trace //////////////////////////////////////////////

pub struct Trace {
    id: Option<TraceID>,
    stopwatch: Option<Stopwatch>,
    value: Vec<u8>,
}

impl Trace {
    pub fn new(label: &str) -> Self {
        TRACE_INSTANTIATIONS.click();
        // If the id is None we won't record in the finish.
        // The start call will take care of sampling.
        let mut id = None;
        TRACER.with(|t| {
            id = t.lock().unwrap().start();
        });
        let trace = Self {
            id,
            stopwatch: None,
            value: Vec::new(),
        };
        let trace = trace.with_value::<string, LABEL_FIELD_NUMBER>(label);
        if let Some(trace_id) = id {
            trace.with_value::<string, TRACE_ID_FIELD_NUMBER>(&trace_id.human_readable())
        } else {
            trace
        }
    }

    pub fn with_value<'a, F: FieldType<'a>, const N: u32>(
        mut self,
        field_value: F::Native,
    ) -> Self
    where
        F: FieldType<'a> + 'a,
        F::Native: Clone + Display + FieldPackHelper<'a, F> + 'a,
    {
        if self.id.is_none() {
            return self;
        }
        TRACE_WITH_VALUE.click();
        stack_pack(F::field_packer(FieldNumber::must(N), &field_value)).append_to_vec(&mut self.value);
        self
    }

    pub fn with_backtrace(self) -> Self {
        if self.id.is_none() {
            return self;
        }
        TRACE_WITH_BACKTRACE.click();
        let backtrace = format!("{}", Backtrace::force_capture());
        self.with_value::<string, BACKTRACE_FIELD_NUMBER>(&backtrace)
    }

    pub fn with_stopwatch(mut self) -> Self {
        if self.id.is_none() {
            return self;
        }
        TRACE_WITH_STOPWATCH.click();
        self.stopwatch = Some(Stopwatch::default());
        self
    }

    pub fn finish(mut self) {
        if let Some(stopwatch) = &self.stopwatch {
            let time_ms: f64 = stopwatch.since();
            self = self.with_value::<double, STOPWATCH_FIELD_NUMBER>(time_ms);
        }
        TRACER.with(|t| {
            t.lock().unwrap().finish(self);
        });
    }

    pub fn panic<S: AsRef<str>>(self, msg: S) -> ! {
        self.finish();
        TRACER.with(|t| {
            t.lock().unwrap().flush();
        });
        panic!("{}\n", msg.as_ref());
    }

    pub fn id(&self) -> Option<TraceID> {
        self.id
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.value
    }
}

////////////////////////////////////////////// Tracer //////////////////////////////////////////////

thread_local! {
    static TRACER: Mutex<Tracer> = Mutex::new(Tracer::new());
}

struct Tracer {
    emitter: Option<Arc<dyn Emitter>>,
}

impl Tracer {
    pub fn start(&mut self) -> Option<TraceID> {
        // TODO(rescrv): sampling
        match TraceID::generate() {
            Some(id) => Some(id),
            None => {
                TRACE_ID_GENERATE_FAILED.click();
                Some(TraceID::default())
            }
        }
    }

    pub fn finish(&mut self, trace: Trace) {
        if trace.id.is_none() {
            TRACE_NOT_SAMPLED.click();
            return;
        }
        let emitter = match &self.emitter {
            Some(e) => e,
            None => {
                TRACE_DROPPED.click();
                return;
            }
        };
        TRACE_EMIT.click();
        emitter.emit(trace);
    }

    pub fn flush(&mut self) {
        let emitter = match &self.emitter {
            Some(e) => e,
            None => {
                TRACE_DROPPED.click();
                return;
            }
        };
        TRACE_FLUSH.click();
        emitter.flush();
    }

    const fn new() -> Self {
        Self { emitter: None }
    }

    fn set_emitter(&mut self, emitter: Arc<dyn Emitter>) {
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
        t.lock().unwrap().set_emitter(Arc::new(emitter));
    });
}
