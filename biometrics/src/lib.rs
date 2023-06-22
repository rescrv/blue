//! biometrics is a library for measuring the vitals of processes using intrusive sensors.
//! A sensor is an object that maintains a view of the system.  Threads active in the system
//! cooperate to update the view, and background threads output the view to elsewhere for analysis.

use std::fs::File;
use std::io::Write;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub mod moments;
pub mod sensors;
pub mod t_digest;

pub use sensors::Counter;
pub use sensors::Gauge;
pub use sensors::Moments;
pub use sensors::TDigest;

////////////////////////////////////////////// Sensor //////////////////////////////////////////////

/// [Sensor] is the core type of the system.
pub trait Sensor {
    type Reading;

    /// Every sensor has a label.  This is a UTF-8 string.  It must be static because sensors are
    /// meant to be instantiated statically as well, and having the constraint here enforces that.
    fn label(&'static self) -> &'static str;
    /// Return a linearlizable view of the sensor.
    fn read(&'static self) -> Self::Reading;
}

////////////////////////////////////////// SensorRegistry //////////////////////////////////////////

/// [SensorRegistry] refers to a set of sensors of the same type.
struct SensorRegistry<S: Sensor + 'static> {
    sensors: Mutex<Vec<&'static S>>,
    register: &'static Counter,
    emit: &'static Counter,
    err: &'static Counter,
}

impl<S: Sensor + 'static> SensorRegistry<S> {
    /// Create a new [SensorRegistry] using the three counters for internal instrumentation.  We
    /// don't define these counters here, so that each registry can define its own counters and get
    /// ground truth about the registry.
    pub fn new(register: &'static Counter, emit: &'static Counter, err: &'static Counter) -> Self {
        Self {
            sensors: Mutex::new(Vec::new()),
            register,
            emit,
            err,
        }
    }

    /// Unconditionally register the sensor with the sensor library.
    pub fn register(&self, sensor: &'static S) {
        {
            let mut sensors = self.sensors.lock().unwrap();
            sensors.push(sensor);
        }
        self.register.click();
    }

    /// Emit readings all sensors through `emitter`+`emit`, recording each sensor reading as close
    /// to `now` as possible.
    fn emit<EM: Emitter<Error = ERR>, ERR>(
        &self,
        emitter: &mut EM,
        emit: &dyn Fn(&mut EM, &'static S, f64) -> Result<(), ERR>,
        now: &dyn Fn() -> f64,
    ) -> Result<(), ERR> {
        let num_sensors = { self.sensors.lock().unwrap().len() };
        let mut sensors: Vec<&'static S> = Vec::with_capacity(num_sensors);
        {
            let sensors_guard = self.sensors.lock().unwrap();
            for s in sensors_guard.iter() {
                sensors.push(*s);
            }
        }
        let mut result = Ok(());
        for sensor in sensors {
            match emit(emitter, sensor, now()) {
                Ok(_) => self.emit.click(),
                Err(e) => {
                    if let Ok(()) = result {
                        result = Err(e);
                    }
                    self.err.click();
                }
            }
        }
        result
    }
}

///////////////////////////////////////////// Collector ////////////////////////////////////////////

/// Collect and register sensors of all types.  One registry per sensor type.
pub struct Collector {
    counters: SensorRegistry<Counter>,
    gauges: SensorRegistry<Gauge>,
    moments: SensorRegistry<Moments>,
    t_digests: SensorRegistry<TDigest>,
}

static COLLECTOR_REGISTER_COUNTER: Counter = Counter::new("collector.register.counter");
static COLLECTOR_REGISTER_GAUGE: Counter = Counter::new("collector.register.gauge");
static COLLECTOR_REGISTER_MOMENTS: Counter = Counter::new("collector.register.moments");
static COLLECTOR_REGISTER_T_DIGEST: Counter = Counter::new("collector.register.t_digest");
static COLLECTOR_EMIT_COUNTER: Counter = Counter::new("collector.emit.counter");
static COLLECTOR_EMIT_GAUGE: Counter = Counter::new("collector.emit.gauge");
static COLLECTOR_EMIT_MOMENTS: Counter = Counter::new("collector.emit.moments");
static COLLECTOR_EMIT_T_DIGEST: Counter = Counter::new("collector.emit.t_digest");
static COLLECTOR_EMIT_FAILURE: Counter = Counter::new("collector.emit.failure");
static COLLECTOR_TIME_FAILURE: Counter = Counter::new("collector.time.failure");

impl Collector {
    /// Get a new [Collector].  The collector will use the global registries and emit to the
    /// COLLECTOR_* counters for easy monitoring.
    pub fn new() -> Self {
        let collector = Self {
            counters: SensorRegistry::new(
                &COLLECTOR_REGISTER_COUNTER,
                &COLLECTOR_EMIT_COUNTER,
                &COLLECTOR_EMIT_FAILURE,
            ),
            gauges: SensorRegistry::new(
                &COLLECTOR_REGISTER_GAUGE,
                &COLLECTOR_EMIT_GAUGE,
                &COLLECTOR_EMIT_FAILURE,
            ),
            moments: SensorRegistry::new(
                &COLLECTOR_REGISTER_MOMENTS,
                &COLLECTOR_EMIT_MOMENTS,
                &COLLECTOR_EMIT_FAILURE,
            ),
            t_digests: SensorRegistry::new(
                &COLLECTOR_REGISTER_T_DIGEST,
                &COLLECTOR_EMIT_T_DIGEST,
                &COLLECTOR_EMIT_FAILURE,
            ),
        };
        // Register counters with the collector.
        collector.register_counter(&COLLECTOR_REGISTER_COUNTER);
        collector.register_counter(&COLLECTOR_REGISTER_GAUGE);
        collector.register_counter(&COLLECTOR_REGISTER_MOMENTS);
        collector.register_counter(&COLLECTOR_REGISTER_T_DIGEST);
        collector.register_counter(&COLLECTOR_EMIT_COUNTER);
        collector.register_counter(&COLLECTOR_EMIT_GAUGE);
        collector.register_counter(&COLLECTOR_EMIT_MOMENTS);
        collector.register_counter(&COLLECTOR_EMIT_T_DIGEST);
        collector.register_counter(&COLLECTOR_EMIT_FAILURE);
        collector.register_counter(&COLLECTOR_TIME_FAILURE);
        // Return the collector with counters initialized.
        collector
    }

    /// Register `counter` with the Collector.
    pub fn register_counter(&self, counter: &'static Counter) {
        self.counters.register(counter);
    }

    /// Register `gauge` with the Collector.
    pub fn register_gauge(&self, gauge: &'static Gauge) {
        self.gauges.register(gauge);
    }

    /// Register `moments` with the Collector.
    pub fn register_moments(&self, moments: &'static Moments) {
        self.moments.register(moments);
    }

    /// Register `t_digest` with the Collector.
    pub fn register_t_digest(&self, t_digest: &'static TDigest) {
        self.t_digests.register(t_digest);
    }

    /// Output the sensors registered to this emitter.
    pub fn emit<EM: Emitter<Error = ERR>, ERR: std::fmt::Debug>(&self, emitter: &mut EM) -> Result<(), ERR> {
        let result = Ok(());
        let result = result.and(self.counters.emit(emitter, &EM::emit_counter, &Self::now_ms));
        let result = result.and(self.gauges.emit(emitter, &EM::emit_gauge, &Self::now_ms));
        let result = result.and(self.moments.emit(emitter, &EM::emit_moments, &Self::now_ms));
        result.and(self.t_digests.emit(emitter, &EM::emit_t_digest, &Self::now_ms))
    }

    fn now_ms() -> f64 {
        // TODO(rescrv):  Make this monotonic with std::time::Instant.
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(now) => now.as_millis() as f64,
            Err(_) => {
                COLLECTOR_TIME_FAILURE.click();
                f64::NAN
            }
        }
    }
}

impl Default for Collector {
    fn default() -> Self {
        Self::new()
    }
}

////////////////////////////////////////////// Emitter /////////////////////////////////////////////

/// [Emitter] outputs the sensor state via I/O.
pub trait Emitter {
    type Error;

    /// Read the provided [Counter].
    fn emit_counter(&mut self, counter: &'static Counter, now: f64) -> Result<(), Self::Error>;
    /// Read the provided [Gauge].
    fn emit_gauge(&mut self, gauge: &'static Gauge, now: f64) -> Result<(), Self::Error>;
    /// Read the provided [Moments].
    fn emit_moments(&mut self, moments: &'static Moments, now: f64) -> Result<(), Self::Error>;
    /// Read the provided [TDigest].
    fn emit_t_digest(&mut self, t_digest: &'static TDigest, now: f64) -> Result<(), Self::Error>;
}

///////////////////////////////////////// PlainTextEmitter /////////////////////////////////////////

/// An emitter that puts readings one-per-line.
pub struct PlainTextEmitter {
    output: File,
}

impl PlainTextEmitter {
    pub fn new(output: File) -> Self {
        Self {
            output,
        }
    }
}

impl Emitter for PlainTextEmitter {
    type Error = std::io::Error;

    fn emit_counter(&mut self, counter: &'static Counter, now: f64) -> Result<(), std::io::Error> {
        self.output.write_fmt(format_args!(
            "{} {} {}\n",
            counter.label(),
            now,
            counter.read()
        ))
    }

    fn emit_gauge(&mut self, gauge: &'static Gauge, now: f64) -> Result<(), std::io::Error> {
        self.output
            .write_fmt(format_args!("{} {} {}\n", gauge.label(), now, gauge.read()))
    }

    fn emit_moments(&mut self, moments: &'static Moments, now: f64) -> Result<(), std::io::Error> {
        let label = moments.label();
        let moments = moments.read();
        self.output.write_fmt(format_args!(
            "{} {} {} {} {} {} {}\n",
            label,
            now,
            moments.n,
            moments.m1,
            moments.m2,
            moments.m3,
            moments.m4,
        ))
    }

    fn emit_t_digest(&mut self, _: &'static TDigest, _: f64) -> Result<(), std::io::Error> {
        // TODO(rescrv): Emit the t-digest.
        Err(std::io::Error::from_raw_os_error(95 /*ENOTSUP*/))
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collector_new() {
        let _: Collector = Collector::new();
    }
}
