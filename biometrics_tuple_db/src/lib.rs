use std::cmp::Reverse;
use std::collections::hash_map::Entry;
use std::collections::HashMap;

use biometrics::Emitter as EmitterTrait;
use biometrics::{Counter, Gauge, Moments, Sensor};

use biometrics_pb::{CounterPb, GaugePb, MomentsPb, SensorID};

use buffertk::stack_pack;

use indicio::Trace;

use prototk::Message;
use prototk::field_types::*;
use prototk_derive::Message;

use sst::Builder;
use sst::ingest::{IngestOptions, Jester};

use tuple_key::{TupleKey, TypedTupleKey};
use tuple_key_derive::TypedTupleKey;

///////////////////////////////////////////// Constants ////////////////////////////////////////////

const WINDOW_SIZE_MS: u64 = 3600 * 1000;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static SENSOR_ID_GENERATE_FAILURE: Counter =
    Counter::new("biometrics.tuple_db.sensor_id_generate_failure");
static EMIT_ROOT_FAILURE: Counter = Counter::new("biometrics.tuple_db.emit_root_failure");
static EMIT_MAX_FAILURE: Counter = Counter::new("biometrics.tuple_db.emit_max_failure");
static EMIT_READING_FAILURE: Counter = Counter::new("biometrics.tuple_db.emit_reading_failure");

/////////////////////////////////////////////// Root ///////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
struct Root {
    #[prototk(1, uint64)]
    start_ms: u64,
}

//////////////////////////////////////////// SensorRoot ////////////////////////////////////////////

trait SensorRoot {
    fn new(table: [u8; 16], label: &'static str, sensor_id: SensorID) -> Self;
}

///////////////////////////////////////////// SensorMax ////////////////////////////////////////////

trait SensorMax {
    fn new(table: [u8; 16], label: &'static str, sensor_id: SensorID) -> Self;
}

/////////////////////////////////////////// Counter Types //////////////////////////////////////////

#[derive(Clone, Debug, Default, TypedTupleKey)]
struct CounterRoot {
    #[tuple_key(1)]
    table: [u8; 16],
    #[tuple_key(1)]
    label: String,
    #[tuple_key(1)]
    sensor_id: SensorID,
}

impl SensorRoot for CounterRoot {
    fn new(table: [u8; 16], label: &'static str, sensor_id: SensorID) -> Self {
        Self {
            table,
            label: label.to_owned(),
            sensor_id,
        }
    }
}

#[derive(Clone, Debug, Default, TypedTupleKey)]
struct CounterMax {
    #[tuple_key(1)]
    table: [u8; 16],
    #[tuple_key(1)]
    label: String,
    #[tuple_key(1)]
    sensor_id: SensorID,
    #[tuple_key(1)]
    #[allow(dead_code)] // never read, used in TypedTupleKey
    unit: (),
}

impl SensorMax for CounterMax {
    fn new(table: [u8; 16], label: &'static str, sensor_id: SensorID) -> Self {
        Self {
            table,
            label: label.to_owned(),
            sensor_id,
            unit: (),
        }
    }
}

#[derive(Clone, Debug, Default, TypedTupleKey)]
struct CounterReading {
    #[tuple_key(1)]
    table: [u8; 16],
    #[tuple_key(2)]
    sensor_id: SensorID,
    #[tuple_key(1)]
    time_ms: Reverse<u64>,
}

//////////////////////////////////////////// Gauge Types ///////////////////////////////////////////

#[derive(Clone, Debug, Default, TypedTupleKey)]
struct GaugeRoot {
    #[tuple_key(1)]
    table: [u8; 16],
    #[tuple_key(3)]
    label: String,
    #[tuple_key(1)]
    sensor_id: SensorID,
}

impl SensorRoot for GaugeRoot {
    fn new(table: [u8; 16], label: &'static str, sensor_id: SensorID) -> Self {
        Self {
            table,
            label: label.to_owned(),
            sensor_id,
        }
    }
}

#[derive(Clone, Debug, Default, TypedTupleKey)]
struct GaugeMax {
    #[tuple_key(1)]
    table: [u8; 16],
    #[tuple_key(3)]
    label: String,
    #[tuple_key(1)]
    sensor_id: SensorID,
    #[tuple_key(1)]
    #[allow(dead_code)] // never read, used in TypedTupleKey
    unit: (),
}

impl SensorMax for GaugeMax {
    fn new(table: [u8; 16], label: &'static str, sensor_id: SensorID) -> Self {
        Self {
            table,
            label: label.to_owned(),
            sensor_id,
            unit: (),
        }
    }
}

#[derive(Clone, Debug, Default, TypedTupleKey)]
struct GaugeReading {
    #[tuple_key(1)]
    table: [u8; 16],
    #[tuple_key(4)]
    sensor_id: SensorID,
    #[tuple_key(1)]
    time_ms: Reverse<u64>,
}

/////////////////////////////////////////// Moments Types //////////////////////////////////////////

#[derive(Clone, Debug, Default, TypedTupleKey)]
struct MomentsRoot {
    #[tuple_key(1)]
    table: [u8; 16],
    #[tuple_key(5)]
    label: String,
    #[tuple_key(1)]
    sensor_id: SensorID,
}

impl SensorRoot for MomentsRoot {
    fn new(table: [u8; 16], label: &'static str, sensor_id: SensorID) -> Self {
        Self {
            table,
            label: label.to_owned(),
            sensor_id,
        }
    }
}

#[derive(Clone, Debug, Default, TypedTupleKey)]
struct MomentsMax {
    #[tuple_key(1)]
    table: [u8; 16],
    #[tuple_key(5)]
    label: String,
    #[tuple_key(1)]
    sensor_id: SensorID,
    #[tuple_key(1)]
    #[allow(dead_code)] // never read, used in TypedTupleKey
    unit: (),
}

impl SensorMax for MomentsMax {
    fn new(table: [u8; 16], label: &'static str, sensor_id: SensorID) -> Self {
        Self {
            table,
            label: label.to_owned(),
            sensor_id,
            unit: (),
        }
    }
}

#[derive(Clone, Debug, Default, TypedTupleKey)]
struct MomentsReading {
    #[tuple_key(1)]
    table: [u8; 16],
    #[tuple_key(6)]
    sensor_id: SensorID,
    #[tuple_key(1)]
    time_ms: Reverse<u64>,
}

////////////////////////////////////////// SensorsByLabel //////////////////////////////////////////

#[derive(Default)]
struct SensorsByLabel {
    sensors: HashMap<&'static str, SensorID>,
}

impl SensorsByLabel {
    fn get<ROOT: SensorRoot + TypedTupleKey>(&mut self, table: [u8; 16], label: &'static str, now_millis: u64, writer: &mut Writer) -> Option<SensorID> {
        match self.sensors.entry(label) {
            Entry::Occupied(occupied) => Some(*occupied.get()),
            Entry::Vacant(vacant) => {
                let sensor_id = match SensorID::generate() {
                    Some(sensor_id) => sensor_id,
                    None => {
                        SENSOR_ID_GENERATE_FAILURE.click();
                        Trace::new("biometrics.tuple_db.generate_sensor_id_failure")
                            .finish();
                        return None;
                    }
                };
                vacant.insert(sensor_id);
                let root_key = ROOT::new(table, label, sensor_id);
                let root_msg = Root {
                    start_ms: now_millis,
                };
                if let Err(err) = writer.emit_message(root_key, now_millis, root_msg) {
                    EMIT_ROOT_FAILURE.click();
                    Trace::new("biometrics.tuple_db.root_error")
                        .with_value::<message<sst::Error>, 1>(err)
                        .finish();
                }
                Some(sensor_id)
            }
        }
    }
}

////////////////////////////////////////// SensorLastSeen //////////////////////////////////////////

#[derive(Default)]
struct SensorLastSeen {
    last_seen: HashMap<SensorID, u64>,
}

impl SensorLastSeen {
    fn update<MAX: SensorMax + TypedTupleKey>(&mut self, table: [u8; 16], label: &'static str, sensor_id: SensorID, now_millis: u64, writer: &mut Writer) {
        let last_seen = self.last_seen.entry(sensor_id).or_insert(0);
        if *last_seen < now_millis {
            let valid_through = now_millis + WINDOW_SIZE_MS;
            self.last_seen.insert(sensor_id, valid_through);
            let max = MAX::new(table, label, sensor_id);
            if let Err(err) = writer.emit_uint64(max, now_millis, valid_through) {
                EMIT_MAX_FAILURE.click();
                Trace::new("biometrics.tuple_db.max_error")
                    .with_value::<message<sst::Error>, 1>(err)
                    .finish();
            }
        }
    }
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
        timestamp: u64,
        value: V,
    ) -> Result<(), sst::Error> {
        let tk: TupleKey = key.into();
        let value = stack_pack(value).to_vec();
        self.jester.put(tk.as_bytes(), timestamp, &value)
    }

    fn emit_uint64<K: TypedTupleKey>(
        &mut self,
        key: K,
        timestamp: u64,
        value: u64,
    ) -> Result<(), sst::Error> {
        let tk: TupleKey = key.into();
        let value = stack_pack(uint64(value)).to_vec();
        self.jester.put(tk.as_bytes(), timestamp, &value)
    }
}

////////////////////////////////////////////// Emitter /////////////////////////////////////////////

pub struct Emitter {
    table: [u8; 16],
    writer: Writer,
    counters: SensorsByLabel,
    counter_last_seen: SensorLastSeen,
    gauges: SensorsByLabel,
    gauge_last_seen: SensorLastSeen,
    moments: SensorsByLabel,
    moments_last_seen: SensorLastSeen,
}

impl Emitter {
    pub fn new(options: IngestOptions, table: [u8; 16]) -> Self {
        Self {
            table,
            writer: Writer::new(options),
            counters: SensorsByLabel::default(),
            counter_last_seen: SensorLastSeen::default(),
            gauges: SensorsByLabel::default(),
            gauge_last_seen: SensorLastSeen::default(),
            moments: SensorsByLabel::default(),
            moments_last_seen: SensorLastSeen::default(),
        }
    }

    fn emit_reading<'a, K: TypedTupleKey, V: Message<'a>>(
        &mut self,
        key: K,
        timestamp: u64,
        value: V,
    ) {
        match self.writer.emit_message(key, timestamp, value) {
            Ok(_) => {},
            Err(err) => {
                EMIT_READING_FAILURE.click();
                Trace::new("biometrics.tuple_db.counter.emit_error")
                    .with_value::<message<sst::Error>, 1>(err.clone())
                    .finish();
            }
        }
    }
}

impl EmitterTrait for Emitter {
    type Error = sst::Error;

    fn emit_counter(&mut self, counter: &'static Counter, now_millis: u64) -> Result<(), Self::Error> {
        let sensor_id = match self.counters.get::<CounterRoot>(self.table, counter.label(), now_millis, &mut self.writer) {
            Some(sensor_id) => sensor_id,
            None => {
                return Ok(());
            },
        };
        self.counter_last_seen.update::<CounterMax>(self.table, counter.label(), sensor_id, now_millis, &mut self.writer);
        let reading_key = CounterReading {
            table: self.table,
            sensor_id,
            time_ms: Reverse(now_millis),
        };
        let reading_value: CounterPb = counter.read().into();
        self.emit_reading(reading_key, now_millis, reading_value);
        Ok(())
    }

    fn emit_gauge(&mut self, gauge: &'static Gauge, now_millis: u64) -> Result<(), Self::Error> {
        let sensor_id = match self.gauges.get::<GaugeRoot>(self.table, gauge.label(), now_millis, &mut self.writer) {
            Some(sensor_id) => sensor_id,
            None => {
                return Ok(());
            },
        };
        self.gauge_last_seen.update::<GaugeMax>(self.table, gauge.label(), sensor_id, now_millis, &mut self.writer);
        let reading_key = GaugeReading {
            table: self.table,
            sensor_id,
            time_ms: Reverse(now_millis),
        };
        let reading_value: GaugePb = gauge.read().into();
        self.emit_reading(reading_key, now_millis, reading_value);
        Ok(())
    }

    fn emit_moments(&mut self, moments: &'static Moments, now_millis: u64) -> Result<(), Self::Error> {
        let sensor_id = match self.moments.get::<MomentsRoot>(self.table, moments.label(), now_millis, &mut self.writer) {
            Some(sensor_id) => sensor_id,
            None => {
                return Ok(());
            },
        };
        self.moments_last_seen.update::<MomentsMax>(self.table, moments.label(), sensor_id, now_millis, &mut self.writer);
        let reading_key = MomentsReading {
            table: self.table,
            sensor_id,
            time_ms: Reverse(now_millis),
        };
        let reading_value: MomentsPb = moments.read().into();
        self.emit_reading(reading_key, now_millis, reading_value);
        Ok(())
    }
}
