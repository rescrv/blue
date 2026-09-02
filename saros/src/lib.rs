#![doc = include_str!("../README.md")]

use std::cmp::Ordering;
use std::fmt::Debug;
use std::ops::{Add, Div, Mul, Rem, Sub};
use std::time::Duration;

use buffertk::{Unpacker, stack_pack};
use chrono::{DateTime, Utc};

use biometrics::Counter;
pub use handled::SError;
use one_two_eight::generate_id;
use tag_index::Tags;
use tatl::{HeyListen, Stationary};

pub mod coding;
pub mod delta_array;
pub mod prometheus;
pub mod query;
pub mod querylang;
pub mod recovery;
pub mod store;
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

/////////////////////////////////////////////// Errors /////////////////////////////////////////////

const PHASE: &str = "saros";

/// A system error was encountered.
pub const CODE_SYSTEM_ERROR: &str = "system-error";
/// A query attempted lookback over lookback.
pub const CODE_NESTED_LOOKBACK: &str = "nested-lookback";
/// A query parameter does not obey the even-divisor rule.
pub const CODE_NON_MULTIPLE_PARAMETER: &str = "non-multiple-parameter";
/// A lookback exceeds the representable time range.
pub const CODE_LOOKBACK_TOO_LARGE: &str = "lookback-too-large";
/// Time conversion failed.
pub const CODE_TIME_ERROR: &str = "time-error";
/// Time series coding failed.
pub const CODE_CODING_ERROR: &str = "coding-error";
/// Arithmetic failed.
pub const CODE_ARITHMETIC_ERROR: &str = "arithmetic-error";
/// Text parsing or formatting failed.
pub const CODE_TEXT_ERROR: &str = "text-error";
/// Parsing failed.
pub const CODE_PARSE_ERROR: &str = "parse-error";
/// An internal invariant was violated.
pub const CODE_INTERNAL_ERROR: &str = "internal-error";

fn error(code: &str) -> SError {
    SError::new(PHASE).with_code(code)
}

pub fn arithmetic_error<S: AsRef<str>>(s: S) -> SError {
    error(CODE_ARITHMETIC_ERROR)
        .with_message("Saros arithmetic error")
        .with_string_field("what", s.as_ref())
}

pub fn coding_error<S: AsRef<str>>(s: S) -> SError {
    error(CODE_CODING_ERROR)
        .with_message("Saros coding error")
        .with_string_field("what", s.as_ref())
}

pub fn internal_error<S: AsRef<str>>(s: S) -> SError {
    error(CODE_INTERNAL_ERROR)
        .with_message("Saros internal error")
        .with_string_field("what", s.as_ref())
}

pub fn nested_lookback() -> SError {
    error(CODE_NESTED_LOOKBACK).with_message("nested lookback is not supported")
}

pub fn non_multiple_parameter() -> SError {
    error(CODE_NON_MULTIPLE_PARAMETER).with_message("parameter is not an even divisor")
}

pub fn lookback_too_large() -> SError {
    error(CODE_LOOKBACK_TOO_LARGE).with_message("lookback is too large")
}

pub fn system_error<S: AsRef<str>>(s: S) -> SError {
    error(CODE_SYSTEM_ERROR)
        .with_message("Saros system error")
        .with_string_field("what", s.as_ref())
}

pub fn text_error<S: AsRef<str>>(s: S) -> SError {
    error(CODE_TEXT_ERROR)
        .with_message("Saros text error")
        .with_string_field("what", s.as_ref())
}

pub fn time_error<S: AsRef<str>>(s: S) -> SError {
    error(CODE_TIME_ERROR)
        .with_message("Saros time error")
        .with_string_field("what", s.as_ref())
}

pub fn parse_error<S: AsRef<str>>(s: S) -> SError {
    error(CODE_PARSE_ERROR)
        .with_message("Saros parse error")
        .with_string_field("what", s.as_ref())
}

impl From<support_nom::ParseError> for SError {
    fn from(what: support_nom::ParseError) -> SError {
        parse_error(what.string)
    }
}

//////////////////////////////////////////////// IDs ///////////////////////////////////////////////

generate_id! {CollectorID, "collector:"}
generate_id! {MetricID, "metric:"}

//////////////////////////////////////////// MetricType ////////////////////////////////////////////

/// The type of metric being requested.  A switch over biometrics sensor types.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub enum MetricType {
    #[default]
    Counter,
    Gauge,
    Moments,
    Histogram,
}

impl MetricType {
    pub fn to_u8(self) -> u8 {
        match self {
            Self::Counter => 0,
            Self::Gauge => 1,
            Self::Moments => 2,
            Self::Histogram => 3,
        }
    }

    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Counter),
            1 => Some(Self::Gauge),
            2 => Some(Self::Moments),
            3 => Some(Self::Histogram),
            _ => None,
        }
    }

    pub fn to_u32(self) -> u32 {
        self.to_u8() as u32
    }

    pub fn from_u32(value: u32) -> Option<Self> {
        u8::try_from(value).ok().and_then(Self::from_u8)
    }
}

/////////////////////////////////////////////// Time ///////////////////////////////////////////////

/// Time since UNIX epoch, in microseconds.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, prototk_derive::Message,
)]
pub struct Time(#[prototk(1, sfixed64)] i64);

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

    /// Convert the time to the number of microseconds since the UNIX epoch.
    pub fn to_micros(self) -> i64 {
        self.0
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

    fn delta(prev: &Self, point: &Self) -> Result<Self, SError> {
        if point.0 < 0 {
            return Err(time_error(
                "visited time before the epoch; time travel's not allowed",
            ));
        }
        if let Some(delta) = point.0.checked_sub(prev.0) {
            if delta < 0 {
                return Err(time_error(format!(
                    "went backwards in time from {} to {}",
                    prev.to_rfc3339(),
                    point.to_rfc3339(),
                )));
            }
            Ok(Time(delta))
        } else {
            Err(arithmetic_error(format!(
                "subtraction underflowed: {point:?} - {prev:?}"
            )))
        }
    }

    fn delta_delta(delta1: &Self, prev: &Self, point: &Self) -> Result<Self, SError> {
        let delta2 = Self::delta(prev, point)?;
        if let Some(delta) = delta2.0.checked_sub(delta1.0) {
            Ok(Time(delta))
        } else {
            Err(arithmetic_error(format!(
                "subtraction underflowed: {point:?} - {prev:?}"
            )))
        }
    }

    fn undelta(prev: &Self, delta: &Self) -> Result<Self, SError> {
        if let Some(time) = prev.0.checked_add(delta.0) {
            if Time(time) < *prev {
                return Err(time_error(
                    "visits time before the epoch; time travel's not allowed",
                ));
            }
            Ok(Time(time))
        } else {
            Err(arithmetic_error(format!(
                "addition overflowed: {prev:?} + {delta:?}"
            )))
        }
    }

    fn undelta_undelta(prev_prev: &Self, prev: &Self, delta: &Self) -> Result<Self, SError> {
        let Some(value) = delta.0.checked_add(prev.0) else {
            return Err(arithmetic_error(format!(
                "addition overflowed: {delta:?} + {prev:?}"
            )));
        };
        let Some(value) = value.checked_add(prev.0) else {
            return Err(arithmetic_error(format!(
                "addition overflowed: {value:?} + {prev:?}"
            )));
        };
        let Some(value) = value.checked_sub(prev_prev.0) else {
            return Err(arithmetic_error(format!(
                "subtraction underflowed: {value:?} - {prev_prev:?}"
            )));
        };
        if Time(value) < *prev {
            return Err(time_error(
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

    fn delta(prev: &Self, point: &Self) -> Result<Self, SError> {
        Ok(*point - *prev)
    }

    fn delta_delta(delta1: &Self, prev: &Self, point: &Self) -> Result<Self, SError> {
        let delta2 = Self::delta(prev, point)?;
        Ok(delta2 - *delta1)
    }

    fn encode(value: &Self, encoded: &mut delta_array::DeltaEncoder) -> Result<(), SError> {
        // TODO(rescrv):  Actually encode something compact.
        let value = value.0.to_bits();
        encoded.push(value)
    }

    fn undelta(prev: &Self, delta: &Self) -> Result<Self, SError> {
        Ok(*prev + *delta)
    }

    fn undelta_undelta(prev_prev: &Self, prev: &Self, delta: &Self) -> Result<Self, SError> {
        let value = *delta + *prev;
        let value = value + *prev;
        Ok(value - *prev_prev)
    }

    fn decode(decoded: &mut delta_array::DeltaDecoder) -> Result<Self, SError> {
        let value = decoded
            .next()
            .ok_or_else(|| coding_error("no next value"))??;
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
    ) -> Result<Self, SError> {
        if !window.can_be_divided_by(step) {
            return Err(non_multiple_parameter());
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

    pub fn decode_chunks(
        label: Option<Tags<'static>>,
        window: Window,
        step: Time,
        chunks: &[SeriesChunk],
    ) -> Result<Option<Self>, SError> {
        if !window.can_be_divided_by(step) {
            return Err(non_multiple_parameter());
        }
        let mut samples = std::collections::BTreeMap::new();
        for chunk in chunks {
            for (time, point) in chunk.decode_samples()? {
                samples.insert(time, point);
            }
        }
        if samples.is_empty() {
            return Ok(None);
        }
        let mut points = Vec::with_capacity(window.divide_by(step));
        let mut sample_iter = samples.into_iter().peekable();
        let mut carry = None;
        for bucket in QueryStepIter::new(window, step) {
            while sample_iter.peek().is_some_and(|(time, _)| *time <= bucket) {
                let (_, point) = sample_iter.next().expect("peek said sample exists");
                carry = Some(point);
            }
            points.push(carry.unwrap_or(Point::NAN));
        }
        Ok(Some(Series {
            label,
            start: window.start,
            step,
            points,
        }))
    }

    pub fn points(&self) -> &[Point] {
        &self.points
    }
}

struct QueryStepIter {
    next: Time,
    limit: Time,
    step: Time,
}

impl QueryStepIter {
    fn new(window: Window, step: Time) -> Self {
        Self {
            next: window.start,
            limit: window.limit,
            step,
        }
    }
}

impl Iterator for QueryStepIter {
    type Item = Time;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next < self.limit {
            let next = self.next;
            self.next = self.next + self.step;
            Some(next)
        } else {
            None
        }
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

/////////////////////////////////////////// SeriesChunk ////////////////////////////////////////////

pub const SERIES_CHUNK_VERSION: u32 = 1;

/// Encodes one contiguous run of samples for a physical series.
///
/// The first sample is stored explicitly in the header.  Remaining timestamps
/// are encoded as one delta followed by delta-deltas, and remaining values are
/// encoded as Gorilla XORs over exact `f64` bit patterns.
#[derive(Clone, Debug, Default, PartialEq, prototk_derive::Message)]
pub struct SeriesChunk {
    /// Chunk format version.
    #[prototk(1, uint32)]
    pub version: u32,
    /// Numeric [`MetricType`] for the chunk.
    #[prototk(2, uint32)]
    pub metric_type: u32,
    /// Timestamp of the first sample in microseconds since the UNIX epoch.
    #[prototk(3, message)]
    pub first_sample_ts: Time,
    /// Timestamp of the last sample in microseconds since the UNIX epoch.
    #[prototk(4, message)]
    pub last_sample_ts: Time,
    /// Number of samples represented by the chunk.
    #[prototk(5, uint64)]
    pub sample_count: u64,
    /// Exact `f64::to_bits()` representation of the first sample value.
    #[prototk(6, fixed64)]
    pub first_value_bits: u64,
    /// Encoded timestamps after the first sample.
    #[prototk(7, bytes)]
    pub timestamp_stream: Vec<u8>,
    /// Gorilla XOR stream for values after the first sample.
    #[prototk(8, bytes)]
    pub value_stream: Vec<u8>,
}

impl SeriesChunk {
    /// Encode sorted samples into one chunk.
    ///
    /// # Errors
    ///
    /// Returns an error for empty input, unsorted timestamps, arithmetic
    /// overflow, or timestamp deltas that cannot be represented by the stream.
    pub fn from_samples(
        metric_type: MetricType,
        samples: &[(Time, Point)],
    ) -> Result<Self, SError> {
        if samples.is_empty() {
            return Err(coding_error("cannot encode empty chunk"));
        }
        let first_sample_ts = samples[0].0;
        let last_sample_ts = samples[samples.len() - 1].0;
        let sample_count =
            u64::try_from(samples.len()).map_err(|_| coding_error("sample count exceeds u64"))?;
        let first_value_bits = samples[0].1.0.to_bits();
        let mut timestamp_encoder = delta_array::DeltaEncoder::default();
        let mut value_encoder = coding::GorillaEncoder::new(first_value_bits);
        let mut prev_prev_ts: Option<Time> = None;
        let mut prev_ts = first_sample_ts;
        for (idx, (ts, point)) in samples.iter().copied().enumerate().skip(1) {
            let delta =
                ts.0.checked_sub(prev_ts.0)
                    .ok_or_else(|| arithmetic_error("timestamp delta underflow"))?;
            if delta < 0 {
                return Err(time_error("samples are not sorted by timestamp"));
            }
            if idx == 1 {
                timestamp_encoder.push(delta as u64)?;
            } else {
                let prev_prev_ts = prev_prev_ts.expect("idx >= 2 has prev_prev_ts");
                let prev_delta = prev_ts
                    .0
                    .checked_sub(prev_prev_ts.0)
                    .ok_or_else(|| arithmetic_error("timestamp delta underflow"))?;
                let dd = delta
                    .checked_sub(prev_delta)
                    .ok_or_else(|| arithmetic_error("timestamp delta-delta underflow"))?;
                timestamp_encoder.push(prototk::zigzag(dd))?;
            }
            value_encoder.push(point.0.to_bits());
            prev_prev_ts = Some(prev_ts);
            prev_ts = ts;
        }
        Ok(Self {
            version: SERIES_CHUNK_VERSION,
            metric_type: metric_type.to_u32(),
            first_sample_ts,
            last_sample_ts,
            sample_count,
            first_value_bits,
            timestamp_stream: timestamp_encoder.as_ref().to_vec(),
            value_stream: value_encoder.seal(),
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        stack_pack(self).to_vec()
    }

    /// Decode one serialized chunk message.
    ///
    /// # Errors
    ///
    /// Returns an error if the buffer is not exactly one protobuf-compatible
    /// `SeriesChunk` message.
    pub fn decode(buf: &[u8]) -> Result<Self, SError> {
        let mut unpacker = Unpacker::new(buf);
        let chunk: Self = unpacker
            .unpack()
            .map_err(|err: prototk::SError| coding_error(err.to_string()))?;
        if !unpacker.is_empty() {
            return Err(coding_error("trailing bytes after series chunk"));
        }
        Ok(chunk)
    }

    /// Decode this chunk into timestamped points.
    ///
    /// # Errors
    ///
    /// Returns an error if the version or metric type is unsupported, if a
    /// stream is truncated or malformed, or if decoded timestamps disagree with
    /// the header.
    pub fn decode_samples(&self) -> Result<Vec<(Time, Point)>, SError> {
        if self.version != SERIES_CHUNK_VERSION {
            return Err(coding_error(format!(
                "unsupported series chunk version: {}",
                self.version
            )));
        }
        if MetricType::from_u32(self.metric_type).is_none() {
            return Err(coding_error(format!(
                "unsupported metric type: {}",
                self.metric_type
            )));
        }
        if self.sample_count == 0 {
            return Err(coding_error("chunk has zero samples"));
        }
        let sample_count = usize::try_from(self.sample_count)
            .map_err(|_| coding_error("sample count exceeds usize"))?;
        let mut times = Vec::with_capacity(sample_count);
        times.push(self.first_sample_ts);
        let mut timestamp_decoder = delta_array::DeltaDecoder::new(&self.timestamp_stream);
        let mut prev_prev_ts = None;
        let mut prev_ts = self.first_sample_ts;
        for idx in 1..sample_count {
            let encoded = timestamp_decoder
                .next()
                .ok_or_else(|| coding_error("timestamp stream ended early"))??;
            let ts = if idx == 1 {
                Time(
                    prev_ts
                        .0
                        .checked_add(
                            i64::try_from(encoded)
                                .map_err(|_| coding_error("timestamp delta exceeds i64"))?,
                        )
                        .ok_or_else(|| arithmetic_error("timestamp delta overflow"))?,
                )
            } else {
                let dd = prototk::unzigzag(encoded);
                let prev_prev_ts = prev_prev_ts.expect("idx >= 2 has prev_prev_ts");
                Time::undelta_undelta(&prev_prev_ts, &prev_ts, &Time(dd))?
            };
            prev_prev_ts = Some(prev_ts);
            prev_ts = ts;
            times.push(ts);
        }
        if timestamp_decoder.next().is_some() {
            return Err(coding_error("timestamp stream has trailing data"));
        }
        if times.first() != Some(&self.first_sample_ts)
            || times.last() != Some(&self.last_sample_ts)
        {
            return Err(coding_error(format!(
                "chunk timestamp header mismatch: expected {:?}..{:?}, got {:?}..{:?}",
                self.first_sample_ts,
                self.last_sample_ts,
                times.first(),
                times.last()
            )));
        }
        let mut values = Vec::with_capacity(sample_count);
        values.push(Point(f64::from_bits(self.first_value_bits)));
        let mut value_decoder =
            coding::GorillaDecoder::new(self.first_value_bits, &self.value_stream);
        for _ in 1..sample_count {
            let bits = value_decoder
                .next()
                .ok_or_else(|| coding_error("value stream ended early"))?;
            values.push(Point(f64::from_bits(bits)));
        }
        Ok(std::iter::zip(times, values).collect())
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

    pub fn push(&mut self, time: Time, point: Point) -> Result<(), SError> {
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
        delta: impl Fn(&T, &T) -> Result<T, SError>,
        delta_delta: impl Fn(&T, &T, &T) -> Result<T, SError>,
    ) -> Result<T, SError> {
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
        undelta: impl Fn(&T, &T) -> Result<T, SError>,
        undelta_undelta: impl Fn(&T, &T, &T) -> Result<T, SError>,
    ) -> Result<T, SError> {
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
    type Item = Result<(Time, Point), SError>;

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

/// Groups fetched chunks with the canonical tags for their physical series.
#[derive(Clone, Debug, Default, prototk_derive::Message)]
pub struct FetchedSeries {
    /// Canonical tag string for this physical series.
    #[prototk(1, string)]
    pub tags: String,
    /// On-disk chunks that intersect the requested materialization range.
    #[prototk(2, message)]
    pub chunks: Vec<SeriesChunk>,
}

#[derive(Clone, Debug, Default, prototk_derive::Message)]
pub struct FetchCountersResponse {
    #[prototk(1, message)]
    pub serieses: Vec<FetchedSeries>,
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
    error = SError;

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
    ) -> Result<FetchCountersResponse, SError> {
        Ok(FetchCountersResponse::default())
    }

    fn fetch_gauges(
        &self,
        _: &rpc_pb::Context,
        _: FetchGaugesRequest,
    ) -> Result<FetchGaugesResponse, SError> {
        Ok(FetchGaugesResponse::default())
    }

    fn fetch_histograms(
        &self,
        _: &rpc_pb::Context,
        _: FetchHistogramsRequest,
    ) -> Result<FetchHistogramsResponse, SError> {
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
    ) -> Result<Vec<Series>, SError> {
        let query = querylang::parse(query)?;
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
            let returned: Vec<Result<(Time, Point), SError>> = decoder.into_iter().collect();
            assert_eq!(expected.len(), returned.len());
            for (idx, (e, r)) in std::iter::zip(expected.into_iter(), returned.into_iter()).enumerate() {
                let r = r.unwrap();
                assert_eq!(e, r, "idx = {idx}");
            }
        }
    }

    #[test]
    fn series_chunk_round_trips_samples() {
        let samples = vec![
            (Time::from_micros(1_000_000).unwrap(), Point(1.0)),
            (Time::from_micros(2_000_000).unwrap(), Point(1.0)),
            (Time::from_micros(4_000_000).unwrap(), Point(-0.0)),
            (
                Time::from_micros(7_000_000).unwrap(),
                Point(f64::from_bits(0x7ff8_1234_5678_9abc)),
            ),
        ];
        let chunk = SeriesChunk::from_samples(MetricType::Counter, &samples).unwrap();
        let encoded = chunk.encode();
        let chunk = SeriesChunk::decode(&encoded).unwrap();
        let decoded = chunk.decode_samples().unwrap();
        assert_eq!(samples.len(), decoded.len());
        for ((exp_time, exp_point), (got_time, got_point)) in samples.iter().zip(decoded.iter()) {
            assert_eq!(exp_time, got_time);
            assert_eq!(exp_point.0.to_bits(), got_point.0.to_bits());
        }
    }

    #[test]
    fn decode_chunks_carries_predecessor_through_window() {
        let samples = vec![
            (Time::from_secs(0).unwrap(), Point(5.0)),
            (Time::from_secs(10).unwrap(), Point(7.0)),
        ];
        let chunk = SeriesChunk::from_samples(MetricType::Counter, &samples).unwrap();
        let window =
            Window::new(Time::from_secs(20).unwrap(), Time::from_secs(50).unwrap()).unwrap();
        let step = Time::from_secs(10).unwrap();
        let series = Series::decode_chunks(None, window, step, &[chunk])
            .unwrap()
            .unwrap();
        assert_eq!(window.start(), series.start);
        assert_eq!(vec![Point(7.0), Point(7.0), Point(7.0)], series.points);
    }

    #[test]
    fn decode_chunks_uses_nan_until_first_sample() {
        let samples = vec![(Time::from_secs(30).unwrap(), Point(9.0))];
        let chunk = SeriesChunk::from_samples(MetricType::Counter, &samples).unwrap();
        let window =
            Window::new(Time::from_secs(20).unwrap(), Time::from_secs(50).unwrap()).unwrap();
        let step = Time::from_secs(10).unwrap();
        let series = Series::decode_chunks(None, window, step, &[chunk])
            .unwrap()
            .unwrap();
        assert!(series.points[0].0.is_nan());
        assert_eq!(Point(9.0), series.points[1]);
        assert_eq!(Point(9.0), series.points[2]);
    }

    #[test]
    fn decode_chunks_includes_sample_at_window_start() {
        let samples = vec![(Time::from_secs(20).unwrap(), Point(9.0))];
        let chunk = SeriesChunk::from_samples(MetricType::Counter, &samples).unwrap();
        let window =
            Window::new(Time::from_secs(20).unwrap(), Time::from_secs(40).unwrap()).unwrap();
        let step = Time::from_secs(10).unwrap();
        let series = Series::decode_chunks(None, window, step, &[chunk])
            .unwrap()
            .unwrap();
        assert_eq!(vec![Point(9.0), Point(9.0)], series.points);
    }

    #[test]
    fn decode_chunks_excludes_sample_at_window_limit() {
        let samples = vec![
            (Time::from_secs(20).unwrap(), Point(9.0)),
            (Time::from_secs(40).unwrap(), Point(99.0)),
        ];
        let chunk = SeriesChunk::from_samples(MetricType::Counter, &samples).unwrap();
        let window =
            Window::new(Time::from_secs(20).unwrap(), Time::from_secs(40).unwrap()).unwrap();
        let step = Time::from_secs(10).unwrap();
        let series = Series::decode_chunks(None, window, step, &[chunk])
            .unwrap()
            .unwrap();
        assert_eq!(vec![Point(9.0), Point(9.0)], series.points);
    }
}
