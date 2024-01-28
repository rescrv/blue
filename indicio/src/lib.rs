#![doc = include_str!("../README.md")]

use std::fmt::Display;
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use biometrics::Counter;
use tatl::{HeyListen, Stationary};

mod parser;

use parser::ParseError;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static EMITTER_FAILURE: Counter = Counter::new("indicio.emitter_failure");
static EMITTER_FAILURE_MONITOR: Stationary =
    Stationary::new("indicio.emitter_failure", &EMITTER_FAILURE);

/// Registers this crate's biometrics with the provided Collector.
pub fn register_biometrics(collector: biometrics::Collector) {
    collector.register_counter(&EMITTER_FAILURE);
}

/// Registers this crate's monitors with the provided HeyListen.
pub fn register_monitors(hey_listen: &mut HeyListen) {
    hey_listen.register_stationary(&EMITTER_FAILURE_MONITOR);
}

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Debug)]
pub enum Error {
    ParseError { display: String },
    InvalidNumberLiteral { as_str: String },
}

impl From<ParseError> for Error {
    fn from(err: ParseError) -> Self {
        Self::ParseError {
            display: err.to_string(),
        }
    }
}

////////////////////////////////////////////// Values //////////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, PartialEq, prototk_derive::Message)]
pub struct Values {
    #[prototk(1, message)]
    values: Vec<Value>,
}

impl Deref for Values {
    type Target = Vec<Value>;

    fn deref(&self) -> &Vec<Value> {
        &self.values
    }
}

impl From<Vec<Value>> for Values {
    fn from(values: Vec<Value>) -> Self {
        Self {
            values,
        }
    }
}

/////////////////////////////////////////// KeyValuePair ///////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, PartialEq, prototk_derive::Message)]
pub struct MapEntry {
    #[prototk(1, string)]
    key: String,
    #[prototk(2, message)]
    value: Value,
}

impl From<(String, Value)> for MapEntry {
    fn from(entry: (String, Value)) -> Self {
        Self {
            key: entry.0,
            value: entry.1,
        }
    }
}

//////////////////////////////////////////////// Map ///////////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, PartialEq, prototk_derive::Message)]
pub struct Map {
    #[prototk(1, message)]
    entries: Vec<MapEntry>,
}

impl Map {
    pub fn insert(&mut self, key: String, value: Value) {
        self.entries.push(MapEntry::from((key, value)));
    }
}

impl FromIterator<(String, Value)> for Map {
    fn from_iter<T: IntoIterator<Item = (String, Value)>>(entries: T) -> Self {
        Self {
            entries: entries.into_iter().map(MapEntry::from).collect(),
        }
    }
}

/////////////////////////////////////////////// Value //////////////////////////////////////////////

#[derive(Clone, Debug, prototk_derive::Message)]
pub enum Value {
    #[prototk(1, Bool)]
    Bool(bool),
    #[prototk(2, uint64)]
    U64(u64),
    #[prototk(3, sint64)]
    I64(i64),
    #[prototk(4, double)]
    F64(f64),
    #[prototk(5, string)]
    String(String),
    #[prototk(6, message)]
    Array(Values),
    #[prototk(7, message)]
    Object(Map),
}

impl Value {
    pub fn parse<S: AsRef<str>>(value: S) -> Result<Self, Error> {
        Ok(parser::parse_all(parser::value)(value.as_ref())?)
    }
}

impl Default for Value {
    fn default() -> Self {
        Self::Object(Map::default())
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            Value::Bool(b) => write!(f, "{}", if *b { "true" } else { "false" }),
            Value::U64(x) => write!(f, "{}", x),
            Value::I64(x) => write!(f, "{}", x),
            Value::F64(x) => write!(f, "{}", x),
            Value::String(s) => {
                pub fn escape(input: &str) -> String {
                    let mut out: Vec<char> = Vec::new();
                    for c in input.chars() {
                        if c == '\\' {
                            out.push('\\');
                            out.push('\\');
                        } else if c == '\n' {
                            out.push('\\');
                            out.push('n');
                        } else if c == '"' {
                            out.push('\\');
                            out.push('"');
                        } else {
                            out.push(c);
                        }
                    }
                    out.into_iter().collect()
                }
                write!(f, "\"{}\"", escape(s))
            }
            Value::Array(values) => {
                let values = values.iter().map(|x| x.to_string()).collect::<Vec<_>>();
                write!(f, "[{}]", values.join(", "))
            }
            Value::Object(values) => {
                let values = values
                    .entries
                    .iter()
                    .map(|entry| format!("{}: {}", entry.key, entry.value))
                    .collect::<Vec<_>>();
                write!(f, "{{{}}}", values.join(", "))
            }
        }
    }
}

impl From<i64> for Value {
    fn from(x: i64) -> Self {
        Self::I64(x)
    }
}

impl From<u64> for Value {
    fn from(x: u64) -> Self {
        Self::U64(x)
    }
}

impl From<f64> for Value {
    fn from(x: f64) -> Self {
        Self::F64(x)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Self::String(s.to_string())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl Eq for Value {}

impl PartialEq for Value {
    fn eq(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Bool(lhs), Value::Bool(rhs)) => lhs == rhs,
            (Value::U64(lhs), Value::U64(rhs)) => lhs == rhs,
            (Value::I64(lhs), Value::I64(rhs)) => lhs == rhs,
            (Value::F64(lhs), Value::F64(rhs)) => lhs.total_cmp(rhs).is_eq(),
            (Value::String(lhs), Value::String(rhs)) => lhs == rhs,
            (Value::Array(lhs), Value::Array(rhs)) => lhs == rhs,
            (Value::Object(lhs), Value::Object(rhs)) => lhs == rhs,
            _ => false,
        }
    }
}

//////////////////////////////////////////// value macro ///////////////////////////////////////////

/// Construct a value literal using something that looks like rust literals.
#[macro_export]
macro_rules! value {
    ($($value:tt)+) => {
        $crate::value_internal!($($value)+)
    };
}

#[macro_export]
macro_rules! value_internal {
    (@array [$($elems:expr,)*]) => {
        vec![$($elems,)*]
    };

    (@array [$($elems:expr),*]) => {
        vec![$($elems),*]
    };

    (@array [$($elems:expr,)*] true $($rem:tt)*) => {
        $crate::value_internal!(@array [$($elems,)* $crate::value_internal!(true)] $($rem)*)
    };

    (@array [$($elems:expr,)*] false $($rem:tt)*) => {
        $crate::value_internal!(@array [$($elems,)* $crate::value_internal!(false)] $($rem)*)
    };

    (@array [$($elems:expr,)*] [$($array:tt)*] $($rem:tt)*) => {
        $crate::value_internal!(@array [$($elems,)* $crate::value_internal!([$($array)*])] $($rem)*)
    };

    (@array [$($elems:expr,)*] {$($obj:tt)*} $($rem:tt)*) => {
        $crate::value_internal!(@array [$($elems,)* $crate::value_internal!({$($obj)*})] $($rem)*)
    };

    (@array [$($elems:expr,)*] $next:expr, $($rem:tt)*) => {
        $crate::value_internal!(@array [$($elems,)* $crate::value_internal!($next),] $($rem)*)
    };

    (@array [$($elems:expr),*] , $($rem:tt)*) => {
        $crate::value_internal!(@array [$($elems,)*] $($rem)*)
    };

    (@array [$($elems:expr,)*] $last:expr) => {
        $crate::value_internal!(@array [$($elems,)* $crate::value_internal!($last),])
    };

    (@object $obj:ident () ()) => {};

    (@object $obj:ident [$key:ident] ($value:expr) , $($rest:tt)*) => {
        let _ = $obj.insert(stringify!($key).to_string(), $value);
        $crate::value_internal!(@object $obj () ($($rest)*));
    };

    (@object $obj:ident [$key:ident] ($value:expr)) => {
        let _ = $obj.insert(stringify!($key).to_string(), $value);
    };

    (@object $obj:ident ($key:ident) (: true $($rest:tt)*)) => {
        $crate::value_internal!(@object $obj [$key] ($crate::value_internal!(true)) $($rest)*);
    };

    (@object $obj:ident ($key:ident) (: false $($rest:tt)*)) => {
        $crate::value_internal!(@object $obj [$key] ($crate::value_internal!(false)) $($rest)*);
    };

    (@object $obj:ident ($key:ident) (: [$($array:tt)*] $($rest:tt)*)) => {
        $crate::value_internal!(@object $obj [$key] ($crate::value_internal!([$($array)*])) $($rest)*);
    };

    (@object $obj:ident ($key:ident) (: {$($map:tt)*} $($rest:tt)*)) => {
        $crate::value_internal!(@object $obj [$key] ($crate::value_internal!({$($map)*})) $($rest)*);
    };

    (@object $obj:ident ($key:ident) (: $value:expr , $($rest:tt)*)) => {
        $crate::value_internal!(@object $obj [$key] ($crate::value_internal!($value)) , $($rest)*);
    };

    (@object $obj:ident ($key:ident) (: $value:expr)) => {
        $crate::value_internal!(@object $obj [$key] ($crate::value_internal!($value)));
    };

    (@object $obj:ident () ($key:ident $($rest:tt)*)) => {
        $crate::value_internal!(@object $obj ($key) ($($rest)*));
    };

    (true) => {
        $crate::Value::Bool(true)
    };

    (false) => {
        $crate::Value::Bool(false)
    };

    ([]) => {
        $crate::Value::Array(vec![].into())
    };

    ([ $($tt:tt)+ ]) => {
        $crate::Value::Array($crate::value_internal!(@array [] $($tt)+).into())
    };

    ({}) => {
        $crate::Value::Object($crate::Map::default())
    };

    ({ $($tt:tt)+ }) => {
        $crate::Value::Object({
            let mut values = $crate::Map::default();
            $crate::value_internal!(@object values () ($($tt)+));
            values
        })
    };

    ($val:expr) => {
        $crate::Value::from($val)
    };
}

////////////////////////////////////////////// Emitter /////////////////////////////////////////////

/// An emitter for indicio tha temits key-value pairs.
pub trait Emitter: Send {
    /// Emit the provided key-value pair at the specified file/line.
    fn emit(&mut self, file: &'static str, line: u32, value: Value);
    /// Flush the emitter with whatever semantics the emitter chooses.
    fn flush(&mut self) {}
}

///////////////////////////////////////////// Collector ////////////////////////////////////////////

/// A collector is meant to be a static singleton that conditionally logs.
pub struct Collector {
    should_log: AtomicBool,
    emitter: Mutex<Option<Box<dyn Emitter>>>,
}

impl Collector {
    /// Create a new collector.
    pub const fn new() -> Self {
        Self {
            should_log: AtomicBool::new(false),
            emitter: Mutex::new(None),
        }
    }

    /// True iff the collector is actively logging.
    pub fn is_logging(&self) -> bool {
        self.should_log.load(Ordering::Relaxed)
    }

    /// Emit the key-value pair via the collector if and only if it is logging and has an emitter
    /// configured.
    pub fn emit(&self, file: &'static str, line: u32, value: Value) {
        if self.is_logging() {
            let mut emitter = self.emitter.lock().unwrap();
            if let Some(emitter) = emitter.as_deref_mut() {
                emitter.emit(file, line, value);
            }
        }
    }

    /// Call flush on the underlying emitter, if one is registered.
    pub fn flush(&self) {
        let mut emitter = self.emitter.lock().unwrap();
        if let Some(emitter) = emitter.as_deref_mut() {
            emitter.flush();
        }
    }

    /// Register the emitter with the collector.
    pub fn register<E: Emitter + 'static>(&self, emitter: E) {
        let boxed: Box<dyn Emitter> = Box::new(emitter);
        let mut emitter = self.emitter.lock().unwrap();
        *emitter = Some(boxed);
        self.should_log.store(true, Ordering::Relaxed);
    }

    /// Unregister any emitter from the collector.
    pub fn deregister(&self) {
        let mut emitter = self.emitter.lock().unwrap();
        *emitter = None;
        self.should_log.store(false, Ordering::Relaxed);
    }
}

///////////////////////////////////////////// the macro ////////////////////////////////////////////

/// Emit the specified key-value pair if and only if the collector is logging.
///
/// This will be lazy, and only evaluate the key-value pair if the collector is logging.
#[macro_export]
macro_rules! clue {
    ($collector:ident, { $($value:tt)* }) => {
        if $collector.is_logging() {
            $collector.emit(file!(), line!(), $crate::value!({ $($value)* }));
        }
    };
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    #![allow(clippy::excessive_precision)]
    #![allow(clippy::approx_constant)]

    use super::*;

    #[test]
    fn value_bool() {
        assert_eq!(Value::Bool(true), value!(true));
        assert_eq!(Value::Bool(false), value!(false));
    }

    #[test]
    fn value_i64() {
        assert_eq!(Value::I64(i64::MIN), value!(-9223372036854775808i64));
        assert_eq!(Value::I64(i64::MAX), value!(9223372036854775807i64));
    }

    #[test]
    fn value_u64() {
        assert_eq!(Value::U64(u64::MAX), value!(18446744073709551615u64));
    }

    #[test]
    fn value_f64() {
        assert_eq!(
            Value::F64(std::f64::consts::PI),
            value!(3.14159265358979323846264338327950288_f64)
        );
    }

    #[test]
    fn value_string() {
        assert_eq!(Value::String("foo".to_string()), value!("foo"));
    }

    #[test]
    fn value_array() {
        assert_eq!(Value::Array(vec![].into()), value!([]));
        assert_eq!(
            Value::Array(vec![
                Value::Bool(false),
                Value::Bool(true),
                Value::Array(vec![
                    Value::String("hello".to_string()),
                    Value::String("world".to_string())
                ].into())
            ].into()),
            value!([false, true, ["hello", "world"]])
        );
    }

    #[test]
    fn value_object() {
        assert_eq!(Value::Object(vec![].into_iter().collect()), value!({}));
        assert_eq!(
            Value::Object(
                vec![
                    (
                        "hello".to_string(),
                        Value::String("world".to_string())
                    ),
                    (
                        "consts".to_string(),
                        Value::Array(vec![
                            Value::F64(2.718281828459045),
                            Value::F64(3.141592653589793)
                        ].into())
                    ),
                    (
                        "recursive".to_string(),
                        Value::Object(
                            vec![
                                (
                                    "hello".to_string(),
                                    Value::String("world".to_string())
                                ),
                                (
                                    "consts".to_string(),
                                    Value::Array(vec![
                                        Value::F64(2.718281828459045),
                                        Value::F64(3.141592653589793)
                                    ].into())
                                ),
                            ]
                            .into_iter()
                            .collect()
                        )
                    ),
                ]
                .into_iter()
                .collect()
            ),
            value!({
                hello: "world",
                consts: [
                    2.71828182845904523536028747135266250_f64,
                    3.14159265358979323846264338327950288_f64,
                ],
                recursive: {
                    hello: "world",
                    consts: [
                        2.71828182845904523536028747135266250_f64,
                        3.14159265358979323846264338327950288_f64,
                    ],
                }
            })
        );
    }
}
