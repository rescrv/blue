//! saros is a simplistic time-series database.  It provides interfaces for storing biometrics data
//! and querying them.  It is intended to favor simplicity over feature-richness.  Consequently,
//! it's quite simple.

use std::cmp::Ordering;
use std::ops::{Add, Bound, RangeBounds, Sub};
use std::sync::Arc;

use biometrics::moments::Moments;
use biometrics::Counter;
use one_two_eight::generate_id;
use tatl::{HeyListen, Stationary};
use zerror::{iotoz, Z};
use zerror_core::ErrorCore;

pub mod dashboard;
pub mod memory;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static DROPPED_METRICS: Counter = Counter::new("saros.dropped_metrics");
static DROPPED_METRICS_MONITOR: Stationary = Stationary::new("saros.dropped_metrics", &DROPPED_METRICS);

static TIME_TRAVEL: Counter = Counter::new("saros.time_travel");
static TIME_TRAVEL_MONITOR: Stationary = Stationary::new("saros.time_travel", &TIME_TRAVEL);

/// Register this crate's biometrics.
pub fn register_biometrics(collector: &biometrics::Collector) {
    collector.register_counter(&DROPPED_METRICS);
    collector.register_counter(&TIME_TRAVEL);
}

/// Register this crate's monitors.
pub fn register_monitors(hey_listen: &mut HeyListen) {
    hey_listen.register_stationary(&DROPPED_METRICS_MONITOR);
    hey_listen.register_stationary(&TIME_TRAVEL_MONITOR);
}

/////////////////////////////////////////////// Error //////////////////////////////////////////////

/// Error captures the ways Saros can fail.
#[derive(Clone, zerror_derive::Z)]
pub enum Error {
    /// A system error was encountered (usually std::io::Error).
    SystemError {
        core: ErrorCore,
        what: String,
    },
    /// The query engine requested an unknown metric ID.
    UnknownMetric {
        core: ErrorCore,
        metric_id: MetricID,
    },
}

iotoz! {Error}

impl From<std::io::Error> for Error {
    fn from(what: std::io::Error) -> Error {
        Error::SystemError {
            core: ErrorCore::default(),
            what: format!("{:?}", what),
        }
    }
}

///////////////////////////////////////////// MetricID /////////////////////////////////////////////

generate_id! {MetricID, "metric:"}

/////////////////////////////////////////////// Label //////////////////////////////////////////////

/// Label is the text-identifier for series.
#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub struct Label(String);

impl<S: AsRef<str>> From<S> for Label {
    fn from(s: S) -> Self {
        Self(s.as_ref().to_string())
    }
}

/////////////////////////////////////////////// Tags ///////////////////////////////////////////////

/// Series may be optionally tagged by Tags.
#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub struct Tags(Vec<String>);

impl Tags {
    /// True if every tag in [other] is in [self].
    fn contains(&self, other: &Tags) -> bool {
        other.0.iter().all(|t| self.0.contains(t))
    }
}

//////////////////////////////////////////// MetricType ////////////////////////////////////////////

/// The type of metric being requested.  A switch over biometrics sensor types.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub enum MetricType {
    #[default]
    Counter,
    Gauge,
    Moments,
}

/////////////////////////////////////////////// Time ///////////////////////////////////////////////

/// Time since UNIX epoch, in microseconds.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Time(i64);

impl Time {
    pub fn from_secs(s: i64) -> Self {
        Self(s.saturating_mul(1_000_000))
    }
}

impl Add<Time> for Time {
    type Output = Time;

    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

impl Sub<Time> for Time {
    type Output = Time;

    fn sub(self, other: Self) -> Self {
        Self(self.0 - other.0)
    }
}

////////////////////////////////////////////// Window //////////////////////////////////////////////

/// A Window has a start and end time.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Window(pub Time, pub Time);

/////////////////////////////////////////////// Point //////////////////////////////////////////////

/// A Point is an algebraic type that supports Add and Sub on itself.  Implemented for each metric
/// type.
pub trait Point: Copy + Default + Add<Self, Output = Self> + Sub<Self, Output = Self> + Sized {}

impl Point for i64 {}
impl Point for f64 {}
impl Point for Moments {}

////////////////////////////////////////////// Series //////////////////////////////////////////////

/// A Series is an ordered list of (time, point) tuples.
#[derive(Clone, Debug, Default)]
pub struct Series<P: Point> {
    points: Vec<(Time, P)>,
}

impl<P: Point> Series<P> {
    /// True if this series has no points.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// The number of points in this series.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Filter the series to just the data points for the window.
    pub fn filter(&self, window: Window) -> Self {
        let mut points = vec![];
        for (t, p) in self.points.iter() {
            if (Bound::Unbounded, Bound::Included(window.0)).contains(t) {
                if !points.is_empty() {
                    points.pop();
                }
                points.push((*t, *p));
            } else if (Bound::Excluded(window.1), Bound::Unbounded).contains(t) {
                break;
            } else {
                points.push((*t, *p));
            }
        }
        Self { points }
    }

    /// Downsample the series to the prescribed number of buckets.
    pub fn buckets(&self, window: Window, buckets: usize) -> Self {
        let delta = Time((window.1 - window.0).0 / buckets as i64);
        let mut thresh = window.0;
        let mut points = vec![];
        for (t, p) in self.points.iter() {
            if thresh > *t && !points.is_empty() {
                points.pop();
            }
            points.push((*t, *p));
            if thresh <= *t {
                thresh = thresh + delta;
            }
        }
        while points.len() > buckets {
            points.pop();
        }
        Self { points }
    }

    /// Merge two series according to the point-wise merge function.
    pub fn merge<F: FnMut(P, P) -> P>(lhs: &Self, rhs: &Self, mut f: F) -> Self {
        let mut points = vec![];
        let mut lhs = lhs.points.iter().copied();
        let mut rhs = rhs.points.iter().copied();
        let mut lhs_next = lhs.next();
        let mut rhs_next = rhs.next();
        let mut lhs_prev = None;
        let mut rhs_prev = None;
        while let (Some(lhs_point), Some(rhs_point)) = (lhs_next, rhs_next) {
            match lhs_point.0.cmp(&rhs_point.0) {
                Ordering::Equal => {
                    points.push((lhs_point.0, f(lhs_point.1, rhs_point.1)));
                    lhs_prev = lhs_next;
                    rhs_prev = rhs_next;
                    lhs_next = lhs.next();
                    rhs_next = rhs.next();
                }
                Ordering::Less => {
                    if let Some(rhs_prev) = rhs_prev {
                        points.push((lhs_point.0, f(lhs_point.1, rhs_prev.1)));
                    } else {
                        points.push((lhs_point.0, lhs_point.1));
                    }
                    lhs_prev = lhs_next;
                    lhs_next = lhs.next();
                }
                Ordering::Greater => {
                    if let Some(lhs_prev) = lhs_prev {
                        points.push((rhs_point.0, f(rhs_point.1, lhs_prev.1)));
                    } else {
                        points.push((rhs_point.0, rhs_point.1));
                    }
                    rhs_prev = rhs_next;
                    rhs_next = rhs.next();
                }
            }
        }
        while let Some(lhs_point) = lhs_next {
            points.push(lhs_point);
            lhs_next = lhs.next();
        }
        while let Some(rhs_point) = rhs_next {
            points.push(rhs_point);
            rhs_next = rhs.next();
        }
        Self { points }
    }
}

impl Series<i64> {
    /// Convert the series to a series of f64 points.
    fn as_f64(&self) -> Series<f64> {
        let points = self.points.iter().map(|(t, p)| (*t, *p as f64)).collect();
        Series {
            points,
        }
    }
}

impl Series<f64> {
    /// Convert the series to a series of f64 points.
    fn as_f64(&self) -> Series<f64> {
        self.clone()
    }
}

impl Series<Moments> {
    /// Convert the series to a series of f64 points.
    fn as_f64(&self) -> Series<f64> {
        let points = self.points.iter().map(|(t, p)| (*t, p.n() as f64 * p.mean())).collect();
        Series {
            points,
        }
    }
}

impl<P: Point> Add<Series<P>> for Series<P> {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self::merge(&self, &other, P::add)
    }
}

impl<P: Point> Sub<Series<P>> for Series<P> {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self::merge(&self, &other, P::sub)
    }
}

/////////////////////////////////////////// LabeledSeries //////////////////////////////////////////

/// A LabeledSeries associates a label and tags with a series.
pub struct LabeledSeries {
    label: Label,
    tags: Tags,
    series: Series<f64>,
}

impl LabeledSeries {
    fn label_with_tags(&self) -> String {
        format!("{} {}", self.label.0, self.tags.0.join("|")).trim().to_string()
    }
}

/////////////////////////////////////////////// Query //////////////////////////////////////////////

/// A Query represents a logical set of time series to be retrieved and arranged.
#[derive(Clone, Debug)]
pub enum Query {
    Simple(Label, Tags),
    Union(Vec<Query>),
}

////////////////////////////////////////// BiometricsStore /////////////////////////////////////////

/// BiometricsStore captures recordings from a biometrics emitter.
pub trait BiometricsStore {
    /// Return the metric IDs for the given label and tags.  The metrics may have more tags, but
    /// will have at least the tags specified.
    fn metrics_by_label(
        &self,
        metric_type: MetricType,
        label: &Label,
        tags: &Tags,
        window: Window,
    ) -> Result<Vec<MetricID>, Error>;

    /// Return a single time series for a counter.
    fn counter_by_metric_id(
        &self,
        metric_id: MetricID,
        window: Window,
    ) -> Result<Series<i64>, Error>;

    /// Return a single time series for a gauge.
    fn gauge_by_metric_id(
        &self,
        metric_id: MetricID,
        window: Window,
    ) -> Result<Series<f64>, Error>;

    /// Return a single time series for moments.
    fn moments_by_metric_id(
        &self,
        metric_id: MetricID,
        window: Window,
    ) -> Result<Series<Moments>, Error>;
}

impl<B: BiometricsStore> BiometricsStore for Arc<B> {
    fn metrics_by_label(
        &self,
        metric_type: MetricType,
        label: &Label,
        tags: &Tags,
        window: Window,
    ) -> Result<Vec<MetricID>, Error> {
        <B as BiometricsStore>::metrics_by_label(self, metric_type, label, tags, window)
    }

    fn counter_by_metric_id(
        &self,
        metric_id: MetricID,
        window: Window,
    ) -> Result<Series<i64>, Error> {
        <B as BiometricsStore>::counter_by_metric_id(self, metric_id, window)
    }

    fn gauge_by_metric_id(
        &self,
        metric_id: MetricID,
        window: Window,
    ) -> Result<Series<f64>, Error> {
        <B as BiometricsStore>::gauge_by_metric_id(self, metric_id, window)
    }

    fn moments_by_metric_id(
        &self,
        metric_id: MetricID,
        window: Window,
    ) -> Result<Series<Moments>, Error> {
        <B as BiometricsStore>::moments_by_metric_id(self, metric_id, window)
    }
}

//////////////////////////////////////////// QueryEngine ///////////////////////////////////////////

/// QueryEngine executes queries against one or more BiometricsStore instances.
pub struct QueryEngine {
    biometrics: Vec<Arc<dyn BiometricsStore>>,
}

impl QueryEngine {
    /// Create a new QueryEngine.
    pub const fn new() -> Self {
        Self {
            biometrics: vec![],
        }
    }

    /// Add a BiometricsStore to this query engine.
    pub fn with_biometrics_store<B: BiometricsStore + 'static>(mut self, bs: B) -> Self {
        self.biometrics.push(Arc::new(bs) as _);
        self
    }

    /// Execute the query against the store, restricting it to the provided window and number of
    /// buckets.
    pub fn query(&self, q: &Query, window: Window, buckets: usize) -> Result<Vec<LabeledSeries>, Error> {
        match q {
            Query::Simple(label, tags) => {
                let series = self.series_for_label(label, tags, window, buckets)?;
                Ok(vec![
                    LabeledSeries {
                        label: label.clone(),
                        tags: tags.clone(),
                        series,
                    }
                ])
            }
            Query::Union(queries) => {
                let mut series = vec![];
                for q in queries.iter() {
                    series.append(&mut self.query(q, window, buckets)?);
                }
                Ok(series)
            }
        }
    }

    fn series_for_label(
        &self,
        label: &Label,
        tags: &Tags,
        window: Window,
        buckets: usize,
    ) -> Result<Series<f64>, Error> {
        let mut agg: Series<f64> = Series::default();
        for biometrics in self.biometrics.iter() {
            let counters = biometrics.metrics_by_label(MetricType::Counter, label, tags, window)?;
            for counter in counters {
                let series = biometrics.counter_by_metric_id(counter, window)?.as_f64();
                agg = agg + series;
            }
            let gauges = biometrics.metrics_by_label(MetricType::Gauge, label, tags, window)?;
            for gauge in gauges {
                let series = biometrics.gauge_by_metric_id(gauge, window)?.as_f64();
                agg = agg + series;
            }
            let moments = biometrics.metrics_by_label(MetricType::Moments, label, tags, window)?;
            for moments in moments {
                let series = biometrics.moments_by_metric_id(moments, window)?.as_f64();
                agg = agg + series;
            }
        }
        Ok(agg.buckets(window, buckets))
    }
}
