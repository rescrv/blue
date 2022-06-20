use std::cell::RefCell;
use std::collections::hash_set::HashSet;
use std::fs::File;
use std::io::Write;
use std::sync::Mutex;
use std::time::{SystemTime,UNIX_EPOCH};

pub mod sensors;
pub mod moments;

pub use sensors::Counter;
pub use sensors::Gauge;
pub use sensors::Moments;

/////////////////////////////////////////// thread locals //////////////////////////////////////////

thread_local! {
    pub static COLLECT: RefCell<Option<*const Collector>> = RefCell::new(None);
}

pub fn register_counter(counter: *const Counter) -> bool {
    let mut result = false;
    COLLECT.with(|f| {
        match *f.borrow() {
            Some(collector) => {
                unsafe {
                    (*collector).register_counter(counter);
                }
                result = true
            },
            None => {},
        }
    });
    result
}

pub fn register_gauge(gauge: *const Gauge) -> bool {
    let mut result = false;
    COLLECT.with(|f| {
        match *f.borrow() {
            Some(collector) => {
                unsafe {
                    (*collector).register_gauge(gauge);
                }
                result = true
            },
            None => {},
        }
    });
    result
}

pub fn register_moments(moments: *const Moments) -> bool {
    let mut result = false;
    COLLECT.with(|f| {
        match *f.borrow() {
            Some(collector) => {
                unsafe {
                    (*collector).register_moments(moments);
                }
                result = true
            },
            None => {},
        }
    });
    result
}


///////////////////////////////////////////// Collector ////////////////////////////////////////////

pub struct Collector {
    counters: Mutex<HashSet<*const Counter>>,
    gauges: Mutex<HashSet<*const Gauge>>,
    moments: Mutex<HashSet<*const Moments>>,
    counter_register_thread: Counter,
    counter_register_counter: Counter,
    counter_register_gauge: Counter,
    counter_register_moments: Counter,
    counter_emit_counter: Counter,
    counter_emit_gauge: Counter,
    counter_emit_moments: Counter,
    counter_emit_failure: Counter,
    counter_time_failure: Counter,
}

impl Collector {
    pub fn new() -> Self {
        let collector = Self {
            counters: Mutex::new(HashSet::new()),
            gauges: Mutex::new(HashSet::new()),
            moments: Mutex::new(HashSet::new()),
            counter_register_thread: Counter::new("collector.register.thread"),
            counter_register_counter: Counter::new("collector.register.counter"),
            counter_register_gauge: Counter::new("collector.register.gauge"),
            counter_register_moments: Counter::new("collector.register.moments"),
            counter_emit_counter: Counter::new("collector.emit.counter"),
            counter_emit_gauge: Counter::new("collector.emit.gauge"),
            counter_emit_moments: Counter::new("collector.emit.moments"),
            counter_emit_failure: Counter::new("collector.emit.failure"),
            counter_time_failure: Counter::new("collector.time.failure"),
        };
        // Register counters with the collector.
        collector.register_counter(&collector.counter_register_thread as *const Counter);
        collector.register_counter(&collector.counter_register_counter as *const Counter);
        collector.register_counter(&collector.counter_register_gauge as *const Counter);
        collector.register_counter(&collector.counter_register_moments as *const Counter);
        collector.register_counter(&collector.counter_emit_counter as *const Counter);
        collector.register_counter(&collector.counter_emit_gauge as *const Counter);
        collector.register_counter(&collector.counter_emit_moments as *const Counter);
        collector.register_counter(&collector.counter_emit_failure as *const Counter);
        collector.register_counter(&collector.counter_time_failure as *const Counter);
        // Mark the counters registered.
        collector.counter_register_thread.mark_registered();
        collector.counter_register_counter.mark_registered();
        collector.counter_register_gauge.mark_registered();
        collector.counter_register_moments.mark_registered();
        collector.counter_emit_counter.mark_registered();
        collector.counter_emit_gauge.mark_registered();
        collector.counter_emit_moments.mark_registered();
        collector.counter_emit_failure.mark_registered();
        collector.counter_time_failure.mark_registered();
        // Return the collector with counters initialized.  They will not reassociate to any other
        // collector.
        collector
    }

    pub fn register_with_thread(&mut self) {
        COLLECT.with(|f| {
            let ptr: *const Collector = self as *const Collector;
            *f.borrow_mut() = Some(ptr);
        });
        self.counter_register_thread.click();
    }

    // Must saturate.
    pub fn register_counter(&self, counter: *const Counter) {
        let mut counters = self.counters.lock().unwrap();
        counters.insert(counter);
        self.counter_register_counter.click();
    }

    // Must saturate.
    pub fn register_gauge(&self, gauge: *const Gauge) {
        let mut gauges = self.gauges.lock().unwrap();
        gauges.insert(gauge);
        self.counter_register_gauge.click();
    }

    // Must saturate.
    pub fn register_moments(&self, moments: *const Moments) {
        let mut momentss = self.moments.lock().unwrap();
        momentss.insert(moments);
        self.counter_register_moments.click();
    }

    pub fn emit(&self, emitter: &mut dyn Emitter) -> Result<(), std::io::Error> {
        // Internal counters always come first.
        emitter.emit_counter(&self.counter_register_thread, self.now_f64())?;
        emitter.emit_counter(&self.counter_register_counter, self.now_f64())?;
        emitter.emit_counter(&self.counter_register_gauge, self.now_f64())?;
        emitter.emit_counter(&self.counter_register_moments, self.now_f64())?;
        emitter.emit_counter(&self.counter_emit_counter, self.now_f64())?;
        emitter.emit_counter(&self.counter_emit_gauge, self.now_f64())?;
        emitter.emit_counter(&self.counter_emit_moments, self.now_f64())?;
        emitter.emit_counter(&self.counter_emit_failure, self.now_f64())?;
        emitter.emit_counter(&self.counter_time_failure, self.now_f64())?;

        // counters
        let num_counters = {
            let counters = self.counters.lock().unwrap();
            counters.len()
        };
        let mut counters: Vec<*const Counter> = Vec::with_capacity(num_counters);
        {
            let counters_guard = self.counters.lock().unwrap();
            for c in counters_guard.iter() {
                counters.push(c.clone());
            }
        }
        for counter in counters {
            self.counter_emit_counter.click();
            let counter: &Counter = unsafe {
                &(*counter)
            };
            match emitter.emit_counter(counter, self.now_f64()) {
                Ok(_) => {},
                Err(_) => {
                    // TODO(rescrv): Maybe not swallow errors.
                    self.counter_emit_failure.click();
                },
            }
        }

        // gauges
        let num_gauges = {
            let gauges = self.gauges.lock().unwrap();
            gauges.len()
        };
        let mut gauges: Vec<*const Gauge> = Vec::with_capacity(num_gauges);
        {
            let gauges_guard = self.gauges.lock().unwrap();
            for g in gauges_guard.iter() {
                gauges.push(g.clone());
            }
        }
        for gauge in gauges {
            self.counter_emit_gauge.click();
            let gauge: &Gauge = unsafe {
                &(*gauge)
            };
            match emitter.emit_gauge(gauge, self.now_f64()) {
                Ok(_) => {},
                Err(_) => {
                    // TODO(rescrv): Maybe not swallow errors.
                    self.counter_emit_failure.click();
                },
            }
        }

        // moments
        let num_moments = {
            let gauges = self.gauges.lock().unwrap();
            gauges.len()
        };
        let mut momentss: Vec<*const Moments> = Vec::with_capacity(num_moments);
        {
            let moments_guard = self.moments.lock().unwrap();
            for m in moments_guard.iter() {
                momentss.push(m.clone());
            }
        }
        for moments in momentss {
            self.counter_emit_moments.click();
            let moments: &Moments = unsafe {
                &(*moments)
            };
            match emitter.emit_moments(moments, self.now_f64()) {
                Ok(_) => {},
                Err(_) => {
                    // TODO(rescrv): Maybe not swallow errors.
                    self.counter_emit_failure.click();
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
                self.counter_time_failure.click();
                f64::NAN
            },
        }
    }
}

////////////////////////////////////////////// Emitter /////////////////////////////////////////////

pub trait Emitter {
    fn emit_counter(&mut self, counter: &Counter, now: f64) -> Result<(), std::io::Error>;
    fn emit_gauge(&mut self, gauge: &Gauge, now: f64) -> Result<(), std::io::Error>;
    fn emit_moments(&mut self, moments: &Moments, now: f64) -> Result<(), std::io::Error>;
}

///////////////////////////////////////// PlainTextEmitter /////////////////////////////////////////

pub struct PlainTextEmitter {
    output: File,
}

impl Emitter for PlainTextEmitter {
    fn emit_counter(&mut self, counter: &Counter, now: f64) -> Result<(), std::io::Error> {
        self.output.write_fmt(format_args!("{} {} {}", counter.what(), now, counter.read()))
    }

    fn emit_gauge(&mut self, gauge: &Gauge, now: f64) -> Result<(), std::io::Error> {
        self.output.write_fmt(format_args!("{} {} {}", gauge.what(), now, gauge.read()))
    }

    fn emit_moments(&mut self, moments: &Moments, now: f64) -> Result<(), std::io::Error> {
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
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_thread() {
        let mut c = Collector::new();
        c.register_with_thread();
        let mut x: Option<*const Collector> = None;
        COLLECT.with(|f| {
            x = *f.borrow();
        });
        assert_eq!(Some(&c as *const Collector), x);
    }
}
