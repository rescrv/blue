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
    /// Create a new counter with the provided label.
    pub const fn new(label: &'static str) -> Counter {
        Counter {
            label,
            count: AtomicU64::new(0),
        }
    }

    /// Increment the counter by one.
    #[inline(always)]
    pub fn click(&self) {
        self.count(1)
    }

    /// Increment the counter by `x`.
    #[inline(always)]
    pub fn count(&self, x: u64) {
        self.count.fetch_add(x, Ordering::Relaxed);
    }
}

impl Sensor for Counter {
    type Reading = u64;

    #[inline(always)]
    fn label(&self) -> &'static str {
        self.label
    }

    #[inline(always)]
    fn read(&self) -> u64 {
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
    /// Create a new Gauge from the provided label.
    pub const fn new(label: &'static str) -> Gauge {
        Gauge {
            label,
            value: AtomicU64::new(GAUGE_INIT),
        }
    }

    /// Set the value of the gauge.
    #[inline(always)]
    pub fn set(&self, x: f64) {
        self.value.store(x.to_bits(), Ordering::Relaxed);
    }
}

impl Sensor for Gauge {
    type Reading = f64;

    #[inline(always)]
    fn label(&self) -> &'static str {
        self.label
    }

    #[inline(always)]
    fn read(&self) -> f64 {
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
    /// Create a new set of moments with the provided label.
    pub const fn new(label: &'static str) -> Self {
        Self {
            label,
            value: Mutex::new(moments::Moments::new()),
        }
    }

    /// Add the provided f64 to the accumulated moments.
    pub fn add(&self, x: f64) {
        let mut value = self.value.lock().unwrap();
        value.push(x);
    }
}

impl Sensor for Moments {
    type Reading = moments::Moments;

    #[inline(always)]
    fn label(&self) -> &'static str {
        self.label
    }

    #[inline(always)]
    fn read(&self) -> moments::Moments {
        let value = self.value.lock().unwrap();
        *value
    }
}

///////////////////////////////////////////// Histogram ////////////////////////////////////////////

pub trait HistogramImpl: Send + Sync {
    fn observe(&self, x: f64) -> Result<(), sig_fig_histogram::Error>;
    fn observe_n(&self, x: f64, n: u64) -> Result<(), sig_fig_histogram::Error>;
    fn to_histogram(&self) -> sig_fig_histogram::Histogram;
}

impl<const N: usize> HistogramImpl for sig_fig_histogram::LockFreeHistogram<N> {
    fn observe(&self, x: f64) -> Result<(), sig_fig_histogram::Error> {
        sig_fig_histogram::LockFreeHistogram::<N>::observe(self, x)
    }

    fn observe_n(&self, x: f64, n: u64) -> Result<(), sig_fig_histogram::Error> {
        sig_fig_histogram::LockFreeHistogram::<N>::observe_n(self, x, n)
    }

    fn to_histogram(&self) -> sig_fig_histogram::Histogram {
        sig_fig_histogram::LockFreeHistogram::<N>::to_histogram(self)
    }
}

pub struct Histogram {
    label: &'static str,
    histogram: &'static dyn HistogramImpl,
    exceeds_max: Counter,
    is_negative: Counter,
}

impl Histogram {
    pub const fn new(label: &'static str, histogram: &'static dyn HistogramImpl) -> Self {
        let exceeds_max = Counter::new(label);
        let is_negative = Counter::new(label);
        Self {
            label,
            histogram,
            exceeds_max,
            is_negative,
        }
    }
}

impl Histogram {
    pub fn exceeds_max(&self) -> &Counter {
        &self.exceeds_max
    }

    pub fn is_negative(&self) -> &Counter {
        &self.is_negative
    }

    pub fn observe(&self, x: f64) {
        match self.histogram.observe(x) {
            Ok(()) => {}
            Err(sig_fig_histogram::Error::ExceedsMax) => {
                self.exceeds_max.click();
            }
            Err(sig_fig_histogram::Error::IsNegative) => {
                self.is_negative.click();
            }
        }
    }

    pub fn observe_n(&self, x: f64, n: u64) {
        match self.histogram.observe_n(x, n) {
            Ok(()) => {}
            Err(sig_fig_histogram::Error::ExceedsMax) => {
                self.exceeds_max.click();
            }
            Err(sig_fig_histogram::Error::IsNegative) => {
                self.is_negative.click();
            }
        }
    }
}

impl Sensor for Histogram {
    type Reading = sig_fig_histogram::Histogram;

    #[inline(always)]
    fn label(&self) -> &'static str {
        self.label
    }

    #[inline(always)]
    fn read(&self) -> sig_fig_histogram::Histogram {
        self.histogram.to_histogram()
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
