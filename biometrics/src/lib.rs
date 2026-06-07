#![doc = include_str!("../README.md")]
#![deny(missing_docs)]

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
    /// Return a linearizable view of the sensor.
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
    fn new(register: &'static Counter, emit: &'static Counter, err: &'static Counter) -> Self {
        Self {
            sensors: Mutex::new(Vec::new()),
            register,
            emit,
            err,
        }
    }

    /// Unconditionally register the sensor with the sensor library.
    fn register(&self, sensor: &'static S) {
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
        let sensors = self.sensors.lock().unwrap().clone();
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

    /// Register `histogram` with the Collector.
    pub fn register_histogram(&self, histogram: &'static Histogram) {
        self.histograms.register(histogram);
    }

    /// Output the sensors registered to this emitter.
    pub fn emit<EM: Emitter<Error = ERR>, ERR>(
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
                .write_fmt(format_args!("{label} {now} {{approx={bucket}}} {count}\n",))?;
        }
        Ok(())
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use std::fs::{File, read_to_string, remove_file};
    use std::sync::Mutex;

    use super::*;

    #[test]
    fn collector_new() {
        let _: Collector = Collector::new();
    }

    #[derive(Default)]
    struct RecordingEmitter {
        events: Vec<(&'static str, &'static str)>,
    }

    impl Emitter for RecordingEmitter {
        type Error = std::io::Error;

        fn emit_counter(&mut self, counter: &Counter, _: u64) -> Result<(), Self::Error> {
            self.events.push(("counter", counter.label()));
            Ok(())
        }

        fn emit_gauge(&mut self, gauge: &Gauge, _: u64) -> Result<(), Self::Error> {
            self.events.push(("gauge", gauge.label()));
            Ok(())
        }

        fn emit_moments(&mut self, moments: &Moments, _: u64) -> Result<(), Self::Error> {
            self.events.push(("moments", moments.label()));
            Ok(())
        }

        fn emit_histogram(&mut self, histogram: &Histogram, _: u64) -> Result<(), Self::Error> {
            self.events.push(("histogram", histogram.label()));
            Ok(())
        }
    }

    #[test]
    fn collector_emits_registered_sensor_types() {
        static COUNTER: Counter = Counter::new("collector.emits.counter");
        static GAUGE: Gauge = Gauge::new("collector.emits.gauge");
        static MOMENTS: Moments = Moments::new("collector.emits.moments");
        static HISTOGRAM_IMPL: sig_fig_histogram::LockFreeHistogram<1> =
            sig_fig_histogram::LockFreeHistogram::new(2);
        static HISTOGRAM: Histogram = Histogram::new("collector.emits.histogram", &HISTOGRAM_IMPL);

        let collector = Collector::new();
        collector.register_counter(&COUNTER);
        collector.register_gauge(&GAUGE);
        collector.register_moments(&MOMENTS);
        collector.register_histogram(&HISTOGRAM);
        let mut emitter = RecordingEmitter::default();

        collector.emit(&mut emitter, 42).unwrap();

        assert!(
            emitter
                .events
                .contains(&("counter", "collector.emits.counter"))
        );
        assert!(emitter.events.contains(&("gauge", "collector.emits.gauge")));
        assert!(
            emitter
                .events
                .contains(&("moments", "collector.emits.moments"))
        );
        assert!(
            emitter
                .events
                .contains(&("histogram", "collector.emits.histogram"))
        );
    }

    #[test]
    fn plain_text_emitter_writes_one_reading_per_line() {
        static MUTEX: Mutex<()> = Mutex::new(());
        static COUNTER: Counter = Counter::new("plain.counter");
        static GAUGE: Gauge = Gauge::new("plain.gauge");
        static MOMENTS: Moments = Moments::new("plain.moments");
        static HISTOGRAM_IMPL: sig_fig_histogram::LockFreeHistogram<2> =
            sig_fig_histogram::LockFreeHistogram::new(2);
        static HISTOGRAM: Histogram = Histogram::new("plain.histogram", &HISTOGRAM_IMPL);
        let _guard = MUTEX.lock().unwrap();
        let path = "tmp.biometrics.plain_text_emitter_writes_one_reading_per_line";
        if std::path::Path::new(path).exists() {
            remove_file(path).unwrap();
        }

        COUNTER.count(3);
        GAUGE.set(12.5);
        MOMENTS.add(1.0);
        MOMENTS.add(3.0);
        HISTOGRAM.observe(1.0);
        HISTOGRAM.observe_n(1.1, 3);
        {
            let file = File::create(path).unwrap();
            let mut emitter = PlainTextEmitter::new(file);
            emitter.emit_counter(&COUNTER, 7).unwrap();
            emitter.emit_gauge(&GAUGE, 7).unwrap();
            emitter.emit_moments(&MOMENTS, 7).unwrap();
            emitter.emit_histogram(&HISTOGRAM, 7).unwrap();
        }

        assert_eq!(
            "plain.counter 7 3\n\
plain.gauge 7 12.5\n\
plain.moments 7 2 2 2 0 2\n\
plain.histogram 7 {approx=1} 1\n\
plain.histogram 7 {approx=1.1} 3\n",
            read_to_string(path).unwrap()
        );
        remove_file(path).unwrap();
    }
}
