#![doc = include_str!("../README.md")]

use std::cmp::Ordering;
use std::fmt::Debug;
use std::ops::{Add, Div, Mul, Rem, Sub};
use std::time::Duration;

use chrono::{DateTime, Utc};

use biometrics::Counter;
use one_two_eight::generate_id;
use tag_index::Tags;
use tatl::{HeyListen, Stationary};
use zerror::{iotoz, Z};
use zerror_core::ErrorCore;

pub mod coding;
pub mod delta_array;
pub mod prometheus;
pub mod query;
pub mod querylang;
pub mod recovery;
pub mod support_nom;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static DROPPED_METRICS: Counter = Counter::new("saros.dropped_metrics");
static DROPPED_METRICS_MONITOR: Stationary =
    Stationary::new("saros.dropped_metrics", &DROPPED_METRICS);

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
#[derive(Clone, prototk_derive::Message, zerror_derive::Z)]
pub enum Error {
    /// A successful operation.
    #[prototk(1, message)]
    Success {
        #[prototk(1, message)]
        core: ErrorCore,
    },
    /// A system error was encountered (usually std::io::Error).
    #[prototk(2, message)]
    SystemError {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        what: String,
    },
    /// There's a query that takes a lookback nested within a query that takes a lookback.
    #[prototk(3, message)]
    NestedLookback {
        #[prototk(1, message)]
        core: ErrorCore,
    },
    /// There's a parameter that's doesn't obey the even-divisor rule.
    #[prototk(4, message)]
    NonMultipleParameter {
        #[prototk(1, message)]
        core: ErrorCore,
    },
    /// The lookback is more than the range that can be represented via a Time type.
    #[prototk(5, message)]
    LookbackTooLarge {
        #[prototk(1, message)]
        core: ErrorCore,
    },
    /// There was an error converting time.
    #[prototk(6, message)]
    TimeError {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        what: String,
    },
    /// There's an error in the decoding of time series.
    #[prototk(7, message)]
    CodingError {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        what: String,
    },
    /// There's an error in the decoding of time series.
    #[prototk(8, message)]
    ArithmeticError {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        what: String,
    },
    /// There's an error in the text-representation of time series.
    #[prototk(9, message)]
    TextError {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        what: String,
    },
    /// RPC error.
    #[prototk(10, message)]
    PrototkError {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, message)]
        what: prototk::Error,
    },
    /// RPC error.
    #[prototk(11, message)]
    RpcPbError {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, message)]
        what: rpc_pb::Error,
    },
    /// Parse error.
    #[prototk(12, message)]
    ParseError {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        what: String,
    },
    /// Internally-enforced invariants were not upheld.
    #[prototk(15, message)]
    InternalError {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        what: String,
    },
}

iotoz! {Error}

impl Error {
    pub fn arithmetic<S: AsRef<str>>(s: S) -> Self {
        Self::ArithmeticError {
            core: ErrorCore::default(),
            what: s.as_ref().to_string(),
        }
    }

    pub fn coding<S: AsRef<str>>(s: S) -> Self {
        Self::CodingError {
            core: ErrorCore::default(),
            what: s.as_ref().to_string(),
        }
    }

    pub fn internal<S: AsRef<str>>(s: S) -> Self {
        Self::InternalError {
            core: ErrorCore::default(),
            what: s.as_ref().to_string(),
        }
    }

    pub fn success() -> Self {
        Self::Success {
            core: ErrorCore::default(),
        }
    }

    pub fn system<S: AsRef<str>>(s: S) -> Self {
        Self::SystemError {
            core: ErrorCore::default(),
            what: s.as_ref().to_string(),
        }
    }

    pub fn text<S: AsRef<str>>(s: S) -> Self {
        Self::TextError {
            core: ErrorCore::default(),
            what: s.as_ref().to_string(),
        }
    }

    pub fn time<S: AsRef<str>>(s: S) -> Self {
        Self::TimeError {
            core: ErrorCore::default(),
            what: s.as_ref().to_string(),
        }
    }
}

impl Default for Error {
    fn default() -> Self {
        Self::success()
    }
}

impl From<std::io::Error> for Error {
    fn from(what: std::io::Error) -> Error {
        Error::SystemError {
            core: ErrorCore::default(),
            what: format!("{what:?}"),
        }
    }
}

impl From<chrono::RoundingError> for Error {
    fn from(what: chrono::RoundingError) -> Error {
        Error::TimeError {
            core: ErrorCore::default(),
            what: format!("{what:?}"),
        }
    }
}

impl From<prototk::Error> for Error {
    fn from(what: prototk::Error) -> Error {
        Error::PrototkError {
            core: ErrorCore::default(),
            what,
        }
    }
}

impl From<rpc_pb::Error> for Error {
    fn from(what: rpc_pb::Error) -> Error {
        Error::RpcPbError {
            core: ErrorCore::default(),
            what,
        }
    }
}

impl From<support_nom::ParseError> for Error {
    fn from(what: support_nom::ParseError) -> Error {
        Error::ParseError {
            core: ErrorCore::default(),
            what: what.string,
        }
    }
}

//////////////////////////////////////////////// IDs ///////////////////////////////////////////////

generate_id! {CollectorID, "collector:"}
generate_id! {MetricID, "metric:"}

//////////////////////////////////////////// MetricType ////////////////////////////////////////////

/// The type of metric being requested.  A switch over biometrics sensor types.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub enum MetricType {
    #[default]
    Counter,
    Gauge,
    Moments,
    Histogram,
}

/////////////////////////////////////////////// Time ///////////////////////////////////////////////

/// Time since UNIX epoch, in microseconds.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, prototk_derive::Message,
)]
pub struct Time(i64);

impl Time {
    pub const ONE_SECOND: Time = Time(1_000_000);

    /// Construct a new time from an integer number of seconds.
    pub fn from_secs(s: i64) -> Option<Self> {
        if let Some(micros) = s.checked_mul(1_000_000) {
            Self::from_micros(micros)
        } else {
            None
        }
    }

    /// Construct a new time from an integer number of microseconds.
    pub fn from_micros(micros: i64) -> Option<Self> {
        if DateTime::<Utc>::from_timestamp_micros(micros).is_some() {
            Some(Self(micros))
        } else {
            None
        }
    }

    /// Now.
    pub fn now() -> Option<Self> {
        Self::from_chrono(Utc::now())
    }

    /// Convert the time to the number of seconds it represents.
    pub fn to_secs(self) -> f64 {
        self.0 as f64 / 1_000_000.0
    }

    /// Construct a new time from an RFC3339-formatted date time.
    pub fn from_rfc3339(s: &str) -> Option<Self> {
        Some(DateTime::parse_from_rfc3339(s).ok()?.to_utc().into())
    }

    /// Construct a new RFC3339-formatted date time.
    pub fn to_rfc3339(self) -> String {
        self.to_chrono().to_rfc3339()
    }

    /// Convert the time to a chrono DateTime.
    pub fn to_chrono(&self) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp_micros(self.0).expect("time should always fit DateTime")
    }

    /// Construct a new time from a chrono DateTime.
    pub fn from_chrono(dt: DateTime<Utc>) -> Option<Self> {
        Self::from_micros(dt.timestamp_micros())
    }

    pub fn can_be_divided_by(&self, step: Time) -> bool {
        is_multiple_of(self.0, step.0)
    }

    pub fn divide_by(&self, step: Time) -> usize {
        assert!(self.can_be_divided_by(step));
        (*self / step).try_into().expect("steps should fit a usize")
    }

    fn delta(prev: &Self, point: &Self) -> Result<Self, Error> {
        if point.0 < 0 {
            return Err(Error::time(
                "visited time before the epoch; time travel's not allowed",
            ));
        }
        if let Some(delta) = point.0.checked_sub(prev.0) {
            if delta < 0 {
                return Err(Error::time(format!(
                    "went backwards in time from {} to {}",
                    prev.to_rfc3339(),
                    point.to_rfc3339(),
                )));
            }
            Ok(Time(delta))
        } else {
            Err(Error::arithmetic(format!(
                "subtraction underflowed: {point:?} - {prev:?}"
            )))
        }
    }

    fn delta_delta(delta1: &Self, prev: &Self, point: &Self) -> Result<Self, Error> {
        let delta2 = Self::delta(prev, point)?;
        if let Some(delta) = delta2.0.checked_sub(delta1.0) {
            Ok(Time(delta))
        } else {
            Err(Error::arithmetic(format!(
                "subtraction underflowed: {point:?} - {prev:?}"
            )))
        }
    }

    fn undelta(prev: &Self, delta: &Self) -> Result<Self, Error> {
        if let Some(time) = prev.0.checked_add(delta.0) {
            if Time(time) < *prev {
                return Err(Error::time(
                    "visits time before the epoch; time travel's not allowed",
                ));
            }
            Ok(Time(time))
        } else {
            Err(Error::arithmetic(format!(
                "addition overflowed: {prev:?} + {delta:?}"
            )))
        }
    }

    fn undelta_undelta(prev_prev: &Self, prev: &Self, delta: &Self) -> Result<Self, Error> {
        let Some(value) = delta.0.checked_add(prev.0) else {
            return Err(Error::arithmetic(format!(
                "addition overflowed: {delta:?} + {prev:?}"
            )));
        };
        let Some(value) = value.checked_add(prev.0) else {
            return Err(Error::arithmetic(format!(
                "addition overflowed: {value:?} + {prev:?}"
            )));
        };
        let Some(value) = value.checked_sub(prev_prev.0) else {
            return Err(Error::arithmetic(format!(
                "subtraction underflowed: {value:?} - {prev_prev:?}"
            )));
        };
        if Time(value) < *prev {
            return Err(Error::time(
                "visits time before the epoch; time travel's not allowed",
            ));
        }
        Ok(Time(value))
    }
}

impl From<DateTime<Utc>> for Time {
    fn from(dt: DateTime<Utc>) -> Self {
        Time(dt.timestamp_micros())
    }
}

impl From<Duration> for Time {
    fn from(d: Duration) -> Self {
        Time(d.as_micros() as i64)
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

impl Mul<i64> for Time {
    type Output = Time;

    fn mul(self, other: i64) -> Self {
        Self(self.0 * other)
    }
}

impl Mul<usize> for Time {
    type Output = Time;

    fn mul(self, other: usize) -> Self {
        Self(self.0 * other as i64)
    }
}

impl Div<Time> for Time {
    type Output = i64;

    fn div(self, other: Self) -> i64 {
        self.0 / other.0
    }
}

impl Div<i64> for Time {
    type Output = Time;

    fn div(self, other: i64) -> Self {
        Self(self.0 / other)
    }
}

impl Rem<Time> for Time {
    type Output = Time;

    fn rem(self, other: Self) -> Self {
        Self(self.0 % other.0)
    }
}

////////////////////////////////////////////// Window //////////////////////////////////////////////

/// A Window has a start and end time.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, prototk_derive::Message)]
pub struct Window {
    #[prototk(1, message)]
    start: Time,
    #[prototk(2, message)]
    limit: Time,
}

impl Window {
    pub fn new(start: Time, limit: Time) -> Option<Self> {
        if start < limit && (limit - start).0 as u64 <= usize::MAX as u64 {
            Some(Self { start, limit })
        } else {
            None
        }
    }

    pub fn from_now(step: Time) -> Option<Self> {
        let now = Time::now()?;
        if now >= step {
            Some(Self {
                start: now - step,
                limit: now,
            })
        } else {
            None
        }
    }

    pub fn can_be_divided_by(&self, step: Time) -> bool {
        is_multiple_of((self.limit - self.start).0, step.0)
    }

    pub fn divide_by(&self, step: Time) -> usize {
        assert!(self.can_be_divided_by(step));
        ((self.limit - self.start) / step.0)
            .0
            .try_into()
            .expect("steps should fit a usize")
    }

    pub fn round_to_seconds(&self) -> Self {
        let start = Time(self.start.0 - (self.start.0 % 1_000_000));
        let limit = Time(self.limit.0 + 1_000_000 - (self.limit.0 % 1_000_000));
        Self { start, limit }
    }

    pub fn start(&self) -> Time {
        self.start
    }

    pub fn limit(&self) -> Time {
        self.limit
    }
}

impl Default for Window {
    fn default() -> Self {
        Self {
            start: Time::from_secs(0).unwrap(),
            limit: Time::from_secs(3600).unwrap(),
        }
    }
}

/////////////////////////////////////////////// Point //////////////////////////////////////////////

#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct Point(pub f64);

impl Point {
    pub const NAN: Point = Point(f64::NAN);

    fn compare(lhs: &Self, rhs: &Self) -> Ordering {
        lhs.0.total_cmp(&rhs.0)
    }

    fn from_usize(x: usize) -> Self {
        Self(x as f64)
    }

    fn mean(points: &[Self]) -> Self {
        if points.is_empty() {
            Point(f64::NAN)
        } else {
            points.iter().copied().fold(Self::default(), Self::add) / Self::from_usize(points.len())
        }
    }

    fn delta(prev: &Self, point: &Self) -> Result<Self, Error> {
        Ok(*point - *prev)
    }

    fn delta_delta(delta1: &Self, prev: &Self, point: &Self) -> Result<Self, Error> {
        let delta2 = Self::delta(prev, point)?;
        Ok(delta2 - *delta1)
    }

    fn encode(value: &Self, encoded: &mut delta_array::DeltaEncoder) -> Result<(), Error> {
        // TODO(rescrv):  Actually encode something compact.
        let value = value.0.to_bits();
        encoded.push(value)
    }

    fn undelta(prev: &Self, delta: &Self) -> Result<Self, Error> {
        Ok(*prev + *delta)
    }

    fn undelta_undelta(prev_prev: &Self, prev: &Self, delta: &Self) -> Result<Self, Error> {
        let value = *delta + *prev;
        let value = value + *prev;
        Ok(value - *prev_prev)
    }

    fn decode(decoded: &mut delta_array::DeltaDecoder) -> Result<Self, Error> {
        let value = decoded
            .next()
            .ok_or_else(|| Error::coding("no next value"))??;
        Ok(Self(f64::from_bits(value)))
    }
}

impl std::ops::Add<Point> for Point {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Point(self.0 + other.0)
    }
}

impl std::ops::Sub<Point> for Point {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Point(self.0 - other.0)
    }
}

impl std::ops::Mul<Point> for Point {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        Point(self.0 * other.0)
    }
}

impl std::ops::Div<Point> for Point {
    type Output = Self;

    fn div(self, other: Self) -> Self {
        Point(self.0 / other.0)
    }
}

impl std::ops::AddAssign<Point> for Point {
    fn add_assign(&mut self, other: Self) {
        self.0 += other.0;
    }
}

impl std::ops::SubAssign<Point> for Point {
    fn sub_assign(&mut self, other: Self) {
        self.0 -= other.0;
    }
}

impl std::ops::MulAssign<Point> for Point {
    fn mul_assign(&mut self, other: Self) {
        self.0 *= other.0;
    }
}

impl std::ops::DivAssign<Point> for Point {
    fn div_assign(&mut self, other: Self) {
        self.0 /= other.0;
    }
}

impl std::fmt::Display for Point {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}", self.0)
    }
}

////////////////////////////////////////////// Series //////////////////////////////////////////////

/// A series is a tagged finite sample of readings taken at discrete time points.
#[derive(Clone, Debug)]
pub struct Series {
    label: Option<Tags<'static>>,
    start: Time,
    step: Time,
    points: Vec<Point>,
}

impl Series {
    /// True if this series has no points.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Return a new series that has every point value replaced with a constant.
    pub fn as_constant(&self, point: Point) -> Series {
        let label = self.label.clone();
        let start = self.start;
        let step = self.step;
        let points = self.points.iter().map(|_| point).collect();
        Series {
            label,
            start,
            step,
            points,
        }
    }

    /// Return the window corresponding to this series.
    pub fn window(&self) -> Window {
        assert!((self.points.len() as u64) < i64::MAX as u64);
        Window {
            start: self.start,
            limit: self.start + self.step * self.points.len() as i64,
        }
    }

    pub fn decode(
        label: Option<Tags<'static>>,
        window: Window,
        step: Time,
        encoded: &EncodedSeries,
    ) -> Result<Self, Error> {
        if !window.can_be_divided_by(step) {
            return Err(Error::NonMultipleParameter {
                core: ErrorCore::default(),
            });
        }
        let mut threshold = window.start;
        let mut points = vec![Point(encoded.initial)];
        for res in SeriesDecoder::from(encoded.bytes.as_ref()) {
            let (time, point) = res?;
            while threshold < time {
                points.push(points[points.len() - 1]);
                threshold = threshold + step;
            }
            let len = points.len() - 1;
            points[len] = point;
        }
        let start = window.start;
        Ok(Series {
            label,
            start,
            step,
            points,
        })
    }

    pub fn points(&self) -> &[Point] {
        &self.points
    }
}

impl std::fmt::Display for Series {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.debug_struct("Series")
            .field("label", &self.label)
            .field("start", &self.start.0)
            .field("step", &self.step.0)
            .finish()
    }
}

/////////////////////////////////////////// SeriesEncoder //////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct SeriesEncoder {
    encoded: delta_array::DeltaEncoder,
    prev_point_t: Option<Time>,
    prev_delta_t: Option<Time>,
    prev_point_p: Option<Point>,
    prev_delta_p: Option<Point>,
}

impl SeriesEncoder {
    pub fn bytes(&self) -> usize {
        self.encoded.as_ref().len()
    }

    pub fn push(&mut self, time: Time, point: Point) -> Result<(), Error> {
        let time = Self::double_delta(
            &mut self.prev_delta_t,
            &mut self.prev_point_t,
            time,
            Time::delta,
            Time::delta_delta,
        )?;
        let point = Self::double_delta(
            &mut self.prev_delta_p,
            &mut self.prev_point_p,
            point,
            Point::delta,
            Point::delta_delta,
        )?;
        self.encoded.push(time.0 as u64)?;
        Point::encode(&point, &mut self.encoded)
    }

    fn double_delta<T: Copy>(
        prev_delta: &mut Option<T>,
        prev_point: &mut Option<T>,
        current: T,
        delta: impl Fn(&T, &T) -> Result<T, Error>,
        delta_delta: impl Fn(&T, &T, &T) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let ret = if let Some(prev_point) = (*prev_point).as_ref() {
            let this_delta = delta(prev_point, &current)?;
            let value = if let Some(prev_delta) = (*prev_delta).as_ref() {
                delta_delta(prev_delta, prev_point, &current)?
            } else {
                this_delta
            };
            *prev_delta = Some(this_delta);
            value
        } else {
            current
        };
        *prev_point = Some(current);
        Ok(ret)
    }
}

impl AsRef<[u8]> for SeriesEncoder {
    fn as_ref(&self) -> &[u8] {
        self.encoded.as_ref()
    }
}

/////////////////////////////////////////// SeriesDecoder //////////////////////////////////////////

pub struct SeriesDecoder<'a> {
    decoded: delta_array::DeltaDecoder<'a>,
    resets: usize,
    prev_prev_t: Option<Time>,
    prev_t: Option<Time>,
    prev_prev_p: Option<Point>,
    prev_p: Option<Point>,
}

impl SeriesDecoder<'_> {
    fn double_undelta<T: Copy>(
        prev_prev: &mut Option<T>,
        prev: &mut Option<T>,
        current: T,
        undelta: impl Fn(&T, &T) -> Result<T, Error>,
        undelta_undelta: impl Fn(&T, &T, &T) -> Result<T, Error>,
    ) -> Result<T, Error> {
        if let Some(p) = (*prev).as_ref() {
            let current = if let Some(pp) = (*prev_prev).as_ref() {
                undelta_undelta(pp, p, &current)?
            } else {
                undelta(p, &current)?
            };
            *prev_prev = prev.take();
            *prev = Some(current);
            Ok(current)
        } else {
            *prev = Some(current);
            Ok(current)
        }
    }
}

impl<'a> From<&'a [u8]> for SeriesDecoder<'a> {
    fn from(buf: &'a [u8]) -> Self {
        let decoded = delta_array::DeltaDecoder::new(buf);
        let resets = 0;
        let prev_prev_t = None;
        let prev_t = None;
        let prev_prev_p = None;
        let prev_p = None;
        Self {
            decoded,
            resets,
            prev_prev_t,
            prev_t,
            prev_prev_p,
            prev_p,
        }
    }
}

impl Iterator for SeriesDecoder<'_> {
    type Item = Result<(Time, Point), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(time) = self.decoded.next() {
            if self.decoded.resets() > self.resets {
                self.resets = self.decoded.resets();
                self.prev_prev_t = None;
                self.prev_t = None;
                self.prev_prev_p = None;
                self.prev_p = None;
            }
            let time = match time {
                Ok(time) => Time(time as i64),
                Err(err) => {
                    self.decoded.drain();
                    return Some(Err(err));
                }
            };
            let point = match Point::decode(&mut self.decoded) {
                Ok(point) => point,
                Err(err) => {
                    self.decoded.drain();
                    return Some(Err(err));
                }
            };
            let time = match Self::double_undelta(
                &mut self.prev_prev_t,
                &mut self.prev_t,
                time,
                Time::undelta,
                Time::undelta_undelta,
            ) {
                Ok(time) => time,
                Err(err) => {
                    self.decoded.drain();
                    return Some(Err(err));
                }
            };
            let point = match Self::double_undelta(
                &mut self.prev_prev_p,
                &mut self.prev_p,
                point,
                Point::undelta,
                Point::undelta_undelta,
            ) {
                Ok(point) => point,
                Err(err) => {
                    self.decoded.drain();
                    return Some(Err(err));
                }
            };
            Some(Ok((time, point)))
        } else {
            None
        }
    }
}

/////////////////////////////////////////// EncodedSeries //////////////////////////////////////////

#[derive(Clone, Debug, Default, PartialEq, prototk_derive::Message)]
pub struct EncodedSeries {
    #[prototk(1, double)]
    initial: f64,
    #[prototk(2, bytes)]
    bytes: Vec<u8>,
}

impl EncodedSeries {
    pub fn new(initial: Point, bytes: Vec<u8>) -> Self {
        let initial = initial.0;
        Self { initial, bytes }
    }
}

/////////////////////////////////////// FetchCountersRequest ///////////////////////////////////////

#[derive(Clone, Debug, Default, prototk_derive::Message)]
pub struct FetchCountersRequest {
    #[prototk(1, string)]
    pub tags: String,
    #[prototk(2, message)]
    pub params: query::QueryParams,
}

/////////////////////////////////////// FetchCountersResponse //////////////////////////////////////

#[derive(Clone, Debug, Default, prototk_derive::Message)]
pub struct FetchCountersResponse {
    #[prototk(1, message)]
    pub serieses: Vec<EncodedSeries>,
}

//////////////////////////////////////// FetchGaugesRequest ////////////////////////////////////////

#[derive(Clone, Debug, Default, prototk_derive::Message)]
pub struct FetchGaugesRequest {
    #[prototk(1, string)]
    pub tags: String,
}

//////////////////////////////////////// FetchGaugesResponse ///////////////////////////////////////

#[derive(Clone, Debug, Default, prototk_derive::Message)]
pub struct FetchGaugesResponse {}

////////////////////////////////////// FetchHistogramsRequest //////////////////////////////////////

#[derive(Clone, Debug, Default, prototk_derive::Message)]
pub struct FetchHistogramsRequest {
    #[prototk(1, string)]
    tags: String,
}

////////////////////////////////////// FetchHistogramsResponse /////////////////////////////////////

#[derive(Clone, Debug, Default, prototk_derive::Message)]
pub struct FetchHistogramsResponse {}

////////////////////////////////////////// BiometricsStore /////////////////////////////////////////

rpc_pb::service! {
    name = BiometricsStore;
    server = BiometricsStoreServer;
    client = BiometricsStoreClient;
    error = Error;

    rpc fetch_counters(FetchCountersRequest) -> FetchCountersResponse;
    rpc fetch_gauges(FetchGaugesRequest) -> FetchGaugesResponse;
    rpc fetch_histograms(FetchHistogramsRequest) -> FetchHistogramsResponse;
}

//////////////////////////////////////////////// () ////////////////////////////////////////////////

impl BiometricsStore for () {
    fn fetch_counters(
        &self,
        _: &rpc_pb::Context,
        _: FetchCountersRequest,
    ) -> Result<FetchCountersResponse, Error> {
        Ok(FetchCountersResponse::default())
    }

    fn fetch_gauges(
        &self,
        _: &rpc_pb::Context,
        _: FetchGaugesRequest,
    ) -> Result<FetchGaugesResponse, Error> {
        Ok(FetchGaugesResponse::default())
    }

    fn fetch_histograms(
        &self,
        _: &rpc_pb::Context,
        _: FetchHistogramsRequest,
    ) -> Result<FetchHistogramsResponse, Error> {
        Ok(FetchHistogramsResponse::default())
    }
}

//////////////////////////////////////////// QueryEngine ///////////////////////////////////////////

pub struct QueryEngine<S: BiometricsStore> {
    biometrics: S,
}

impl<S: BiometricsStore> QueryEngine<S> {
    pub fn new(biometrics: S) -> Self {
        Self { biometrics }
    }

    pub fn query(
        &self,
        ctx: &rpc_pb::Context,
        query: &str,
        params: query::QueryParams,
    ) -> Result<Vec<Series>, Error> {
        let query = support_nom::parse_all(querylang::expr)(query)?;
        (*query)(ctx, &self.biometrics, &params)
    }
}

/////////////////////////////////////////////// misc ///////////////////////////////////////////////

fn is_multiple_of(range: i64, multiplier: i64) -> bool {
    multiplier > 0 && (range / multiplier) * multiplier == range
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_double_delta() {
        let mut prev_delta = None;
        let mut prev_point = None;
        assert_eq!(
            Time(1),
            SeriesEncoder::double_delta(
                &mut prev_delta,
                &mut prev_point,
                Time(1),
                Time::delta,
                Time::delta_delta
            )
            .unwrap()
        );
        assert_eq!(None, prev_delta);
        assert_eq!(Some(Time(1)), prev_point);
        assert_eq!(
            Time(1),
            SeriesEncoder::double_delta(
                &mut prev_delta,
                &mut prev_point,
                Time(2),
                Time::delta,
                Time::delta_delta
            )
            .unwrap()
        );
        assert_eq!(Some(Time(1)), prev_delta);
        assert_eq!(Some(Time(2)), prev_point);
        assert_eq!(
            Time(1),
            SeriesEncoder::double_delta(
                &mut prev_delta,
                &mut prev_point,
                Time(4),
                Time::delta,
                Time::delta_delta
            )
            .unwrap()
        );
        assert_eq!(Some(Time(2)), prev_delta);
        assert_eq!(Some(Time(4)), prev_point);
        assert_eq!(
            Time(1),
            SeriesEncoder::double_delta(
                &mut prev_delta,
                &mut prev_point,
                Time(7),
                Time::delta,
                Time::delta_delta
            )
            .unwrap()
        );
        assert_eq!(Some(Time(3)), prev_delta);
        assert_eq!(Some(Time(7)), prev_point);
        assert_eq!(
            Time(1),
            SeriesEncoder::double_delta(
                &mut prev_delta,
                &mut prev_point,
                Time(11),
                Time::delta,
                Time::delta_delta
            )
            .unwrap()
        );
        assert_eq!(Some(Time(11)), prev_point);
    }

    #[test]
    fn time_double_undelta() {
        let mut prev_prev = None;
        let mut prev = None;
        assert_eq!(
            Time(1),
            SeriesDecoder::double_undelta(
                &mut prev_prev,
                &mut prev,
                Time(1),
                Time::undelta,
                Time::undelta_undelta
            )
            .unwrap()
        );
        assert_eq!(
            Time(2),
            SeriesDecoder::double_undelta(
                &mut prev_prev,
                &mut prev,
                Time(1),
                Time::undelta,
                Time::undelta_undelta
            )
            .unwrap()
        );
        assert_eq!(
            Time(4),
            SeriesDecoder::double_undelta(
                &mut prev_prev,
                &mut prev,
                Time(1),
                Time::undelta,
                Time::undelta_undelta
            )
            .unwrap()
        );
        assert_eq!(
            Time(7),
            SeriesDecoder::double_undelta(
                &mut prev_prev,
                &mut prev,
                Time(1),
                Time::undelta,
                Time::undelta_undelta
            )
            .unwrap()
        );
        assert_eq!(
            Time(11),
            SeriesDecoder::double_undelta(
                &mut prev_prev,
                &mut prev,
                Time(1),
                Time::undelta,
                Time::undelta_undelta
            )
            .unwrap()
        );
    }

    proptest::prop_compose! {
        pub fn arb_delta()(bv in proptest::collection::vec((0..256, -256..256), 0..1024)) -> Vec<(i32, i32)> {
            bv
        }
    }

    proptest::proptest! {
        #[test]
        fn series_encoder(deltas in arb_delta(), start_time in 0i64..1000000, start_point in 0i64..1000000) {
            let mut expected = vec![];
            let mut encoder = SeriesEncoder::default();
            let mut time = Time(start_time);
            let mut point = start_point;
            for (t, p) in deltas.into_iter() {
                time = time + Time(t as i64);
                point += p as i64;
                encoder.push(time, Point(point as f64)).unwrap();
                expected.push((time, Point(point as f64)));
            }
            let decoder: SeriesDecoder = SeriesDecoder::from(encoder.as_ref());
            let returned: Vec<Result<(Time, Point), Error>> = decoder.into_iter().collect();
            assert_eq!(expected.len(), returned.len());
            for (idx, (e, r)) in std::iter::zip(expected.into_iter(), returned.into_iter()).enumerate() {
                let r = r.unwrap();
                assert_eq!(e, r, "idx = {idx}");
            }
        }
    }
}
