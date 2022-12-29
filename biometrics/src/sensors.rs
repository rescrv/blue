use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

use crate::moments;
use crate::t_digest;
use crate::{register_counter,register_gauge,register_moments, register_t_digest, Sensor};

////////////////////////////////////////////// Counter /////////////////////////////////////////////

pub struct Counter {
    label: &'static str,
    count: AtomicU64,
    init: AtomicBool,
}

impl Counter {
    pub const fn new(label: &'static str) -> Counter {
        Counter {
            label,
            count: AtomicU64::new(0),
            init: AtomicBool::new(false),
        }
    }

    #[inline(always)]
    pub fn click(&'static self) {
        self.count(1)
    }

    #[inline(always)]
    pub fn count(&'static self, x: u64) {
        if !self.init.load(Ordering::Relaxed) {
            // This can race.  That is OK.
            self.init.store(register_counter(self), Ordering::Relaxed);
        }
        self.count.fetch_add(x, Ordering::Relaxed);
    }
}

impl Sensor for Counter {
    type Reading = u64;

    #[inline(always)]
    fn label(&'static self) -> &'static str {
        self.label
    }

    #[inline(always)]
    fn read(&'static self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    #[inline(always)]
    fn mark_registered(&'static self) {
        self.init.store(true, Ordering::Relaxed);
    }
}

impl Eq for &'static Counter {
}

impl PartialEq for &'static Counter {
    fn eq(&self, rhs: &&'static Counter) -> bool {
        *self as *const Counter == *rhs as *const Counter
    }
}

impl std::hash::Hash for &'static Counter {
    fn hash<H>(&self, state: &mut H)
        where H: std::hash::Hasher
        {
            (*self as *const Counter).hash(state)
        }
}

/////////////////////////////////////////////// Gauge //////////////////////////////////////////////

const GAUGE_INIT: u64 = 0;

pub struct Gauge {
    label: &'static str,
    value: AtomicU64,
    init: AtomicBool,
}

impl Gauge {
    pub const fn new(label: &'static str) -> Gauge {
        Gauge {
            label,
            value: AtomicU64::new(GAUGE_INIT),
            init: AtomicBool::new(false),
        }
    }

    #[inline(always)]
    pub fn set(&'static self, x: f64) {
        if !self.init.load(Ordering::Relaxed) {
            // This can race.  That is OK.
            self.init.store(register_gauge(self), Ordering::Relaxed);
        }
        self.value.store(x.to_bits(), Ordering::Relaxed);
    }
}

impl Sensor for Gauge {
    type Reading = f64;

    #[inline(always)]
    fn label(&'static self) -> &'static str {
        self.label
    }

    #[inline(always)]
    fn read(&'static self) -> f64 {
        let u = self.value.load(Ordering::Relaxed);
        f64::from_bits(u)
    }

    #[inline(always)]
    fn mark_registered(&'static self) {
        self.init.store(true, Ordering::Relaxed);
    }
}

impl Eq for &'static Gauge {
}

impl PartialEq for &'static Gauge {
    fn eq(&self, rhs: &&'static Gauge) -> bool {
        *self as *const Gauge == *rhs as *const Gauge
    }
}

impl std::hash::Hash for &'static Gauge {
    fn hash<H>(&self, state: &mut H)
        where H: std::hash::Hasher
        {
            (*self as *const Gauge).hash(state)
        }
}

////////////////////////////////////////////// Moments /////////////////////////////////////////////

pub struct Moments {
    label: &'static str,
    value: Mutex<moments::Moments>,
    init: AtomicBool,
}

impl Moments {
    pub const fn new(label: &'static str) -> Self {
        Self {
            label,
            value: Mutex::new(moments::Moments::new()),
            init: AtomicBool::new(false),
        }
    }

    pub fn add(&'static self, x: f64) {
        if !self.init.load(Ordering::Relaxed) {
            // This can race.  That is OK.
            self.init.store(register_moments(self), Ordering::Relaxed);
        }
        let mut value = self.value.lock().unwrap();
        value.push(x);
    }
}

impl Sensor for Moments {
    type Reading = moments::Moments;

    #[inline(always)]
    fn label(&'static self) -> &'static str {
        self.label
    }

    #[inline(always)]
    fn read(&'static self) -> moments::Moments {
        let value = self.value.lock().unwrap();
        value.clone()
    }

    #[inline(always)]
    fn mark_registered(&'static self) {
        self.init.store(true, Ordering::Relaxed);
    }
}

impl Eq for &'static Moments {
}

impl PartialEq for &'static Moments {
    fn eq(&self, rhs: &&'static Moments) -> bool {
        *self as *const Moments == *rhs as *const Moments
    }
}

impl std::hash::Hash for &'static Moments {
    fn hash<H>(&self, state: &mut H)
        where H: std::hash::Hasher
        {
            (*self as *const Moments).hash(state)
        }
}

////////////////////////////////////////////// TDigest /////////////////////////////////////////////

pub struct TDigest {
    label: &'static str,
    value: Mutex<t_digest::TDigest>,
    init: AtomicBool,
}

impl TDigest {
    pub const fn new(label: &'static str, delta: u64) -> Self {
        Self {
            label,
            init: AtomicBool::new(false),
            value: Mutex::new(t_digest::TDigest::new(delta)),
        }
    }

    pub fn add(&'static self, point: f64) {
        if !self.init.load(Ordering::Relaxed) {
            // This can race.  That is OK.
            self.init.store(register_t_digest(self), Ordering::Relaxed);
        }
        let mut value = self.value.lock().unwrap();
        value.add(point);
    }
}

impl Sensor for TDigest {
    type Reading = t_digest::TDigest;

    #[inline(always)]
    fn label(&'static self) -> &'static str {
        self.label
    }

    #[inline(always)]
    fn read(&'static self) -> t_digest::TDigest {
        let value = self.value.lock().unwrap();
        value.clone()
    }

    #[inline(always)]
    fn mark_registered(&'static self) {
        self.init.store(true, Ordering::Relaxed);
    }
}

impl Eq for &'static TDigest {
}

impl PartialEq for &'static TDigest {
    fn eq(&self, rhs: &&'static TDigest) -> bool {
        *self as *const TDigest == *rhs as *const TDigest
    }
}

impl std::hash::Hash for &'static TDigest {
    fn hash<H>(&self, state: &mut H)
        where H: std::hash::Hasher
        {
            (*self as *const TDigest).hash(state)
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
        static _COUNTER: Counter = Counter::new("counter.may.be.static");
        _COUNTER.click();
    }

    #[test]
    fn gauge_may_be_static() {
        static _GAUGE: Gauge = Gauge::new("gauge.may.be.static");
    }

    #[test]
    fn sync_moments_may_be_static() {
        static _MOMENTS: Moments = Moments::new("sync.moments.may.be.static");
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

    #[test]
    fn sync_t_digest_may_be_static() {
        static _T_DIGEST: TDigest = TDigest::new("sync.moments.may.be.static", 1000);
    }
}
