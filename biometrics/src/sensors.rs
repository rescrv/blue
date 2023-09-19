//! Sensors that implement the [Sensor] trait.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use crate::moments;
use crate::Sensor;

////////////////////////////////////////////// Counter /////////////////////////////////////////////

/// [Counter] captures a monotonically increasing value.
pub struct Counter {
    label: &'static str,
    count: AtomicU64,
}

impl Counter {
    pub const fn new(label: &'static str) -> Counter {
        Counter {
            label,
            count: AtomicU64::new(0),
        }
    }

    #[inline(always)]
    pub fn click(&'static self) {
        self.count(1)
    }

    #[inline(always)]
    pub fn count(&'static self, x: u64) {
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
}

/////////////////////////////////////////////// Gauge //////////////////////////////////////////////

const GAUGE_INIT: u64 = 0;

/// [Gauge] captures a floating point value.
pub struct Gauge {
    label: &'static str,
    value: AtomicU64,
}

impl Gauge {
    pub const fn new(label: &'static str) -> Gauge {
        Gauge {
            label,
            value: AtomicU64::new(GAUGE_INIT),
        }
    }

    #[inline(always)]
    pub fn set(&'static self, x: f64) {
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
}

////////////////////////////////////////////// Moments /////////////////////////////////////////////

/// [Moments] captures mean, stdev, skewness, and kurtosis.
pub struct Moments {
    label: &'static str,
    value: Mutex<moments::Moments>,
}

impl Moments {
    pub const fn new(label: &'static str) -> Self {
        Self {
            label,
            value: Mutex::new(moments::Moments::new()),
        }
    }

    pub fn add(&'static self, x: f64) {
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
}
