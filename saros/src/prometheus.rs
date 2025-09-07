use std::collections::HashMap;

use nom::{
    branch::alt,
    bytes::complete::{tag, take_while},
    character::complete::{alphanumeric1, newline, none_of},
    combinator::{cut, map, opt, recognize},
    error::context,
    multi::{many0, many1_count, separated_list0},
    number::complete::double,
    sequence::{delimited, pair, terminated, tuple},
};

use crate::support_nom::{ewsunl, string_literal, ws0, ws1, ParseResult};

/////////////////////////////////////////// MetricReading //////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct MetricReading {
    pub metric_name: String,
    pub labels: HashMap<String, String>,
    pub reading: f64,
    pub timestamp: Option<f64>,
}

impl Eq for MetricReading {}

impl PartialEq for MetricReading {
    fn eq(&self, other: &Self) -> bool {
        let timestamp_eq = match (self.timestamp.as_ref(), other.timestamp.as_ref()) {
            (Some(lhs), Some(rhs)) => f64::total_cmp(lhs, rhs).is_eq(),
            (None, None) => true,
            _ => false,
        };
        self.metric_name == other.metric_name
            && self.labels == other.labels
            && f64::total_cmp(&self.reading, &other.reading).is_eq()
            && timestamp_eq
    }
}

///////////////////////////////////////////// HelpText /////////////////////////////////////////////

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HelpText(String);

//////////////////////////////////////////// SensorType ////////////////////////////////////////////

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SensorType {
    Counter,
    Gauge,
    Histogram,
    Unsupported,
}

////////////////////////////////////////// TypeDeclaration /////////////////////////////////////////

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeDeclaration(String, SensorType);

impl TypeDeclaration {
    pub fn label(&self) -> &str {
        &self.0
    }

    pub fn sensor_type(&self) -> SensorType {
        self.1
    }
}

////////////////////////////////////////// PrometheusLine //////////////////////////////////////////

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrometheusLine {
    MetricReading(MetricReading),
    HelpText(HelpText),
    TypeDeclaration(TypeDeclaration),
}

////////////////////////////////////////////// parsers /////////////////////////////////////////////

pub fn atom(input: &str) -> ParseResult<'_, String> {
    context(
        "metric name",
        map(
            recognize(many1_count(alt((
                alphanumeric1,
                tag("."),
                tag(":"),
                tag("-"),
                tag("_"),
            )))),
            String::from,
        ),
    )(input)
}

pub fn label(input: &str) -> ParseResult<'_, (String, String)> {
    context(
        "label",
        map(
            tuple((
                ws0,
                context("metric name", atom),
                ws0,
                context("equals", tag("=")),
                ws0,
                context("metric value", string_literal),
                ws0,
            )),
            |(_, key, _, _, _, value, _)| (key, value),
        ),
    )(input)
}

pub fn labels(input: &str) -> ParseResult<'_, HashMap<String, String>> {
    context(
        "label",
        map(
            delimited(
                context("opening brace", tag("{")),
                terminated(separated_list0(tag(","), label), opt(tag(","))),
                context("closing brace", tag("}")),
            ),
            HashMap::from_iter,
        ),
    )(input)
}

pub fn reading(input: &str) -> ParseResult<'_, f64> {
    context(
        "sensor reading",
        alt((
            map(tag("+Inf"), |_| f64::INFINITY),
            map(tag("-Inf"), |_| f64::NEG_INFINITY),
            map(tag("NaN"), |_| f64::NAN),
            double,
        )),
    )(input)
}

pub fn metric_reading(input: &str) -> ParseResult<'_, MetricReading> {
    context(
        "metric reading",
        map(
            tuple((
                ws0,
                atom,
                ws0,
                opt(labels),
                ws0,
                reading,
                opt(tuple((ws1, double))),
                ewsunl,
            )),
            |(_, metric_name, _, labels, _, reading, timestamp, _)| {
                let labels = labels.unwrap_or_default();
                let timestamp = timestamp.map(|x| x.1);
                MetricReading {
                    metric_name,
                    labels,
                    reading,
                    timestamp,
                }
            },
        ),
    )(input)
}

pub fn help_text(input: &str) -> ParseResult<'_, HelpText> {
    context(
        "help comment",
        map(
            tuple((
                ws0,
                tag("#"),
                ws0,
                tag("HELP"),
                ws1,
                recognize(pair(take_while(|c: char| c != '\n'), newline)),
            )),
            |(_, _, _, _, _, text): ((), &str, (), &str, (), &str)| {
                HelpText(text.trim().to_string())
            },
        ),
    )(input)
}

pub fn type_declaration(input: &str) -> ParseResult<'_, TypeDeclaration> {
    context(
        "type declaration",
        alt((
            map(
                tuple((
                    ws0,
                    tag("#"),
                    ws0,
                    tag("TYPE"),
                    ws1,
                    atom,
                    ws1,
                    tag("counter"),
                    cut(ewsunl),
                )),
                |(_, _, _, _, _, metric_name, _, _, _)| {
                    TypeDeclaration(metric_name, SensorType::Counter)
                },
            ),
            map(
                tuple((
                    ws0,
                    tag("#"),
                    ws0,
                    tag("TYPE"),
                    ws1,
                    atom,
                    ws1,
                    tag("gauge"),
                    cut(ewsunl),
                )),
                |(_, _, _, _, _, metric_name, _, _, _)| {
                    TypeDeclaration(metric_name, SensorType::Gauge)
                },
            ),
            map(
                tuple((
                    ws0,
                    tag("#"),
                    ws0,
                    tag("TYPE"),
                    ws1,
                    atom,
                    ws1,
                    tag("histogram"),
                    cut(ewsunl),
                )),
                |(_, _, _, _, _, metric_name, _, _, _)| {
                    TypeDeclaration(metric_name, SensorType::Histogram)
                },
            ),
        )),
    )(input)
}

pub fn comment(input: &str) -> ParseResult<'_, ()> {
    context(
        "comment",
        map(tuple((ws0, tag("#"), many0(none_of("\n")), ewsunl)), |_| ()),
    )(input)
}

pub fn parse(input: &str) -> ParseResult<'_, Vec<PrometheusLine>> {
    context(
        "prometheus metrics exposition",
        map(
            many0(alt((
                map(metric_reading, |r| Some(PrometheusLine::MetricReading(r))),
                map(help_text, |t| Some(PrometheusLine::HelpText(t))),
                map(type_declaration, |t| {
                    Some(PrometheusLine::TypeDeclaration(t))
                }),
                map(comment, |_| None),
            ))),
            |promlines| promlines.into_iter().flatten().collect(),
        ),
    )(input)
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod test {
    use nom::combinator::{all_consuming, complete, cut};

    use super::*;
    use crate::support_nom::{interpret_error_for_test, parse_all, ParseError};

    #[test]
    fn atoms() {
        assert_eq!("atom".to_string(), parse_all(atom)("atom").unwrap(),);
        assert_eq!(
            ParseError::from(
                r#"0: at line 1, in metric name:

^"#
                .to_string()
            ),
            interpret_error_for_test(cut(complete(all_consuming(atom))))("").unwrap_err()
        );
    }

    #[test]
    fn string_literal() {
        assert_eq!("FOO", parse_all(super::string_literal)(r#""FOO""#).unwrap());
        assert_eq!("\\", parse_all(super::string_literal)(r#""\\""#).unwrap());
        assert_eq!("\"", parse_all(super::string_literal)(r#""\"""#).unwrap());
        assert_eq!("\n", parse_all(super::string_literal)(r#""\n""#).unwrap());
        assert_eq!("n", parse_all(super::string_literal)(r#""n""#).unwrap());
    }

    #[test]
    fn label() {
        assert_eq!(
            ("foo".to_string(), "FOO".to_string()),
            parse_all(super::label)(r#"foo="FOO""#).unwrap()
        );
        assert_eq!(
            (
                "foo".to_string(),
                r#"\
""#
                .to_string()
            ),
            parse_all(super::label)(r#"foo="\\\n\"""#).unwrap()
        );
    }

    #[test]
    fn labels() {
        assert_eq!(HashMap::default(), parse_all(super::labels)("{}").unwrap(),);
        assert_eq!(
            HashMap::from_iter([("foo".to_string(), "FOO".to_string())]),
            parse_all(super::labels)(r#"{foo="FOO"}"#).unwrap(),
        );
        assert_eq!(
            HashMap::from_iter([("bar".to_string(), "\\\n\"".to_string())]),
            parse_all(super::labels)(r#"{bar="\\\n\""}"#).unwrap(),
        );
    }

    #[test]
    fn metric_reading() {
        assert_eq!(
            MetricReading {
                metric_name: "foo_metric".to_string(),
                labels: HashMap::from_iter([("foo".to_string(), "FOO".to_string())]),
                reading: 1e9,
                timestamp: None
            },
            parse_all(super::metric_reading)(
                r#"foo_metric{foo="FOO"} 1e9
"#
            )
            .unwrap(),
        );
        assert_eq!(
            MetricReading {
                metric_name: "foo_metric".to_string(),
                labels: HashMap::from_iter([("foo".to_string(), "FOO".to_string())]),
                reading: 1e9,
                timestamp: Some(12345.),
            },
            parse_all(super::metric_reading)(
                r#"foo_metric{foo="FOO"} 1e9 12345
"#
            )
            .unwrap(),
        );
        assert_eq!(
            MetricReading {
                metric_name: "something_weird".to_string(),
                labels: HashMap::from_iter([(
                    "problem".to_string(),
                    "division by zero".to_string()
                )]),
                reading: f64::INFINITY,
                timestamp: Some(-3982045.0),
            },
            parse_all(super::metric_reading)(
                r#"something_weird{problem="division by zero"} +Inf -3982045
"#
            )
            .unwrap(),
        );
    }

    #[test]
    fn help_text() {
        assert_eq!(
            HelpText("This is the help text.".to_string()),
            parse_all(super::help_text)(
                "# HELP This is the help text.
"
            )
            .unwrap(),
        );
    }

    #[test]
    fn parse_type() {
        assert_eq!(
            TypeDeclaration("metric_name".to_string(), SensorType::Counter),
            parse_all(super::type_declaration)(
                "# TYPE metric_name counter
"
            )
            .unwrap(),
        );
        assert_eq!(
            TypeDeclaration("metric_name".to_string(), SensorType::Gauge),
            parse_all(super::type_declaration)(
                "# TYPE metric_name gauge
"
            )
            .unwrap(),
        );
        assert_eq!(
            TypeDeclaration("metric_name".to_string(), SensorType::Histogram),
            parse_all(super::type_declaration)(
                "# TYPE metric_name histogram
"
            )
            .unwrap(),
        );
    }

    #[test]
    fn parse_prometheus() {
        const SAMPLE: &str = r#"# HELP http_requests_total The total number of HTTP requests.
# TYPE http_requests_total counter

# Escaping in label values:
msdos_file_access_time_seconds{path="C:\\DIR\\FILE.TXT",error="Cannot find file:\n\"FILE.TXT\""} 1.458255915e9

# Minimalistic line:
metric_without_timestamp_and_labels 12.47

# A weird metric from before the epoch:
something_weird{problem="division by zero"} +Inf -3982045

# A histogram, which has a pretty complex representation in the text format:
# HELP http_request_duration_seconds A histogram of the request duration.
# TYPE http_request_duration_seconds histogram
http_request_duration_seconds_bucket{le="0.05"} 24054
http_request_duration_seconds_bucket{le="0.1"} 33444
http_request_duration_seconds_bucket{le="0.2"} 100392
http_request_duration_seconds_bucket{le="0.5"} 129389
http_request_duration_seconds_bucket{le="1"} 133988
http_request_duration_seconds_bucket{le="+Inf"} 144320
http_request_duration_seconds_sum 53423
http_request_duration_seconds_count 144320

# TODO(rescrv):  Support quantiles?
# # Finally a summary, which has a complex representation, too:
# # HELP rpc_duration_seconds A summary of the RPC duration in seconds.
# # TYPE rpc_duration_seconds summary
# rpc_duration_seconds{quantile="0.01"} 3102
# rpc_duration_seconds{quantile="0.05"} 3272
# rpc_duration_seconds{quantile="0.5"} 4773
# rpc_duration_seconds{quantile="0.9"} 9001
# rpc_duration_seconds{quantile="0.99"} 76656
# rpc_duration_seconds_sum 1.7560473e+07
# rpc_duration_seconds_count 2693
"#;
        assert_eq!(
            vec![
                PrometheusLine::HelpText(HelpText(
                    "http_requests_total The total number of HTTP requests.".to_string(),
                )),
                PrometheusLine::TypeDeclaration(TypeDeclaration(
                    "http_requests_total".to_string(),
                    SensorType::Counter
                )),
                PrometheusLine::MetricReading(MetricReading {
                    metric_name: "msdos_file_access_time_seconds".to_string(),
                    labels: HashMap::from_iter([
                        ("path".to_string(), r#"C:\DIR\FILE.TXT"#.to_string()),
                        (
                            "error".to_string(),
                            r#"Cannot find file:
"FILE.TXT""#
                                .to_string(),
                        ),
                    ]),
                    reading: 1458255915.0,
                    timestamp: None
                }),
                PrometheusLine::MetricReading(MetricReading {
                    metric_name: "metric_without_timestamp_and_labels".to_string(),
                    labels: HashMap::default(),
                    reading: 12.47,
                    timestamp: None
                }),
                PrometheusLine::MetricReading(MetricReading {
                    metric_name: "something_weird".to_string(),
                    labels: HashMap::from_iter([(
                        "problem".to_string(),
                        "division by zero".to_string()
                    ),]),
                    reading: f64::INFINITY,
                    timestamp: Some(-3982045.0)
                }),
                PrometheusLine::HelpText(HelpText(
                    "http_request_duration_seconds A histogram of the request duration."
                        .to_string(),
                ),),
                PrometheusLine::TypeDeclaration(TypeDeclaration(
                    "http_request_duration_seconds".to_string(),
                    SensorType::Histogram,
                ),),
                PrometheusLine::MetricReading(MetricReading {
                    metric_name: "http_request_duration_seconds_bucket".to_string(),
                    labels: HashMap::from_iter([("le".to_string(), "0.05".to_string()),]),
                    reading: 24054.0,
                    timestamp: None,
                },),
                PrometheusLine::MetricReading(MetricReading {
                    metric_name: "http_request_duration_seconds_bucket".to_string(),
                    labels: HashMap::from_iter([("le".to_string(), "0.1".to_string()),]),
                    reading: 33444.0,
                    timestamp: None,
                },),
                PrometheusLine::MetricReading(MetricReading {
                    metric_name: "http_request_duration_seconds_bucket".to_string(),
                    labels: HashMap::from_iter([("le".to_string(), "0.2".to_string()),]),
                    reading: 100392.0,
                    timestamp: None,
                },),
                PrometheusLine::MetricReading(MetricReading {
                    metric_name: "http_request_duration_seconds_bucket".to_string(),
                    labels: HashMap::from_iter([("le".to_string(), "0.5".to_string()),]),
                    reading: 129389.0,
                    timestamp: None,
                },),
                PrometheusLine::MetricReading(MetricReading {
                    metric_name: "http_request_duration_seconds_bucket".to_string(),
                    labels: HashMap::from_iter([("le".to_string(), "1".to_string()),]),
                    reading: 133988.0,
                    timestamp: None,
                },),
                PrometheusLine::MetricReading(MetricReading {
                    metric_name: "http_request_duration_seconds_bucket".to_string(),
                    labels: HashMap::from_iter([("le".to_string(), "+Inf".to_string()),]),
                    reading: 144320.0,
                    timestamp: None,
                },),
                PrometheusLine::MetricReading(MetricReading {
                    metric_name: "http_request_duration_seconds_sum".to_string(),
                    labels: HashMap::default(),
                    reading: 53423.0,
                    timestamp: None,
                },),
                PrometheusLine::MetricReading(MetricReading {
                    metric_name: "http_request_duration_seconds_count".to_string(),
                    labels: HashMap::default(),
                    reading: 144320.0,
                    timestamp: None,
                },),
            ],
            parse_all(super::parse)(SAMPLE).unwrap(),
        );
    }
}
