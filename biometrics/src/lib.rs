use std::cell::RefCell;
use std::collections::hash_set::HashSet;
use std::fs::File;
use std::io::Write;
use std::rc::Rc;
use std::sync::Mutex;
use std::time::{SystemTime,UNIX_EPOCH};

pub mod sensors;
pub mod moments;
pub mod t_digest;

pub use sensors::Counter;
pub use sensors::Gauge;
pub use sensors::Moments;
pub use sensors::TDigest;

/////////////////////////////////////////// thread locals //////////////////////////////////////////

thread_local! {
    pub static COLLECT: RefCell<Option<Rc<Collector>>> = RefCell::new(None);
}

pub fn register_counter(counter: &'static Counter) -> bool {
    let mut result = false;
    COLLECT.with(|f| {
        match f.borrow().as_ref() {
            Some(collector) => {
                collector.register_counter(counter);
                result = true
            },
            None => {},
        }
    });
    if result {
        counter.mark_registered();
    }
    result
}

pub fn register_gauge(gauge: &'static Gauge) -> bool {
    let mut result = false;
    COLLECT.with(|f| {
        match f.borrow().as_ref() {
            Some(collector) => {
                collector.register_gauge(gauge);
                result = true
            },
            None => {},
        }
    });
    if result {
        gauge.mark_registered();
    }
    result
}

pub fn register_moments(moments: &'static Moments) -> bool {
    let mut result = false;
    COLLECT.with(|f| {
        match f.borrow().as_ref() {
            Some(collector) => {
                collector.register_moments(moments);
                result = true
            },
            None => {},
        }
    });
    if result {
        moments.mark_registered();
    }
    result
}

pub fn register_t_digest(t_digest: &'static TDigest) -> bool {
    let mut result = false;
    COLLECT.with(|f| {
        match f.borrow().as_ref() {
            Some(collector) => {
                collector.register_t_digest(t_digest);
                result = true
            },
            None => {},
        }
    });
    if result {
        t_digest.mark_registered();
    }
    result
}

///////////////////////////////////////////// Collector ////////////////////////////////////////////

pub struct Collector {
    counters: Mutex<HashSet<&'static Counter>>,
    gauges: Mutex<HashSet<&'static Gauge>>,
    moments: Mutex<HashSet<&'static Moments>>,
    t_digests: Mutex<HashSet<&'static TDigest>>,
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
            counters: Mutex::new(HashSet::new()),
            gauges: Mutex::new(HashSet::new()),
            moments: Mutex::new(HashSet::new()),
            t_digests: Mutex::new(HashSet::new()),
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
        // Return the collector with counters initialized.  They will not reassociate to any other
        // collector.
        Rc::new(collector)
    }

    pub fn register_with_thread(self: Rc<Collector>) {
        COLLECTOR_REGISTER_THREAD.click();
        COLLECT.with(|f| {
            *f.borrow_mut() = Some(Rc::clone(&self));
        });
    }

    // Must saturate.
    pub fn register_counter(&self, counter: &'static Counter) {
        let mut counters = self.counters.lock().unwrap();
        counters.insert(counter);
        COLLECTOR_REGISTER_COUNTER.click();
    }

    // Must saturate.
    pub fn register_gauge(&self, gauge: &'static Gauge) {
        let mut gauges = self.gauges.lock().unwrap();
        gauges.insert(gauge);
        COLLECTOR_REGISTER_GAUGE.click();
    }

    // Must saturate.
    pub fn register_moments(&self, moments: &'static Moments) {
        let mut momentss = self.moments.lock().unwrap();
        momentss.insert(moments);
        COLLECTOR_REGISTER_MOMENTS.click();
    }

    // Must saturate.
    pub fn register_t_digest(&self, t_digest: &'static TDigest) {
        let mut t_digests = self.t_digests.lock().unwrap();
        t_digests.insert(t_digest);
        COLLECTOR_REGISTER_T_DIGEST.click();
    }

    pub fn emit(&self, emitter: &mut dyn Emitter) -> Result<(), std::io::Error> {
        // counters
        let num_counters = {
            let counters = self.counters.lock().unwrap();
            counters.len()
        };
        let mut counters: Vec<&'static Counter> = Vec::with_capacity(num_counters);
        {
            let counters_guard = self.counters.lock().unwrap();
            for c in counters_guard.iter() {
                counters.push(c.clone());
            }
        }
        for counter in counters {
            COLLECTOR_EMIT_COUNTER.click();
            match emitter.emit_counter(counter, self.now_f64()) {
                Ok(_) => {},
                Err(_) => {
                    // TODO(rescrv): Maybe not swallow errors.
                    COLLECTOR_EMIT_FAILURE.click();
                },
            }
        }

        // gauges
        let num_gauges = {
            let gauges = self.gauges.lock().unwrap();
            gauges.len()
        };
        let mut gauges: Vec<&'static Gauge> = Vec::with_capacity(num_gauges);
        {
            let gauges_guard = self.gauges.lock().unwrap();
            for g in gauges_guard.iter() {
                gauges.push(g.clone());
            }
        }
        for gauge in gauges {
            COLLECTOR_EMIT_GAUGE.click();
            match emitter.emit_gauge(gauge, self.now_f64()) {
                Ok(_) => {},
                Err(_) => {
                    // TODO(rescrv): Maybe not swallow errors.
                    COLLECTOR_EMIT_FAILURE.click();
                },
            }
        }

        // moments
        let num_moments = {
            let gauges = self.gauges.lock().unwrap();
            gauges.len()
        };
        let mut momentss: Vec<&'static Moments> = Vec::with_capacity(num_moments);
        {
            let moments_guard = self.moments.lock().unwrap();
            for m in moments_guard.iter() {
                momentss.push(m.clone());
            }
        }
        for moments in momentss {
            COLLECTOR_EMIT_MOMENTS.click();
            match emitter.emit_moments(moments, self.now_f64()) {
                Ok(_) => {},
                Err(_) => {
                    // TODO(rescrv): Maybe not swallow errors.
                    COLLECTOR_EMIT_FAILURE.click();
                },
            }
        }

        // t_digests
        let num_t_digests = {
            let t_digests = self.t_digests.lock().unwrap();
            t_digests.len()
        };
        let mut t_digests: Vec<&'static TDigest> = Vec::with_capacity(num_t_digests);
        {
            let t_digests_guard = self.t_digests.lock().unwrap();
            for t in t_digests_guard.iter() {
                t_digests.push(t.clone());
            }
        }
        for t_digest in t_digests {
            COLLECTOR_EMIT_T_DIGEST.click();
            match emitter.emit_t_digest(t_digest, self.now_f64()) {
                Ok(_) => {},
                Err(_) => {
                    // TODO(rescrv): Maybe not swallow errors.
                    COLLECTOR_EMIT_FAILURE.click();
                },
            }
        }

        Ok(())
    }

    fn now_f64(&self) -> f64 {
        // TODO(rescrv):  Make this monotonic with std::time::Instant.
        let now = SystemTime::now().duration_since(UNIX_EPOCH);
        match now {
            Ok(now) => { now.as_millis() as f64 },
            Err(_) => {
                COLLECTOR_TIME_FAILURE.click();
                f64::NAN
            },
        }
    }
}

/////////////////////////////////////////////// click //////////////////////////////////////////////

#[macro_export]
macro_rules! click {
    ($name:literal) => {
        static COUNTER: $crate::Counter = $crate::Counter::new($name);
        COUNTER.click();
    }
}

////////////////////////////////////////////// Emitter /////////////////////////////////////////////

pub trait Emitter {
    fn emit_counter(&mut self, counter: &'static Counter, now: f64) -> Result<(), std::io::Error>;
    fn emit_gauge(&mut self, gauge: &'static Gauge, now: f64) -> Result<(), std::io::Error>;
    fn emit_moments(&mut self, moments: &'static Moments, now: f64) -> Result<(), std::io::Error>;
    fn emit_t_digest(&mut self, t_digest: &'static TDigest, now: f64) -> Result<(), std::io::Error>;
}

///////////////////////////////////////// PlainTextEmitter /////////////////////////////////////////

pub struct PlainTextEmitter {
    output: File,
}

impl Emitter for PlainTextEmitter {
    fn emit_counter(&mut self, counter: &'static Counter, now: f64) -> Result<(), std::io::Error> {
        self.output.write_fmt(format_args!("{} {} {}", counter.what(), now, counter.read()))
    }

    fn emit_gauge(&mut self, gauge: &'static Gauge, now: f64) -> Result<(), std::io::Error> {
        self.output.write_fmt(format_args!("{} {} {}", gauge.what(), now, gauge.read()))
    }

    fn emit_moments(&mut self, moments: &'static Moments, now: f64) -> Result<(), std::io::Error> {
        let what = moments.what();
        let moments = moments.read();
        self.output.write_fmt(format_args!("{} {} {} {} {} {} {}",
                                           what,
                                           now,
                                           moments.n(),
                                           moments.mean(),
                                           moments.variance(),
                                           moments.skewness(),
                                           moments.kurtosis()))
    }

    fn emit_t_digest(&mut self, _: &'static TDigest, _: f64) -> Result<(), std::io::Error> {
        // TODO(rescrv): Emit the t-digest.
        Err(std::io::Error::from_raw_os_error(95/*ENOTSUP*/))
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
        COLLECT.with(|f| {
            x = f.borrow().as_ref().cloned()
        });
    }
}
