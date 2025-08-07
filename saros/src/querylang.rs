use std::time::Duration;

use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{alpha1, alphanumeric1},
    combinator::{cut, map, map_res, recognize},
    error::context,
    multi::{many0_count, many1_count, separated_list1},
    number::complete::double,
    sequence::{pair, tuple},
};

use tag_index::Tags;

use crate::query;
use crate::support_nom::{ws0, ParseError, ParseResult};
use crate::{BiometricsStore, Error, Point, Series, Time};

////////////////////////////////////////////// parsers /////////////////////////////////////////////

pub fn atom(input: &str) -> ParseResult<String> {
    context(
        "atom",
        map(
            recognize(pair(
                alt((alpha1, tag("_"))),
                many1_count(alt((alphanumeric1, tag("."), tag(":"), tag("-"), tag("_")))),
            )),
            String::from,
        ),
    )(input)
}

pub fn metric_name(input: &str) -> ParseResult<String> {
    context(
        "metric name",
        map(
            recognize(pair(
                alt((alpha1, tag("_"), tag("."), tag("-"), tag(":"))),
                many0_count(alt((alphanumeric1, tag("."), tag(":"), tag("-"), tag("_")))),
            )),
            String::from,
        ),
    )(input)
}

pub fn duration(input: &str) -> ParseResult<Duration> {
    context(
        "duration",
        map(
            pair(double, alt((tag("s"), tag("m"), tag("h"), tag("d")))),
            |(value, unit)| match unit {
                "s" => Duration::from_secs_f64(value),
                "m" => Duration::from_secs_f64(value * 60.0),
                "h" => Duration::from_secs_f64(value * 60.0 * 60.0),
                "d" => Duration::from_secs_f64(value * 60.0 * 60.0 * 24.0),
                _ => unreachable!(),
            },
        ),
    )(input)
}

////////////////////////////////////////////// sensors /////////////////////////////////////////////

#[allow(clippy::type_complexity)]
pub fn counters(
    input: &str,
) -> ParseResult<
    impl Fn(&rpc_pb::Context, &dyn BiometricsStore, &query::QueryParams) -> Result<Vec<Series>, Error>,
> {
    context(
        "counters",
        map_res(
            tuple((
                tag("counters"),
                cut(ws0),
                cut(tag("(")),
                cut(ws0),
                cut(separated_list1(tuple((ws0, tag(","), ws0)), metric_name)),
                cut(ws0),
                cut(tag(")")),
                cut(ws0),
            )),
            |(_, _, _, _, counters, _, _, _)| {
                let mut tags = Vec::with_capacity(counters.len());
                for counter in counters.into_iter() {
                    if counter.starts_with(":") && counter.ends_with(":") {
                        tags.push(
                            Tags::new(counter)
                                .to_owned()
                                .ok_or(Error::text("tag did not parse"))?,
                        );
                    } else {
                        tags.push(
                            Tags::new(format!(":__name__={counter}:"))
                                .to_owned()
                                .ok_or(Error::text("tag did not construct"))?,
                        );
                    }
                }
                Ok::<_, Error>(
                    move |ctx: &rpc_pb::Context,
                          store: &dyn BiometricsStore,
                          params: &query::QueryParams|
                          -> Result<Vec<Series>, Error> {
                        let mut serieses = vec![];
                        for tags in tags.iter() {
                            serieses.extend(query::counters(tags)(ctx, store, params)?.into_iter());
                        }
                        Ok(serieses)
                    },
                )
            },
        ),
    )(input)
}

//////////////////////////////////////////// aggregates ////////////////////////////////////////////

#[allow(clippy::type_complexity)]
fn aggregate_helper(input: &str) -> ParseResult<Box<dyn Fn(&[Point]) -> Point>> {
    context(
        "aggregate function",
        alt((
            map(tag("maximum"), |_| Box::new(query::agg::maximum) as _),
            map(tag("mean"), |_| Box::new(query::agg::mean) as _),
            map(tag("median"), |_| Box::new(query::agg::median) as _),
            map(tag("minimum"), |_| Box::new(query::agg::minimum) as _),
            map(tag("stddev"), |_| Box::new(query::agg::stddev) as _),
            map(tag("stdvar"), |_| Box::new(query::agg::stdvar) as _),
        )),
    )(input)
}

#[allow(clippy::type_complexity)]
pub fn aggregate(
    input: &str,
) -> ParseResult<
    impl Fn(&rpc_pb::Context, &dyn BiometricsStore, &query::QueryParams) -> Result<Vec<Series>, Error>,
> {
    context(
        "aggregate",
        map(
            tuple((
                tag("aggregate"),
                cut(ws0),
                cut(tag("(")),
                cut(ws0),
                cut(aggregate_helper),
                cut(ws0),
                cut(tag(",")),
                cut(expr),
                cut(ws0),
                cut(tag(")")),
            )),
            |(_, _, _, _, func, _, _, e, _, _)| {
                move |ctx: &rpc_pb::Context,
                      store: &dyn BiometricsStore,
                      params: &query::QueryParams|
                      -> Result<Vec<Series>, Error> {
                    query::aggregate(|store, params| e(ctx, store, params), |p| func(p))(
                        store, params,
                    )
                }
            },
        ),
    )(input)
}

////////////////////////////////////////////// rollups /////////////////////////////////////////////

#[allow(clippy::type_complexity)]
fn rollup_helper(input: &str) -> ParseResult<Box<dyn Fn(&[Point]) -> Point>> {
    context(
        "rollup function",
        alt((
            map(tag("changes"), |_| Box::new(query::over_time::changes) as _),
            map(tag("maximum"), |_| Box::new(query::over_time::maximum) as _),
            map(tag("mean"), |_| Box::new(query::over_time::mean) as _),
            map(tag("median"), |_| Box::new(query::over_time::median) as _),
            map(tag("median_absolute_deviation"), |_| {
                Box::new(query::over_time::median_absolute_deviation) as _
            }),
            map(tag("minimum"), |_| Box::new(query::over_time::minimum) as _),
            map(tag("range"), |_| Box::new(query::over_time::range) as _),
            map(tag("stddev"), |_| Box::new(query::over_time::stddev) as _),
            map(tag("stdvar"), |_| Box::new(query::over_time::stdvar) as _),
        )),
    )(input)
}

#[allow(clippy::type_complexity)]
pub fn rollup(
    input: &str,
) -> ParseResult<
    impl Fn(&rpc_pb::Context, &dyn BiometricsStore, &query::QueryParams) -> Result<Vec<Series>, Error>,
> {
    context(
        "rollup",
        map(
            tuple((
                tag("rollup"),
                cut(ws0),
                cut(tag("(")),
                cut(ws0),
                cut(rollup_helper),
                cut(ws0),
                cut(cut(tag(","))),
                cut(ws0),
                cut(expr),
                cut(ws0),
                cut(tag(",")),
                cut(ws0),
                cut(duration),
                cut(ws0),
                cut(tag(")")),
            )),
            |(_, _, _, _, func, _, _, _, e, _, _, _, lookback, _, _)| {
                move |ctx: &rpc_pb::Context,
                      store: &dyn BiometricsStore,
                      params: &query::QueryParams|
                      -> Result<Vec<Series>, Error> {
                    query::rollup(
                        |store, params| e(ctx, store, params),
                        |p| func(p),
                        lookback.into(),
                    )(store, params)
                }
            },
        ),
    )(input)
}

///////////////////////////////////////////// pointwise ////////////////////////////////////////////

macro_rules! pointwise_helper {
    ($name:ident) => {
        map(
            tuple((
                tag(stringify!($name)),
                cut(ws0),
                cut(tag("(")),
                cut(ws0),
                cut(expr),
                cut(ws0),
                cut(tag(")")),
            )),
            |(_, _, _, _, e, _, _)| {
                Box::new(
                    move |ctx: &rpc_pb::Context,
                          store: &dyn BiometricsStore,
                          params: &query::QueryParams|
                          -> Result<Vec<Series>, Error> {
                        query::point::$name(|store, params| e(ctx, store, params))(store, params)
                    },
                ) as _
            },
        )
    };
}

#[allow(clippy::type_complexity)]
pub fn pointwise(
    input: &str,
) -> ParseResult<
    Box<
        dyn Fn(
            &rpc_pb::Context,
            &dyn BiometricsStore,
            &query::QueryParams,
        ) -> Result<Vec<Series>, Error>,
    >,
> {
    context(
        "pointwise function",
        alt((
            alt((
                pointwise_helper!(abs),
                pointwise_helper!(acos),
                pointwise_helper!(acosh),
                pointwise_helper!(asin),
                pointwise_helper!(asinh),
                pointwise_helper!(atan),
                pointwise_helper!(atanh),
                pointwise_helper!(cbrt),
                pointwise_helper!(ceil),
                pointwise_helper!(cos),
                pointwise_helper!(cosh),
                pointwise_helper!(exp),
                pointwise_helper!(exp2),
                pointwise_helper!(exp_m1),
                pointwise_helper!(floor),
                pointwise_helper!(fract),
                pointwise_helper!(ln),
                pointwise_helper!(ln_1p),
                pointwise_helper!(log10),
                pointwise_helper!(log2),
                pointwise_helper!(recip),
            )),
            alt((
                pointwise_helper!(round),
                pointwise_helper!(signum),
                pointwise_helper!(sin),
                pointwise_helper!(sinh),
                pointwise_helper!(sqrt),
                pointwise_helper!(tan),
                pointwise_helper!(tanh),
                pointwise_helper!(to_radians),
                pointwise_helper!(trunc),
                pointwise_helper!(sign),
            )),
        )),
    )(input)
}

#[allow(clippy::type_complexity)]
fn uniform_helper(input: &str) -> ParseResult<Box<dyn Fn(&[Point]) -> Point>> {
    context(
        "uniform function",
        alt((
            map(tag("first"), |_| Box::new(query::range::first) as _),
            map(tag("last"), |_| Box::new(query::range::last) as _),
            map(tag("maximum"), |_| Box::new(query::range::maximum) as _),
            map(tag("mean"), |_| Box::new(query::range::mean) as _),
            map(tag("median"), |_| Box::new(query::range::median) as _),
            map(tag("median_absolute_deviation"), |_| {
                Box::new(query::range::median_absolute_deviation) as _
            }),
            map(tag("minimum"), |_| Box::new(query::range::minimum) as _),
            map(tag("stddev"), |_| Box::new(query::range::stddev) as _),
            map(tag("stdvar"), |_| Box::new(query::range::stdvar) as _),
            map(tag("sum"), |_| Box::new(query::range::sum) as _),
        )),
    )(input)
}

#[allow(clippy::type_complexity)]
pub fn uniform(
    input: &str,
) -> ParseResult<
    impl Fn(&rpc_pb::Context, &dyn BiometricsStore, &query::QueryParams) -> Result<Vec<Series>, Error>,
> {
    context(
        "uniform",
        map(
            tuple((
                tag("uniform"),
                cut(ws0),
                tag("("),
                ws0,
                uniform_helper,
                ws0,
                tag(","),
                expr,
                tag(")"),
            )),
            |(_, _, _, _, func, _, _, e, _)| {
                move |ctx: &rpc_pb::Context,
                      store: &dyn BiometricsStore,
                      params: &query::QueryParams|
                      -> Result<Vec<Series>, Error> {
                    query::uniform(|store, params| e(ctx, store, params), |p| func(p))(
                        store, params,
                    )
                }
            },
        ),
    )(input)
}

/////////////////////////////////////////////// time ///////////////////////////////////////////////

#[allow(clippy::type_complexity)]
fn time_helper(input: &str) -> ParseResult<Box<dyn Fn(Time) -> Point>> {
    context(
        "time function",
        alt((
            map(tag("days_in_year"), |_| {
                Box::new(query::time::days_in_year) as _
            }),
            map(tag("days_in_month"), |_| {
                Box::new(query::time::days_in_month) as _
            }),
            map(tag("weekday"), |_| Box::new(query::time::weekday) as _),
            map(tag("year"), |_| Box::new(query::time::year) as _),
            map(tag("month"), |_| Box::new(query::time::month) as _),
            map(tag("day"), |_| Box::new(query::time::day) as _),
            map(tag("hour"), |_| Box::new(query::time::hour) as _),
            map(tag("minute"), |_| Box::new(query::time::minute) as _),
            map(tag("second"), |_| Box::new(query::time::second) as _),
            map(tag("unix_micros"), |_| {
                Box::new(query::time::unix_micros) as _
            }),
        )),
    )(input)
}

#[allow(clippy::type_complexity)]
pub fn time(
    input: &str,
) -> ParseResult<
    impl Fn(&rpc_pb::Context, &dyn BiometricsStore, &query::QueryParams) -> Result<Vec<Series>, Error>,
> {
    context(
        "uniform",
        map(
            tuple((
                tag("time"),
                cut(ws0),
                tag("("),
                ws0,
                time_helper,
                ws0,
                tag(")"),
            )),
            |(_, _, _, _, func, _, __)| {
                move |_: &rpc_pb::Context,
                      store: &dyn BiometricsStore,
                      params: &query::QueryParams|
                      -> Result<Vec<Series>, Error> {
                    query::function_of_time(&func)(store, params)
                }
            },
        ),
    )(input)
}

#[allow(clippy::type_complexity)]
pub fn expr(
    input: &str,
) -> ParseResult<
    Box<
        dyn Fn(
            &rpc_pb::Context,
            &dyn BiometricsStore,
            &query::QueryParams,
        ) -> Result<Vec<Series>, Error>,
    >,
> {
    context(
        "expression",
        alt((
            map(counters, |c| Box::new(c) as _),
            map(aggregate, |c| Box::new(c) as _),
            map(rollup, |c| Box::new(c) as _),
            map(uniform, |c| Box::new(c) as _),
            map(time, |c| Box::new(c) as _),
            pointwise,
        )),
    )(input)
}

#[allow(clippy::type_complexity)]
pub fn parse(
    input: &str,
) -> Result<
    Box<
        dyn Fn(
            &rpc_pb::Context,
            &dyn BiometricsStore,
            &query::QueryParams,
        ) -> Result<Vec<Series>, Error>,
    >,
    ParseError,
> {
    crate::support_nom::parse_all(expr)(input)
}
