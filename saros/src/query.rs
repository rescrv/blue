use tag_index::Tags;
use zerror_core::ErrorCore;

use super::{BiometricsStore, Error, FetchCountersRequest, Point, Series, Time, Window};

//////////////////////////////////////////// QueryParams ///////////////////////////////////////////

/// QueryParams dictate the time window, display step, and optionally the lookback for a query.  An
/// assumption of the system is that there will be a single series that needs with lookback.  A
/// lookback over a lookback is both cumbersome to implement and as meaningless as an average of
/// averages.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash, prototk_derive::Message)]
pub struct QueryParams {
    #[prototk(1, message)]
    pub window: Window,
    #[prototk(2, message)]
    pub step: Time,
    #[prototk(3, message)]
    pub lookback: Option<Time>,
}

impl QueryParams {
    /// Create a new set of QueryParams.
    ///
    /// Returns None if the window and time step are not well-aligned.
    pub fn new(window: Window, step: Time) -> Option<QueryParams> {
        if step.0 == 0 || window.limit <= window.start || !window.can_be_divided_by(step) {
            None
        } else {
            let lookback = None;
            Some(Self {
                window,
                step,
                lookback,
            })
        }
    }

    /// Create a new set of QueryParams based-upon rendering constraints.  This will align to the
    /// limit and generate `buckets` steps going backwards in time.
    ///
    /// Returns None if the query parameters don't make sense.
    pub fn for_rendering(start: Time, limit: Time, buckets: usize) -> Option<QueryParams> {
        if limit < start || buckets as u64 > i64::MAX as u64 {
            None
        } else {
            let buckets = buckets as i64;
            let step = (limit - start) / buckets;
            let step = step - (step % Time::ONE_SECOND);
            let start = limit - step * buckets;
            Self::new(Window::new(start, limit)?, step)
        }
    }

    /// Return the time steps representing boundaries for these QueryParams.
    pub fn steps(&self) -> impl Iterator<Item = Time> {
        let steps = (self.window.limit - self.window.start) / self.step;
        let step = self.step;
        let start = self.window.start;
        (0..steps).map(move |idx| start + step * idx)
    }

    /// Create a QueryParams with a new time step that's an even divisor of the current time step.
    pub fn with_step(&self, step: Time) -> Result<QueryParams, Error> {
        if !self.step.can_be_divided_by(step) {
            Err(Error::NonMultipleParameter {
                core: ErrorCore::default(),
            })
        } else {
            let mut this = *self;
            this.step = step;
            Ok(this)
        }
    }

    /// Create a new QueryParams with lookback.
    pub fn with_lookback(&self, lookback: Time) -> Result<QueryParams, Error> {
        if self.lookback.is_some() {
            Err(Error::NestedLookback {
                core: ErrorCore::default(),
            })
        } else if !lookback.can_be_divided_by(self.step) {
            Err(Error::NonMultipleParameter {
                core: ErrorCore::default(),
            })
        } else if lookback > self.window.start {
            Err(Error::LookbackTooLarge {
                core: ErrorCore::default(),
            })
        } else {
            let window = self.window;
            let step = self.step;
            let lookback = Some(lookback);
            Ok(Self {
                window,
                step,
                lookback,
            })
        }
    }

    /// Returns the window of the query params, including lookback.
    pub fn window_including_lookback(&self) -> Window {
        if let Some(lookback) = self.lookback {
            Window {
                start: self.window.start - lookback,
                limit: self.window.limit,
            }
        } else {
            self.window
        }
    }

    /// Window accessor.
    pub fn window(&self) -> Window {
        self.window
    }

    /// step accessor.
    pub fn step(&self) -> Time {
        self.step
    }
}

////////////////////////////////////////////// leaves //////////////////////////////////////////////

pub fn counters<'a>(
    tags: &'a Tags<'a>,
) -> impl Fn(&rpc_pb::Context, &dyn BiometricsStore, &QueryParams) -> Result<Vec<Series>, Error> + 'a
{
    move |ctx, store, params| {
        let mut results = vec![];
        let resp = store.fetch_counters(
            ctx,
            FetchCountersRequest {
                params: *params,
                tags: tags.to_string(),
            },
        )?;
        for series in resp.serieses {
            let series = Series::decode(
                Some(tags.clone().into_owned()),
                params.window,
                params.step,
                &series,
            )?;
            results.push(series);
        }
        Ok(results)
    }
}

//////////////////////////////////////////// aggregates ////////////////////////////////////////////

/// Aggregate the series produced by query to create a new, single series.
///
/// This will use the merge function on a point-wise basis to create the new series's points.
pub fn aggregate(
    query: impl Fn(&dyn BiometricsStore, &QueryParams) -> Result<Vec<Series>, Error>,
    merge: impl Fn(&[Point]) -> Point,
) -> impl Fn(&dyn BiometricsStore, &QueryParams) -> Result<Vec<Series>, Error> {
    move |store, params| {
        let series = query(store, params)?;
        if series.is_empty() {
            return Ok(vec![]);
        }
        if !series.iter().all(|s| s.start == series[0].start) {
            return Err(Error::internal("series have irregular start"));
        }
        if !series
            .iter()
            .all(|s| s.points.len() == series[0].points.len())
        {
            return Err(Error::internal("series have irregular limit"));
        }
        if !series.iter().all(|s| s.step == series[0].step) {
            return Err(Error::internal("series have irregular step"));
        }
        let mut points = Vec::with_capacity(series[0].points.len());
        let mut scratch = Vec::with_capacity(series.len());
        for idx in 0..series[0].points.len() {
            scratch.clear();
            for series in series.iter() {
                if let Some(point) = series.points.get(idx) {
                    scratch.push(*point);
                }
            }
            points.push(merge(&scratch));
        }
        let label = None;
        let start = series[0].start;
        let step = series[0].step;
        Ok(vec![Series {
            label,
            start,
            step,
            points,
        }])
    }
}

pub mod agg {
    use biometrics::moments::Moments;

    use crate::Point;

    /// The maximum data point across all provided points.
    pub fn maximum(points: &[Point]) -> Point {
        // SAFETY(rescrv): We should never get an aggregate with zero points.
        points
            .iter()
            .copied()
            .max_by(Point::compare)
            .unwrap_or(Point::NAN)
    }

    /// The mean across all provided points.
    pub fn mean(points: &[Point]) -> Point {
        Point::mean(points)
    }

    /// The median across all provided data points.
    pub fn median(points: &[Point]) -> Point {
        let mut points = points.to_vec();
        points.sort_by(Point::compare);
        points[points.len() / 2]
    }

    // The minimum data point across all provided points.
    pub fn minimum(points: &[Point]) -> Point {
        // SAFETY(rescrv): We should never get an aggregate with zero points.
        points
            .iter()
            .copied()
            .min_by(Point::compare)
            .unwrap_or(Point::NAN)
    }

    pub fn stddev(points: &[Point]) -> Point {
        Point(stdvar(points).0.sqrt())
    }

    /// Convert the points to moments and return the variance of the points.
    pub fn stdvar(points: &[Point]) -> Point {
        let mut moments = Moments::new();
        points.iter().copied().for_each(|p| {
            moments.push(p.0);
        });
        Point(moments.variance())
    }
}

////////////////////////////////////////////// rollups /////////////////////////////////////////////

fn rollup_helper(
    params: &QueryParams,
    series: &Series,
    rollup: impl Fn(&[Point]) -> Point,
) -> Result<Series, Error> {
    let Some(lookback) = params.lookback.as_ref().copied() else {
        return Err(Error::internal("rollup_helper called without lookback"));
    };
    if series.start + lookback != params.window.start {
        return Err(Error::internal(
            "rollup_helper called without appropriate lookback",
        ));
    }
    if !lookback.can_be_divided_by(params.step) {
        return Err(Error::internal(
            "rollup_helper called with lookback that's not a multiple of step",
        ));
    }
    let steps_per_lookback = lookback.0 / params.step.0;
    let Ok(steps_per_lookback): Result<usize, _> = steps_per_lookback.try_into() else {
        return Err(Error::internal(
            "number of steps per lookback exceeds usize",
        ));
    };
    if steps_per_lookback == 0 {
        return Err(Error::internal("rollup_helper called with lookback of 0."));
    }
    let mut points = Vec::with_capacity(series.points.len());
    for window in series.points.windows(steps_per_lookback + 1) {
        points.push(rollup(window))
    }
    let label = None;
    let start = params.window.start;
    let step = series.step;
    Ok(Series {
        label,
        start,
        step,
        points,
    })
}

/// Rollup the provided query using a roll-up function and lookback time.  The rollup function will
/// be given a window of points corresponding to the lookback time.
pub fn rollup(
    query: impl Fn(&dyn BiometricsStore, &QueryParams) -> Result<Vec<Series>, Error>,
    rollup: impl Fn(&[Point]) -> Point,
    lookback: Time,
) -> impl Fn(&dyn BiometricsStore, &QueryParams) -> Result<Vec<Series>, Error> {
    move |store, params| {
        let params = params.with_lookback(lookback)?;
        let series = query(store, &params)?;
        let mut result = Vec::with_capacity(series.len());
        for series in series.into_iter() {
            result.push(rollup_helper(&params, &series, &rollup)?);
        }
        Ok(result)
    }
}

pub mod over_time {
    use biometrics::moments::Moments;

    use crate::Point;

    /// Returns 1.0 for buckets that change and 0.0 for buckets that don't change.
    pub fn changes(points: &[Point]) -> Point {
        points
            .iter()
            .zip(points[1..].iter())
            .map(|(p1, p2)| {
                if Point::compare(p1, p2).is_eq() {
                    Point(0.0)
                } else {
                    Point(1.0)
                }
            })
            .max_by(Point::compare)
            .unwrap_or(Point(0.0))
    }

    /// The maximum point in the lookback interval.
    pub fn maximum(points: &[Point]) -> Point {
        points
            .iter()
            .copied()
            .max_by(Point::compare)
            .unwrap_or(Point::NAN)
    }

    /// The mean across all points in the lookback interval.
    pub fn mean(points: &[Point]) -> Point {
        Point::mean(points)
    }

    /// The median across all points in the lookback interval.
    pub fn median(points: &[Point]) -> Point {
        super::median(points)
    }

    /// The median absolute deviation across all points in the lookback interval.
    pub fn median_absolute_deviation(points: &[Point]) -> Point {
        super::median_absolute_deviation(points)
    }

    /// The minimum across all points in the lookback interval.
    pub fn minimum(points: &[Point]) -> Point {
        points
            .iter()
            .copied()
            .min_by(Point::compare)
            .unwrap_or(Point::NAN)
    }

    /// The delta between the minimum and maximum values over the lookback interval.
    pub fn range(points: &[Point]) -> Point {
        let min = minimum(points);
        let max = maximum(points);
        max - min
    }

    /// The standard deviation of the points in the lookback interval.
    pub fn stddev(points: &[Point]) -> Point {
        Point(stdvar(points).0.sqrt())
    }

    /// The variance of the points in the lookback interval.
    pub fn stdvar(points: &[Point]) -> Point {
        let mut moments = Moments::new();
        points.iter().for_each(|p| {
            moments.push(p.0);
        });
        Point(moments.variance())
    }
}

///////////////////////////////////////////// pointwise ////////////////////////////////////////////

/// Calculate a point-wise transform for every point in the provided series set.
pub fn pointwise(
    query: impl Fn(&dyn BiometricsStore, &QueryParams) -> Result<Vec<Series>, Error>,
    transform: impl Fn(f64) -> f64,
) -> impl Fn(&dyn BiometricsStore, &QueryParams) -> Result<Vec<Series>, Error> {
    move |store, params| {
        let mut series = query(store, params)?;
        for series in series.iter_mut() {
            for point in series.points.iter_mut() {
                *point = Point(transform(point.0));
            }
        }
        Ok(series)
    }
}

pub mod point {
    use super::{pointwise, BiometricsStore, Error, QueryParams, Series};

    macro_rules! pointwise_f64 {
        ($name:ident) => {
            pub fn $name(
                query: impl Fn(&dyn BiometricsStore, &QueryParams) -> Result<Vec<Series>, Error>,
            ) -> impl Fn(&dyn BiometricsStore, &QueryParams) -> Result<Vec<Series>, Error> {
                pointwise(query, f64::$name)
            }
        };
    }

    pointwise_f64!(abs);
    pointwise_f64!(acos);
    pointwise_f64!(acosh);
    pointwise_f64!(asin);
    pointwise_f64!(asinh);
    pointwise_f64!(atan);
    pointwise_f64!(atanh);
    pointwise_f64!(cbrt);
    pointwise_f64!(ceil);
    pointwise_f64!(cos);
    pointwise_f64!(cosh);
    pointwise_f64!(exp);
    pointwise_f64!(exp2);
    pointwise_f64!(exp_m1);
    pointwise_f64!(floor);
    pointwise_f64!(fract);
    pointwise_f64!(ln);
    pointwise_f64!(ln_1p);
    pointwise_f64!(log10);
    pointwise_f64!(log2);
    pointwise_f64!(recip);
    pointwise_f64!(round);
    pointwise_f64!(signum);
    pointwise_f64!(sin);
    pointwise_f64!(sinh);
    pointwise_f64!(sqrt);
    pointwise_f64!(tan);
    pointwise_f64!(tanh);
    pointwise_f64!(to_radians);
    pointwise_f64!(trunc);

    pub fn sign(
        query: impl Fn(&dyn BiometricsStore, &QueryParams) -> Result<Vec<Series>, Error>,
    ) -> impl Fn(&dyn BiometricsStore, &QueryParams) -> Result<Vec<Series>, Error> {
        pointwise(query, |x| {
            if x > 0.0 {
                1.0
            } else if x < 0.0 {
                -1.0
            } else {
                0.0
            }
        })
    }
}

////////////////////////////////////////////// ranges //////////////////////////////////////////////

/// Convert the series in the query to a single, uniform value provided by the range function.
pub fn uniform(
    query: impl Fn(&dyn BiometricsStore, &QueryParams) -> Result<Vec<Series>, Error>,
    range: impl Fn(&[Point]) -> Point,
) -> impl Fn(&dyn BiometricsStore, &QueryParams) -> Result<Vec<Series>, Error> {
    move |store, params| {
        let series = query(store, params)?;
        let mut result = Vec::with_capacity(series.len());
        for series in series.into_iter() {
            if !series.points.is_empty() {
                let p = range(&series.points);
                result.push(series.as_constant(p).to_owned());
            } else {
                result.push(series.as_constant(Point::default()).to_owned());
            }
        }
        Ok(result)
    }
}

pub mod range {
    use biometrics::moments::Moments;

    use crate::Point;

    /// The first value in the series.
    pub fn first(points: &[Point]) -> Point {
        if points.is_empty() {
            Point::NAN
        } else {
            points[0]
        }
    }

    /// The last value in the series.
    pub fn last(points: &[Point]) -> Point {
        if points.is_empty() {
            Point::NAN
        } else {
            points[points.len() - 1]
        }
    }

    /// The maximum value of the series.
    pub fn maximum(points: &[Point]) -> Point {
        points
            .iter()
            .copied()
            .max_by(Point::compare)
            .unwrap_or(Point::NAN)
    }

    /// The mean value of the series.
    pub fn mean(points: &[Point]) -> Point {
        Point::mean(points)
    }

    /// The median value of the series.
    pub fn median(points: &[Point]) -> Point {
        super::median(points)
    }

    /// The median absolute deviation value of the series.
    pub fn median_absolute_deviation(points: &[Point]) -> Point {
        super::median_absolute_deviation(points)
    }

    /// The minimum value of the series.
    pub fn minimum(points: &[Point]) -> Point {
        points
            .iter()
            .copied()
            .min_by(Point::compare)
            .unwrap_or(Point::NAN)
    }

    /// The standard deviation of the series.
    pub fn stddev(points: &[Point]) -> Point {
        Point(stdvar(points).0.sqrt())
    }

    /// The variance of the series.
    pub fn stdvar(points: &[Point]) -> Point {
        let mut moments = Moments::new();
        points.iter().for_each(|p| {
            moments.push(p.0);
        });
        Point(moments.variance())
    }

    /// The sum of the series.
    pub fn sum(points: &[Point]) -> Point {
        points
            .iter()
            .copied()
            .fold(Point::default(), std::ops::Add::add)
    }
}

/////////////////////////////////////////////// time ///////////////////////////////////////////////

pub fn function_of_time(
    func: impl Fn(Time) -> Point,
) -> impl Fn(&dyn BiometricsStore, &QueryParams) -> Result<Vec<Series>, Error> {
    move |_, params| {
        let label = None;
        let start = params.window.start;
        let step = params.step;
        let points = params.steps().map(&func).collect();
        Ok(vec![Series {
            label,
            start,
            step,
            points,
        }])
    }
}

pub mod time {
    use chrono::{Datelike, Timelike};

    use crate::{Point, Time};

    /// The number of days in the year corresponding to each time step.
    pub fn days_in_year(t: Time) -> Point {
        if t.to_chrono().date_naive().leap_year() {
            Point(366.0)
        } else {
            Point(365.0)
        }
    }

    /// The number of days in the month corresponding to each time step.
    pub fn days_in_month(t: Time) -> Point {
        let t = t.to_chrono().date_naive();
        Point(match t.month() {
            1 => 31,
            2 => {
                if t.leap_year() {
                    29
                } else {
                    28
                }
            }
            3 => 31,
            4 => 30,
            5 => 31,
            6 => 30,
            7 => 31,
            8 => 31,
            9 => 30,
            10 => 31,
            11 => 30,
            12 => 31,
            _ => 0,
        } as f64)
    }

    /// The day of the week starting from Sunday = 1 corresponding to each time step.
    pub fn weekday(t: Time) -> Point {
        let t = t.to_chrono().date_naive();
        Point(t.weekday().number_from_sunday() as f64)
    }

    /// The year corresponding to each time step.
    pub fn year(t: Time) -> Point {
        let t = t.to_chrono().date_naive();
        Point(t.year() as f64)
    }

    /// The month corresponding to each time step.
    pub fn month(t: Time) -> Point {
        let t = t.to_chrono().date_naive();
        Point(t.month() as f64)
    }

    /// The day corresponding to each time step.
    pub fn day(t: Time) -> Point {
        let t = t.to_chrono().date_naive();
        Point(t.day() as f64)
    }

    /// The hour corresponding to each time step.
    pub fn hour(t: Time) -> Point {
        let t = t.to_chrono().time();
        Point(t.hour() as f64)
    }

    /// The minute corresponding to each time step.
    pub fn minute(t: Time) -> Point {
        let t = t.to_chrono().time();
        Point(t.minute() as f64)
    }

    /// The second corresponding to each time step.
    pub fn second(t: Time) -> Point {
        let t = t.to_chrono().time();
        Point(t.second() as f64)
    }

    /// The number of microseconds since UNIX epoch for each time step.
    pub fn unix_micros(t: Time) -> Point {
        Point(t.0 as f64)
    }
}

/////////////////////////////////////////////// misc ///////////////////////////////////////////////

/// Return the union of several queries of the same type.
#[allow(clippy::type_complexity)]
pub fn union(
    queries: Vec<Box<dyn Fn(&dyn BiometricsStore, &QueryParams) -> Result<Vec<Series>, Error>>>,
) -> impl Fn(&dyn BiometricsStore, &QueryParams) -> Result<Vec<Series>, Error> {
    move |store, params| {
        let mut result = vec![];
        for query in queries.iter() {
            result.append(&mut query(store, params)?);
        }
        Ok(result)
    }
}

/// Relabel the series.
pub fn relabel(
    query: impl Fn(&dyn BiometricsStore, &QueryParams) -> Result<Vec<Series>, Error>,
    label: Option<Tags<'static>>,
) -> impl Fn(&dyn BiometricsStore, &QueryParams) -> Result<Vec<Series>, Error> {
    move |store, params| {
        let mut series = query(store, params)?;
        for series in series.iter_mut() {
            series.label = label.clone();
        }
        Ok(series)
    }
}

fn median(points: &[Point]) -> Point {
    let mut points: Vec<Point> = points.to_vec();
    points.sort_by(Point::compare);
    if points.is_empty() {
        Point::NAN
    } else {
        points[points.len() / 2]
    }
}

fn median_absolute_deviation(points: &[Point]) -> Point {
    if points.is_empty() {
        return Point::NAN;
    }
    let m = median(points);
    let mut result = Vec::with_capacity(points.len());
    for point in points.iter().copied() {
        result.push(Point((point - m).0.abs()));
    }
    result.sort_by(Point::compare);
    result[points.len() / 2]
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Error, Series, Time};

    const PI: Point = Point(std::f64::consts::PI);
    const E: Point = Point(std::f64::consts::E);

    macro_rules! assert_approx_eq {
        ($lhs:expr, $rhs: expr) => {
            if !approx_eq($lhs, $rhs) {
                panic!(
                    "{}={} does not approximately equal {}={}",
                    stringify!($lhs),
                    $lhs,
                    stringify!($rhs),
                    $rhs
                );
            }
        };
    }

    macro_rules! series {
        ($start:expr, $step:expr) => {
            {
                let label = None;
                let start = Time::from_rfc3339($start).expect("start should be a valid RFC3339 DateTime");
                let step = Time::from_secs($step).expect("step should be valid number of seconds");
                let points = vec![];
                Series {
                    label,
                    start,
                    step,
                    points,
                }
            }
        };

        ($start:expr, $step:expr, $($points:expr),*) => {
            {
                let label = None;
                let start = Time::from_rfc3339($start).expect("start should be a valid RFC3339 DateTime");
                let step = Time::from_secs($step).expect("step should be valid number of seconds");
                let points = vec![$(Point($points)),*];
                Series {
                    label,
                    start,
                    step,
                    points,
                }
            }
        };
    }

    fn approx_eq(lhs: Point, rhs: Point) -> bool {
        (lhs.0 - rhs.0).abs() < 0.00001
    }

    fn render(series: Vec<Series>) -> String {
        if series.is_empty() {
            return "<empty>".to_string();
        }
        if series[0].points.is_empty() {
            return "<no_data>".to_string();
        }
        assert!(series
            .iter()
            .all(|s| s.points.len() == series[0].points.len()));
        let mut result = String::new();
        for step in 0..series[0].points.len() {
            let mut line = String::new();
            for series in series.iter() {
                if !line.is_empty() {
                    line += " ";
                }
                line += &series.points[step].to_string();
            }
            result += &line;
            result += "\n";
        }
        result
    }

    fn three_minute_query_params() -> QueryParams {
        let step = Time::from_secs(60).expect("60 seconds should always fit a time");
        let start =
            Time::from_rfc3339("2024-03-29T08:55:00Z").expect("start time should always parse");
        let limit =
            Time::from_rfc3339("2024-03-29T08:58:00Z").expect("limit time should always parse");
        QueryParams::new(Window { start, limit }, step)
            .expect("query params should always construct")
    }

    #[test]
    fn aggregation_maximum() {
        assert_approx_eq!(PI, agg::maximum(&[E, PI]));
    }

    #[test]
    fn aggregation_mean() {
        assert_approx_eq!(PI, agg::mean(&[PI, PI]));
        assert_approx_eq!(E, agg::mean(&[E, E]));
        assert_approx_eq!(Point(2.929937241024419), agg::mean(&[E, PI]));
    }

    #[test]
    fn aggregation_median() {
        assert_approx_eq!(PI, agg::median(&[PI, E, PI]));
        assert_approx_eq!(E, agg::median(&[E, PI, E]));
    }

    #[test]
    fn aggregation_minimum() {
        assert_approx_eq!(E, agg::minimum(&[E, PI]));
    }

    #[test]
    fn aggregation_stdvar() {
        assert_approx_eq!(Point(0.089596), agg::stdvar(&[E, PI]));
    }

    #[test]
    fn aggregation() {
        use std::f64::consts::{E, PI};
        let query = |_: &dyn BiometricsStore, params: &QueryParams| {
            let series1 = series! {"2024-03-29T08:55:00Z", 60, E, E, PI};
            let series2 = series! {"2024-03-29T08:55:00Z", 60, E, PI, PI};
            assert_eq!(params.window, series1.window());
            assert_eq!(params.window, series2.window());
            Ok::<_, Error>(vec![series1, series2])
        };
        let params = three_minute_query_params();
        assert_eq!(
            r#"
2.718281828459045
2.718281828459045
3.141592653589793
            "#
            .trim(),
            render(aggregate(query, agg::minimum)(&(), &params).expect("aggregate should succeed"))
                .trim(),
        );
        assert_eq!(
            r#"
2.718281828459045
3.141592653589793
3.141592653589793
            "#
            .trim(),
            render(aggregate(query, agg::maximum)(&(), &params).expect("aggregate should succeed"))
                .trim(),
        );
        assert_eq!(
            r#"
2.718281828459045
2.929937241024419
3.141592653589793
            "#
            .trim(),
            render(aggregate(query, agg::mean)(&(), &params).expect("aggregate should succeed"))
                .trim(),
        );
    }

    #[test]
    fn over_time_changes() {
        assert_approx_eq!(
            Point(0.0),
            over_time::changes(&[
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0)
            ])
        );
        assert_approx_eq!(
            Point(1.0),
            over_time::changes(&[
                Point(0.0),
                Point(1.0),
                Point(2.0),
                Point(3.0),
                Point(4.0),
                Point(5.0)
            ])
        );
        assert_approx_eq!(
            Point(1.0),
            over_time::changes(&[
                Point(0.0),
                Point(2.5),
                Point(2.5),
                Point(2.5),
                Point(2.5),
                Point(5.0)
            ])
        );
    }

    #[test]
    fn over_time_maximum() {
        assert_approx_eq!(
            Point(0.0),
            over_time::maximum(&[
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0)
            ])
        );
        assert_approx_eq!(
            Point(5.0),
            over_time::maximum(&[
                Point(0.0),
                Point(1.0),
                Point(2.0),
                Point(3.0),
                Point(4.0),
                Point(5.0)
            ])
        );
        assert_approx_eq!(
            Point(5.0),
            over_time::maximum(&[
                Point(0.0),
                Point(2.5),
                Point(2.5),
                Point(2.5),
                Point(2.5),
                Point(5.0)
            ])
        );
        assert_approx_eq!(
            Point(5.0),
            over_time::maximum(&[
                Point(5.0),
                Point(4.0),
                Point(3.0),
                Point(2.0),
                Point(1.0),
                Point(0.0)
            ])
        );
    }

    #[test]
    fn over_time_mean() {
        assert_approx_eq!(
            Point(0.0),
            over_time::mean(&[
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0)
            ])
        );
        assert_approx_eq!(
            Point(2.5),
            over_time::mean(&[
                Point(0.0),
                Point(1.0),
                Point(2.0),
                Point(3.0),
                Point(4.0),
                Point(5.0)
            ])
        );
        assert_approx_eq!(
            Point(2.5),
            over_time::mean(&[
                Point(0.0),
                Point(2.5),
                Point(2.5),
                Point(2.5),
                Point(2.5),
                Point(5.0)
            ])
        );
        assert_approx_eq!(
            Point(2.5),
            over_time::mean(&[
                Point(5.0),
                Point(4.0),
                Point(3.0),
                Point(2.0),
                Point(1.0),
                Point(0.0)
            ])
        );
    }

    #[test]
    fn over_time_median() {
        assert_approx_eq!(
            Point(0.0),
            over_time::median(&[
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0)
            ])
        );
        assert_approx_eq!(
            Point(3.0),
            over_time::median(&[
                Point(0.0),
                Point(1.0),
                Point(2.0),
                Point(3.0),
                Point(4.0),
                Point(5.0)
            ])
        );
        assert_approx_eq!(
            Point(2.5),
            over_time::median(&[
                Point(0.0),
                Point(2.5),
                Point(2.5),
                Point(2.5),
                Point(2.5),
                Point(5.0)
            ])
        );
        assert_approx_eq!(
            Point(3.0),
            over_time::median(&[
                Point(5.0),
                Point(4.0),
                Point(3.0),
                Point(2.0),
                Point(1.0),
                Point(0.0)
            ])
        );
    }

    #[test]
    fn over_time_minimum() {
        assert_approx_eq!(
            Point(0.0),
            over_time::minimum(&[
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0)
            ])
        );
        assert_approx_eq!(
            Point(0.0),
            over_time::minimum(&[
                Point(0.0),
                Point(1.0),
                Point(2.0),
                Point(3.0),
                Point(4.0),
                Point(5.0)
            ])
        );
        assert_approx_eq!(
            Point(0.0),
            over_time::minimum(&[
                Point(0.0),
                Point(2.5),
                Point(2.5),
                Point(2.5),
                Point(2.5),
                Point(5.0)
            ])
        );
        assert_approx_eq!(
            Point(0.0),
            over_time::minimum(&[
                Point(5.0),
                Point(4.0),
                Point(3.0),
                Point(2.0),
                Point(1.0),
                Point(0.0)
            ])
        );
    }

    #[test]
    fn over_time_range() {
        assert_approx_eq!(
            Point(0.0),
            over_time::range(&[
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0)
            ])
        );
        assert_approx_eq!(
            Point(5.0),
            over_time::range(&[
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(5.0)
            ])
        );
        assert_approx_eq!(
            Point(1.0),
            over_time::range(&[
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(1.0),
                Point(0.0),
                Point(0.0)
            ])
        );
        assert_approx_eq!(
            Point(2.0),
            over_time::range(&[
                Point(1.0),
                Point(1.0),
                Point(1.0),
                Point(3.0),
                Point(1.0),
                Point(2.0)
            ])
        );
    }

    #[test]
    fn rollups() {
        let query = |_: &dyn BiometricsStore, params: &QueryParams| {
            let mut series = series! {"2024-03-29T08:54:59Z", 1};
            series.points.push(Point(1.0));
            series.points.push(Point(1.0));
            for _ in 0..59 {
                let fib =
                    series.points[series.points.len() - 1] + series.points[series.points.len() - 2];
                series.points.push(fib);
            }
            assert_eq!(params.window_including_lookback(), series.window());
            Ok::<_, Error>(vec![series])
        };
        let step = Time::from_secs(1).expect("1 seconds should always fit a time");
        let start =
            Time::from_rfc3339("2024-03-29T08:55:00Z").expect("start time should always parse");
        let limit =
            Time::from_rfc3339("2024-03-29T08:56:00Z").expect("limit time should always parse");
        let params = QueryParams::new(Window { start, limit }, step)
            .expect("query params should always construct");
        assert_eq!(
            r#"
0
1
1
2
3
5
8
13
21
34
55
89
144
233
377
610
987
1597
2584
4181
6765
10946
17711
28657
46368
75025
121393
196418
317811
514229
832040
1346269
2178309
3524578
5702887
9227465
14930352
24157817
39088169
63245986
102334155
165580141
267914296
433494437
701408733
1134903170
1836311903
2971215073
4807526976
7778742049
12586269025
20365011074
32951280099
53316291173
86267571272
139583862445
225851433717
365435296162
591286729879
956722026041
            "#
            .trim(),
            render(
                rollup(query, |x| x[1] - x[0], step)(&(), &params).expect("rollup should succeed")
            )
            .trim(),
        );
    }

    #[test]
    fn pointwises() {
        let query = |_: &dyn BiometricsStore, params: &QueryParams| {
            let series = series! {"2024-03-29T08:55:00Z", 20, -1.0, 2.0, -3.0, 4.0, -5.0, 6.0, -7.0, 8.0, -9.0};
            assert_eq!(params.window, series.window());
            Ok::<_, Error>(vec![series])
        };
        let params = three_minute_query_params();
        assert_eq!(
            r#"
1
2
3
4
5
6
7
8
9
            "#
            .trim(),
            render(point::abs(query)(&(), &params).expect("pointwise should succeed")).trim(),
        );
    }

    #[test]
    fn range_first() {
        assert_approx_eq!(
            Point(0.0),
            range::first(&[
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0)
            ])
        );
        assert_approx_eq!(
            Point(1.0),
            range::first(&[
                Point(1.0),
                Point(2.0),
                Point(3.0),
                Point(4.0),
                Point(5.0),
                Point(6.0)
            ])
        );
    }

    #[test]
    fn range_last() {
        assert_approx_eq!(
            Point(0.0),
            range::last(&[
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0)
            ])
        );
        assert_approx_eq!(
            Point(6.0),
            range::last(&[
                Point(1.0),
                Point(2.0),
                Point(3.0),
                Point(4.0),
                Point(5.0),
                Point(6.0)
            ])
        );
    }

    #[test]
    fn range_maximum() {
        assert_approx_eq!(
            Point(0.0),
            range::maximum(&[
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0)
            ])
        );
        assert_approx_eq!(
            Point(6.0),
            range::maximum(&[
                Point(1.0),
                Point(2.0),
                Point(6.0),
                Point(4.0),
                Point(5.0),
                Point(3.0)
            ])
        );
    }

    #[test]
    fn range_mean() {
        assert_approx_eq!(
            Point(0.0),
            range::mean(&[
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0)
            ])
        );
        assert_approx_eq!(
            Point(3.5),
            range::mean(&[
                Point(1.0),
                Point(2.0),
                Point(6.0),
                Point(4.0),
                Point(5.0),
                Point(3.0)
            ])
        );
    }

    #[test]
    fn range_median() {
        assert_approx_eq!(
            Point(0.0),
            range::median(&[
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0)
            ])
        );
        assert_approx_eq!(
            Point(4.0),
            range::median(&[
                Point(1.0),
                Point(2.0),
                Point(6.0),
                Point(4.0),
                Point(5.0),
                Point(3.0)
            ])
        );
    }

    #[test]
    fn range_minimum() {
        assert_approx_eq!(
            Point(0.0),
            range::minimum(&[
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0)
            ])
        );
        assert_approx_eq!(
            Point(1.0),
            range::minimum(&[
                Point(1.0),
                Point(2.0),
                Point(6.0),
                Point(4.0),
                Point(5.0),
                Point(3.0)
            ])
        );
    }

    #[test]
    fn range_sum() {
        assert_approx_eq!(
            Point(0.0),
            range::sum(&[
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0),
                Point(0.0)
            ])
        );
        assert_approx_eq!(
            Point(21.0),
            range::sum(&[
                Point(1.0),
                Point(2.0),
                Point(6.0),
                Point(4.0),
                Point(5.0),
                Point(3.0)
            ])
        );
    }

    #[test]
    fn uniforms() {
        let query = |_: &dyn BiometricsStore, params: &QueryParams| {
            let series =
                series! {"2024-03-29T08:55:00Z", 20, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0};
            assert_eq!(params.window, series.window());
            Ok::<_, Error>(vec![series])
        };
        let params = three_minute_query_params();
        assert_eq!(
            r#"
45
45
45
45
45
45
45
45
45
            "#
            .trim(),
            render(uniform(query, range::sum)(&(), &params).expect("aggregate should succeed"))
                .trim(),
        );
    }
}
