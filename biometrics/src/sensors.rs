use std::fmt;
use std::sync::atomic::{AtomicBool,AtomicU64};
use std::sync::atomic::Ordering;

use crate::moments;
use crate::{register_counter,register_gauge,register_moments};

////////////////////////////////////////////// Counter /////////////////////////////////////////////

pub struct Counter {
    what: &'static str,
    count: AtomicU64,
    init: AtomicBool,
}

impl Counter {
    pub const fn new(what: &'static str) -> Counter {
        Counter {
            what,
            count: AtomicU64::new(0),
            init: AtomicBool::new(false),
        }
    }

    #[inline(always)]
    pub fn what(&self) -> &'static str {
        self.what
    }

    #[inline(always)]
    pub fn click(&self) {
        self.count(1)
    }

    #[inline(always)]
    pub fn count(&self, x: u64) {
        if !self.init.load(Ordering::Relaxed) {
            // This can race.  That is OK.
            self.init.store(register_counter(self as *const Counter), Ordering::Relaxed);
        }
        self.count.fetch_add(x, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn read(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    pub fn mark_registered(&self) {
        self.init.store(true, Ordering::Relaxed);
    }
}

impl fmt::Debug for Counter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "counter({})", self.read())
    }
}

/////////////////////////////////////////////// Gauge //////////////////////////////////////////////

const GAUGE_INIT: u64 = 0;

pub struct Gauge {
    what: &'static str,
    value: AtomicU64,
    init: AtomicBool,
}

impl Gauge {
    pub const fn new(what: &'static str) -> Gauge {
        Gauge {
            what,
            value: AtomicU64::new(GAUGE_INIT),
            init: AtomicBool::new(false),
        }
    }

    #[inline(always)]
    pub fn what(&self) -> &'static str {
        self.what
    }

    #[inline(always)]
    pub fn set(&self, x: f64) {
        if !self.init.load(Ordering::Relaxed) {
            // This can race.  That is OK.
            self.init.store(register_gauge(self as *const Gauge), Ordering::Relaxed);
        }
        self.value.store(x.to_bits(), Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn read(&self) -> f64 {
        let u = self.value.load(Ordering::Relaxed);
        f64::from_bits(u)
    }

    pub fn mark_registered(&self) {
        self.init.store(true, Ordering::Relaxed);
    }
}

impl fmt::Debug for Gauge {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "gauge({})", self.read())
    }
}

////////////////////////////////////////////// Moments /////////////////////////////////////////////

pub struct Moments {
    what: &'static str,
    spin: AtomicBool,
    init: AtomicBool,
    value: moments::Moments,
}

impl Moments {
    pub const fn new(what: &'static str) -> Self {
        Self {
            what,
            spin: AtomicBool::new(false),
            init: AtomicBool::new(false),
            value: moments::Moments::new(),
        }
    }

    #[inline(always)]
    pub fn what(&self) -> &'static str {
        self.what
    }

    pub fn add(&self, x: f64) {
        if !self.init.load(Ordering::Relaxed) {
            // This can race.  That is OK.
            self.init.store(register_moments(self as *const Moments), Ordering::Relaxed);
        }
        self.lock();
        self.ptr().push(x);
        self.unlock();
    }

    pub fn read(&self) -> moments::Moments {
        self.lock();
        let value = self.value.clone();
        self.unlock();
        value
    }

    pub fn mark_registered(&self) {
        self.init.store(true, Ordering::Relaxed);
    }

    #[inline(always)]
    fn lock(&self) {
        loop {
            match self.spin.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed) {
                Ok(_) => { break; }
                Err(_) => { std::hint::spin_loop(); }
            }
        }
    }

    #[inline(always)]
    fn ptr(&self) -> &mut moments::Moments {
        unsafe {
            let p = self as *const Self;
            let p = p as *mut Self;
            &mut (*p).value
        }
    }

    #[inline(always)]
    fn unlock(&self) {
        self.spin.store(false, Ordering::Release);
    }
}

impl fmt::Debug for Moments {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let moments = self.read();
        write!(f, "moments({}, {}, {}, {}, {})",
            moments.n, moments.m1, moments.m2, moments.m3, moments.m4)
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gauge_init() {
        let x: f64 = 0.0;
        let y: u64 = x.to_bits();
        assert_eq!(y, GAUGE_INIT);
    }

    #[test]
    fn counter_may_be_static() {
        static COUNTER: Counter = Counter::new("counter.may.be.static");
        println!("{:?}", COUNTER);
    }

    #[test]
    fn gauge_may_be_static() {
        static GAUGE: Gauge = Gauge::new("gauge.may.be.static");
        println!("{:?}", GAUGE);
    }

    #[test]
    fn sync_moments_may_be_static() {
        static MOMENTS: Moments = Moments::new("sync.moments.may.be.static");
        println!("{:?}", MOMENTS);
    }

    #[test]
    fn sync_moments_multiple_add() {
        static MOMENTS: Moments = Moments::new("sync.moments.multiple.add");
        MOMENTS.add(0.0);
        MOMENTS.add(5.0);
        MOMENTS.add(10.0);
        assert_eq!(MOMENTS.read().n(), 3);
        assert_eq!(MOMENTS.read().mean(), 5.0);
    }
}
