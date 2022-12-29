use std::cell::RefCell;
use std::fs::File;
use std::io::Write;
use std::rc::Rc;
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

pub trait Sensor {
    type Reading;

    fn what(&'static self) -> &'static str;
    fn read(&'static self) -> Self::Reading;
    fn mark_registered(&'static self);
}

////////////////////////////////////////// SensorRegistry //////////////////////////////////////////

pub struct SensorRegistry<S: Sensor + 'static> {
    sensors: Mutex<Vec<&'static S>>,
    register: &'static Counter,
    emit: &'static Counter,
    err: &'static Counter,
}

impl<S: Sensor + 'static> SensorRegistry<S> {
    pub fn new(register: &'static Counter, emit: &'static Counter, err: &'static Counter) -> Self {
        Self {
            sensors: Mutex::new(Vec::new()),
            register,
            emit,
            err,
        }
    }

    pub fn register(&self, sensor: &'static S) {
        {
            let mut sensors = self.sensors.lock().unwrap();
            sensors.push(sensor);
        }
        self.register.click();
        sensor.mark_registered();
    }

    pub fn emit<EM: Emitter<Error = ERR>, ERR>(
        &self,
        emitter: &mut EM,
        emit: &dyn Fn(&mut EM, &'static S, f64) -> Result<(), ERR>,
        now: &dyn Fn() -> f64
    ) -> Result<(), ERR> {
        let num_sensors = { self.sensors.lock().unwrap().len() };
        let mut sensors: Vec<&'static S> = Vec::with_capacity(num_sensors);
        {
            let sensors_guard = self.sensors.lock().unwrap();
            for s in sensors_guard.iter() {
                sensors.push(s.clone());
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

/////////////////////////////////////////// thread locals //////////////////////////////////////////

thread_local! {
    pub static COLLECT: RefCell<Option<Rc<Collector>>> = RefCell::new(None);
}

pub fn register_counter(counter: &'static Counter) -> bool {
    let mut result = false;
    COLLECT.with(|f| match f.borrow().as_ref() {
        Some(collector) => {
            collector.register_counter(counter);
            result = true
        }
        None => {}
    });
    if result {
        counter.mark_registered();
    }
    result
}

pub fn register_gauge(gauge: &'static Gauge) -> bool {
    let mut result = false;
    COLLECT.with(|f| match f.borrow().as_ref() {
        Some(collector) => {
            collector.register_gauge(gauge);
            result = true
        }
        None => {}
    });
    if result {
        gauge.mark_registered();
    }
    result
}

pub fn register_moments(moments: &'static Moments) -> bool {
    let mut result = false;
    COLLECT.with(|f| match f.borrow().as_ref() {
        Some(collector) => {
            collector.register_moments(moments);
            result = true
        }
        None => {}
    });
    if result {
        moments.mark_registered();
    }
    result
}

pub fn register_t_digest(t_digest: &'static TDigest) -> bool {
    let mut result = false;
    COLLECT.with(|f| match f.borrow().as_ref() {
        Some(collector) => {
            collector.register_t_digest(t_digest);
            result = true
        }
        None => {}
    });
    if result {
        t_digest.mark_registered();
    }
    result
}

///////////////////////////////////////////// Collector ////////////////////////////////////////////

pub struct Collector {
    counters: SensorRegistry<Counter>,
    gauges: SensorRegistry<Gauge>,
    moments: SensorRegistry<Moments>,
    t_digests: SensorRegistry<TDigest>,
}

static COLLECTOR_REGISTER_THREAD: Counter = Counter::new("collector.register.thread");
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
    pub fn new() -> Rc<Self> {
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
        collector.register_counter(&COLLECTOR_REGISTER_THREAD);
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
        // Return the collector with counters initialized.  All collectors will return the global
        // counters.
        Rc::new(collector)
    }

    pub fn register_with_thread(self: Rc<Collector>) {
        COLLECTOR_REGISTER_THREAD.click();
        COLLECT.with(|f| {
            *f.borrow_mut() = Some(Rc::clone(&self));
        });
    }

    pub fn register_counter(&self, counter: &'static Counter) {
        self.counters.register(counter);
    }

    pub fn register_gauge(&self, gauge: &'static Gauge) {
        self.gauges.register(gauge);
    }

    pub fn register_moments(&self, moments: &'static Moments) {
        self.moments.register(moments);
    }

    pub fn register_t_digest(&self, t_digest: &'static TDigest) {
        self.t_digests.register(t_digest);
    }

    pub fn emit<EM: Emitter<Error = ERR>, ERR>(&self, emitter: &mut EM) -> Result<(), ERR> {
        let result = Ok(());
        let result = result.and(self.counters.emit(emitter, &EM::emit_counter, &|| { self.now() }));
        let result = result.and(self.gauges.emit(emitter, &EM::emit_gauge, &|| { self.now() }));
        let result = result.and(self.moments.emit(emitter, &EM::emit_moments, &|| { self.now() }));
        let result = result.and(self.t_digests.emit(emitter, &EM::emit_t_digest, &|| { self.now() }));
        result
    }

    fn now(&self) -> f64 {
        // TODO(rescrv):  Make this monotonic with std::time::Instant.
        let now = SystemTime::now().duration_since(UNIX_EPOCH);
        match now {
            Ok(now) => now.as_millis() as f64,
            Err(_) => {
                COLLECTOR_TIME_FAILURE.click();
                f64::NAN
            }
        }
    }
}

/////////////////////////////////////////////// click //////////////////////////////////////////////

#[macro_export]
macro_rules! click {
    ($name:literal) => {
        static COUNTER: $crate::Counter = $crate::Counter::new($name);
        COUNTER.click();
    };
}

////////////////////////////////////////////// Emitter /////////////////////////////////////////////

pub trait Emitter {
    type Error;

    fn emit_counter(&mut self, counter: &'static Counter, now: f64) -> Result<(), Self::Error>;
    fn emit_gauge(&mut self, gauge: &'static Gauge, now: f64) -> Result<(), Self::Error>;
    fn emit_moments(&mut self, moments: &'static Moments, now: f64) -> Result<(), Self::Error>;
    fn emit_t_digest(&mut self, t_digest: &'static TDigest, now: f64) -> Result<(), Self::Error>;
}

///////////////////////////////////////// PlainTextEmitter /////////////////////////////////////////

pub struct PlainTextEmitter {
    output: File,
}

impl Emitter for PlainTextEmitter {
    type Error = std::io::Error;

    fn emit_counter(&mut self, counter: &'static Counter, now: f64) -> Result<(), std::io::Error> {
        self.output.write_fmt(format_args!(
            "{} {} {}",
            counter.what(),
            now,
            counter.read()
        ))
    }

    fn emit_gauge(&mut self, gauge: &'static Gauge, now: f64) -> Result<(), std::io::Error> {
        self.output
            .write_fmt(format_args!("{} {} {}", gauge.what(), now, gauge.read()))
    }

    fn emit_moments(&mut self, moments: &'static Moments, now: f64) -> Result<(), std::io::Error> {
        let what = moments.what();
        let moments = moments.read();
        self.output.write_fmt(format_args!(
            "{} {} {} {} {} {} {}",
            what,
            now,
            moments.n(),
            moments.mean(),
            moments.variance(),
            moments.skewness(),
            moments.kurtosis()
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
    fn register_thread() {
        let c: Rc<Collector> = Collector::new();
        c.register_with_thread();
        let mut x: Option<Rc<Collector>> = None;
        COLLECT.with(|f| x = f.borrow().as_ref().cloned());
    }
}
