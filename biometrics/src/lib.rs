#![doc = include_str!("../README.md")]

use std::fs::File;
use std::io::Write;
use std::sync::Mutex;

pub mod moments;
mod sensors;

pub use sensors::Counter;
pub use sensors::Gauge;
pub use sensors::Histogram;
pub use sensors::Moments;

////////////////////////////////////////////// Sensor //////////////////////////////////////////////

/// [Sensor] is the core type of the system.  A sensor must be algebraic to be included in this
/// library.  An algebraic sensor allows one to take two readings, one on each side of a bucket,
/// and compute the bucket with a single subtraction.
pub trait Sensor {
    /// The type of a sensor reading.
    type Reading;

    /// Every sensor has a label.  This is a UTF-8 string.  It must be static because sensors are
    /// meant to be instantiated statically as well, and having the constraint here enforces that.
    fn label(&self) -> &'static str;
    /// Return a linearlizable view of the sensor.
    fn read(&self) -> Self::Reading;
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
        emit: impl Fn(&mut EM, &S, u64) -> Result<(), ERR>,
        now: u64,
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
            match emit(emitter, sensor, now) {
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
    histograms: SensorRegistry<Histogram>,
}

static COLLECTOR_REGISTER_COUNTER: Counter = Counter::new("biometrics.collector.register.counter");
static COLLECTOR_REGISTER_GAUGE: Counter = Counter::new("biometrics.collector.register.gauge");
static COLLECTOR_REGISTER_MOMENTS: Counter = Counter::new("biometrics.collector.register.moments");
static COLLECTOR_REGISTER_HISTOGRAM: Counter =
    Counter::new("biometrics.collector.register.histogram");
static COLLECTOR_EMIT_COUNTER: Counter = Counter::new("biometrics.collector.emit.counter");
static COLLECTOR_EMIT_GAUGE: Counter = Counter::new("biometrics.collector.emit.gauge");
static COLLECTOR_EMIT_MOMENTS: Counter = Counter::new("biometrics.collector.emit.moments");
static COLLECTOR_EMIT_HISTOGRAM: Counter = Counter::new("biometrics.collector.emit.histogram");
static COLLECTOR_EMIT_FAILURE: Counter = Counter::new("biometrics.collector.emit.failure");
static COLLECTOR_TIME_FAILURE: Counter = Counter::new("biometrics.collector.time.failure");

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
            histograms: SensorRegistry::new(
                &COLLECTOR_REGISTER_HISTOGRAM,
                &COLLECTOR_EMIT_HISTOGRAM,
                &COLLECTOR_EMIT_FAILURE,
            ),
        };
        // Register counters with the collector.
        collector.register_counter(&COLLECTOR_REGISTER_COUNTER);
        collector.register_counter(&COLLECTOR_REGISTER_GAUGE);
        collector.register_counter(&COLLECTOR_REGISTER_MOMENTS);
        collector.register_counter(&COLLECTOR_REGISTER_HISTOGRAM);
        collector.register_counter(&COLLECTOR_EMIT_COUNTER);
        collector.register_counter(&COLLECTOR_EMIT_GAUGE);
        collector.register_counter(&COLLECTOR_EMIT_MOMENTS);
        collector.register_counter(&COLLECTOR_EMIT_HISTOGRAM);
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

    /// Register `moments` with the Collector.
    pub fn register_histogram(&self, histogram: &'static Histogram) {
        self.histograms.register(histogram);
    }

    /// Output the sensors registered to this emitter.
    pub fn emit<EM: Emitter<Error = ERR>, ERR: std::fmt::Debug>(
        &self,
        emitter: &mut EM,
        now: u64,
    ) -> Result<(), ERR> {
        let result = Ok(());
        let result = result.and(self.counters.emit(emitter, EM::emit_counter, now));
        let result = result.and(self.gauges.emit(emitter, EM::emit_gauge, now));
        let result = result.and(self.moments.emit(emitter, EM::emit_moments, now));
        result.and(self.histograms.emit(emitter, EM::emit_histogram, now))
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
    /// The type of error this emitter returns.
    type Error;

    /// Read the provided [Counter].
    fn emit_counter(&mut self, counter: &Counter, now_millis: u64) -> Result<(), Self::Error>;
    /// Read the provided [Gauge].
    fn emit_gauge(&mut self, gauge: &Gauge, now_millis: u64) -> Result<(), Self::Error>;
    /// Read the provided [Moments].
    fn emit_moments(&mut self, moments: &Moments, now_millis: u64) -> Result<(), Self::Error>;
    /// Read the provided [Histogram].
    fn emit_histogram(&mut self, histogram: &Histogram, now_millis: u64)
        -> Result<(), Self::Error>;
}

///////////////////////////////////////// PlainTextEmitter /////////////////////////////////////////

/// An emitter that puts readings one-per-line.
pub struct PlainTextEmitter {
    output: File,
}

impl PlainTextEmitter {
    /// Create a new plain-text emitter.
    pub fn new(output: File) -> Self {
        Self { output }
    }
}

impl Emitter for PlainTextEmitter {
    type Error = std::io::Error;

    fn emit_counter(&mut self, counter: &Counter, now: u64) -> Result<(), std::io::Error> {
        self.output.write_fmt(format_args!(
            "{} {} {}\n",
            counter.label(),
            now,
            counter.read()
        ))
    }

    fn emit_gauge(&mut self, gauge: &Gauge, now: u64) -> Result<(), std::io::Error> {
        self.output
            .write_fmt(format_args!("{} {} {}\n", gauge.label(), now, gauge.read()))
    }

    fn emit_moments(&mut self, moments: &Moments, now: u64) -> Result<(), std::io::Error> {
        let label = moments.label();
        let moments = moments.read();
        self.output.write_fmt(format_args!(
            "{} {} {} {} {} {} {}\n",
            label, now, moments.n, moments.m1, moments.m2, moments.m3, moments.m4,
        ))
    }

    fn emit_histogram(&mut self, histogram: &Histogram, now: u64) -> Result<(), std::io::Error> {
        let label = histogram.label();
        for (bucket, count) in histogram.read().iter() {
            self.output
                .write_fmt(format_args!("{} {now} {{approx={bucket}}} {count}", label,))?;
        }
        Ok(())
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
