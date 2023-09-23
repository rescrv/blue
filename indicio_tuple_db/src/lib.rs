use arrrg_derive::CommandLine;

use biometrics::Counter;

use buffertk::stack_pack;

use indicio::Emitter as EmitterTrait;
use indicio::{Trace, TraceID};

use one_two_eight::{generate_id, generate_id_tuple_element};

use prototk::Message;
use prototk::field_types::*;
use prototk_derive::Message;

use sst::Builder;
use sst::ingest::{IngestOptions, Jester};

use tatl::{HeyListen, Stationary};

use tuple_key::{TupleKey, TypedTupleKey};
use tuple_key_derive::TypedTupleKey;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static EMIT_SUCCESS: Counter = Counter::new("indicio.tuple_db.emit");
static EMIT_FAILURE: Counter = Counter::new("indicio.tuple_db.emit.failure");
static EMIT_FAILURE_MONITOR: Stationary = Stationary::new("indicio.tuple_db.emit.failure", &EMIT_FAILURE);

static FLUSH_FAILURE: Counter = Counter::new("indicio.tuple_db.flush.failure");
static FLUSH_FAILURE_MONITOR: Stationary = Stationary::new("indicio.tuple_db.flush.failure", &FLUSH_FAILURE);

static TRACE_DROPPED_NO_ID: Counter = Counter::new("indicio.tuple_db.trace_dropped.no_id");

/// Register all biometrics for the crate.
pub fn register_biometrics(collector: &biometrics::Collector) {
    collector.register_counter(&EMIT_SUCCESS);
    collector.register_counter(&EMIT_FAILURE);
    collector.register_counter(&FLUSH_FAILURE);
    collector.register_counter(&TRACE_DROPPED_NO_ID);
}

/// Register all monitors for the crate.
pub fn register_monitors(hey_listen: &mut HeyListen) {
    hey_listen.register_stationary(&EMIT_FAILURE_MONITOR);
}

/////////////////////////////////////////////// Keys ///////////////////////////////////////////////

#[derive(Clone, Debug, Default, TypedTupleKey)]
struct ByTimestamp {
    #[tuple_key(1)]
    table: IndicioTableID,
    #[tuple_key(1)]
    timestamp_micros: u64,
    #[tuple_key(1)]
    trace_id: TraceID,
}

#[derive(Clone, Debug, Default, TypedTupleKey)]
struct ByTraceID {
    #[tuple_key(1)]
    table: IndicioTableID,
    #[tuple_key(1)]
    trace_id: TraceID,
    #[tuple_key(1)]
    timestamp_micros: u64,
}

#[derive(Clone, Debug, Default, Message)]
struct Empty {
}

////////////////////////////////////////////// Writer //////////////////////////////////////////////

struct Writer {
    jester: Jester,
}

impl Writer {
    fn new(options: IngestOptions) -> Self {
        Self {
            jester: Jester::new(options),
        }
    }

    fn emit_message<'a, K: TypedTupleKey, V: Message<'a>>(
        &mut self,
        key: K,
        timestamp_micros: u64,
        value: V,
    ) -> Result<(), sst::Error> {
        let tk: TupleKey = key.into();
        let value = stack_pack(value).to_vec();
        self.jester.put(tk.as_bytes(), timestamp_micros / 1000, &value)
    }

    fn emit_trace<'a, K: TypedTupleKey>(
        &mut self,
        key: K,
        timestamp_micros: u64,
        trace: Trace,
    ) -> Result<(), sst::Error> {
        let tk: TupleKey = key.into();
        self.jester.put(tk.as_bytes(), timestamp_micros / 1000, trace.as_bytes())
    }

    fn flush(&mut self) -> Result<(), sst::Error> {
        self.jester.flush()
    }
}

////////////////////////////////////////////// TableID /////////////////////////////////////////////

generate_id!{IndicioTableID, "tracing:"}
generate_id_tuple_element!{IndicioTableID}

////////////////////////////////////////// IndicioOptions //////////////////////////////////////////

#[derive(Clone, CommandLine, Debug, Default, Eq, PartialEq)]
pub struct IndicioOptions {
    #[arrrg(optional, "Emit tracing to this table.")]
    table: IndicioTableID,
    #[arrrg(nested)]
    ingest: IngestOptions,
}

////////////////////////////////////////////// Emitter /////////////////////////////////////////////

struct Emitter {
    table: IndicioTableID,
    writer: Writer,
}

impl Emitter {
    pub fn new(options: IndicioOptions) -> Self {
        Self {
            table: options.table,
            writer: Writer::new(options.ingest.clone()),
        }
    }
}

impl EmitterTrait for Emitter {
    fn emit(&self, trace: Trace) {
        let trace_id = match trace.id() {
            Some(trace_id) => trace_id,
            None => {
                TRACE_DROPPED_NO_ID.click();
                return;
            },
        };
        let timestamp_micros = 0;
        let by_timestamp = ByTimestamp {
            table: self.table,
            timestamp_micros,
            trace_id,
        };
        let by_trace_id = ByTraceID {
            table: self.table,
            trace_id,
            timestamp_micros,
        };
        match self.writer.emit_message(by_timestamp, timestamp_micros, Empty{}) {
            Ok(_) => {},
            Err(err) => {
                EMIT_FAILURE.click();
                // NOTE(rescrv): We drop the error; we won't be able to emit it anyway.
                return;
            }
        }
        match self.writer.emit_trace(by_trace_id, timestamp_micros, trace) {
            Ok(_) => {},
            Err(err) => {
                EMIT_FAILURE.click();
                // NOTE(rescrv): Second verse, same as the first.
                return;
            }
        }
    }

    fn flush(&self) {
        if let Some(err) = self.writer.flush() {
            FLUSH_FAILURE.click();
        }
    }
}
