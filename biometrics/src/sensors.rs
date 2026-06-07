//! Sensors that implement the [Sensor] trait.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::Sensor;
use crate::moments;

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

/// Stores histogram observations for a [`Histogram`] sensor.
///
/// Implementations provide the storage strategy.  The crate ships an implementation for
/// [`sig_fig_histogram::LockFreeHistogram`], which is suitable for static sensors shared across
/// threads.
pub trait HistogramImpl: Send + Sync {
    /// Record one observation.
    fn observe(&self, x: f64) -> Result<(), sig_fig_histogram::Error>;
    /// Record `n` copies of one observation.
    fn observe_n(&self, x: f64, n: u64) -> Result<(), sig_fig_histogram::Error>;
    /// Convert the current state to an owned histogram reading.
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

/// [Histogram] captures a distribution of non-negative floating point observations.
///
/// Invalid observations do not escape as errors.  Instead, observations that exceed the backing
/// histogram's maximum bucket increment [`Histogram::exceeds_max`], while negative observations
/// increment [`Histogram::is_negative`].
pub struct Histogram {
    label: &'static str,
    histogram: &'static dyn HistogramImpl,
    exceeds_max: Counter,
    is_negative: Counter,
}

impl Histogram {
    /// Create a new histogram sensor with the provided label and backing implementation.
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
    /// Return the counter for observations that exceed the backing histogram's maximum bucket.
    pub fn exceeds_max(&self) -> &Counter {
        &self.exceeds_max
    }

    /// Return the counter for observations that are negative.
    pub fn is_negative(&self) -> &Counter {
        &self.is_negative
    }

    /// Record one observation or count why it could not be recorded.
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

    /// Record `n` copies of one observation or count why they could not be recorded.
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
    fn counter_counts_clicks_and_batches() {
        let counter = Counter::new("counter.counts.clicks.and.batches");

        assert_eq!("counter.counts.clicks.and.batches", counter.label());
        assert_eq!(0, counter.read());
        counter.click();
        counter.count(41);
        assert_eq!(42, counter.read());
    }

    #[test]
    fn gauge_may_be_static() {
        static _GAUGE: Gauge = Gauge::new("gauge.may.be.static");
    }

    #[test]
    fn gauge_sets_floating_point_value() {
        let gauge = Gauge::new("gauge.sets.floating.point.value");

        assert_eq!("gauge.sets.floating.point.value", gauge.label());
        assert_eq!(0.0_f64.to_bits(), gauge.read().to_bits());
        gauge.set(-13.5);
        assert_eq!((-13.5_f64).to_bits(), gauge.read().to_bits());
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
        assert_eq!(
            moments::Moments {
                n: 3,
                m1: 5.0,
                m2: 50.0,
                m3: 0.0,
                m4: 1250.0,
            },
            MOMENTS.read()
        );
    }

    #[test]
    fn histogram() {
        static HISTOGRAM: sig_fig_histogram::LockFreeHistogram<1000> =
            sig_fig_histogram::LockFreeHistogram::new(3);
        static HISTOGRAM_SENSOR: Histogram = Histogram::new("histogram", &HISTOGRAM);
        HISTOGRAM_SENSOR.observe(0.0);
        HISTOGRAM_SENSOR.observe(5.0);
        HISTOGRAM_SENSOR.observe(10.0);
    }

    #[test]
    fn histogram_records_observations_and_error_counters() {
        static HISTOGRAM: sig_fig_histogram::LockFreeHistogram<1> =
            sig_fig_histogram::LockFreeHistogram::new(2);
        static HISTOGRAM_SENSOR: Histogram = Histogram::new(
            "histogram.records.observations.and.error.counters",
            &HISTOGRAM,
        );

        HISTOGRAM_SENSOR.observe_n(1.0, 4);
        HISTOGRAM_SENSOR.observe(2.0);
        HISTOGRAM_SENSOR.observe(-1.0);

        assert_eq!(
            vec![(1.0, 4)],
            HISTOGRAM_SENSOR.read().iter().collect::<Vec<_>>()
        );
        assert_eq!(1, HISTOGRAM_SENSOR.exceeds_max().read());
        assert_eq!(1, HISTOGRAM_SENSOR.is_negative().read());
    }
}
