#![doc = include_str!("../README.md")]

use std::fmt::Display;
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use biometrics::Counter;
use tatl::{HeyListen, Stationary};

pub mod stdio;

#[cfg(feature = "prototk")]
pub mod protobuf;

///////////////////////////////////////////// constants ////////////////////////////////////////////

pub const ALWAYS: u64 = 0;
pub const ERROR: u64 = 3;
pub const WARNING: u64 = 6;
pub const INFO: u64 = 9;
pub const DEBUG: u64 = 12;
pub const TRACING: u64 = 15;

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

////////////////////////////////////////////// Values //////////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[cfg_attr(feature = "prototk", derive(prototk_derive::Message))]
pub struct Values {
    #[cfg_attr(feature = "prototk", prototk(1, message))]
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
        Self { values }
    }
}

///////////////////////////////////////////// MapEntry /////////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[cfg_attr(feature = "prototk", derive(prototk_derive::Message))]
pub struct MapEntry {
    #[cfg_attr(feature = "prototk", prototk(1, string))]
    key: String,
    #[cfg_attr(feature = "prototk", prototk(2, message))]
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[cfg_attr(feature = "prototk", derive(prototk_derive::Message))]
pub struct Map {
    #[cfg_attr(feature = "prototk", prototk(1, message))]
    entries: Vec<MapEntry>,
}

impl Map {
    pub fn insert(&mut self, key: String, value: Value) {
        self.entries.push(MapEntry::from((key, value)));
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> + '_ {
        self.entries.iter().map(|e| (e.key.as_str(), &e.value))
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

#[derive(Clone, Debug)]
#[cfg_attr(feature = "prototk", derive(prototk_derive::Message))]
pub enum Value {
    #[cfg_attr(feature = "prototk", prototk(1, Bool))]
    Bool(bool),
    #[cfg_attr(feature = "prototk", prototk(2, uint64))]
    U64(u64),
    #[cfg_attr(feature = "prototk", prototk(3, sint64))]
    I64(i64),
    #[cfg_attr(feature = "prototk", prototk(4, double))]
    F64(f64),
    #[cfg_attr(feature = "prototk", prototk(5, string))]
    String(String),
    #[cfg_attr(feature = "prototk", prototk(6, message))]
    Array(Values),
    #[cfg_attr(feature = "prototk", prototk(7, message))]
    Object(Map),
}

impl Value {
    pub fn lookup(&self, key: &str) -> Option<&Value> {
        if let Value::Object(map) = self {
            map.entries
                .iter()
                .filter(|e| e.key == key)
                .take(1)
                .map(|e| &e.value)
                .next()
        } else {
            None
        }
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
                    .map(|entry| format!("\"{}\": {}", entry.key, entry.value))
                    .collect::<Vec<_>>();
                write!(f, "{{{}}}", values.join(", "))
            }
        }
    }
}

impl From<char> for Value {
    fn from(x: char) -> Self {
        let mut s = String::new();
        s.push(x);
        Self::String(s)
    }
}

impl From<bool> for Value {
    fn from(x: bool) -> Self {
        Self::Bool(x)
    }
}

impl TryFrom<&Value> for bool {
    type Error = ();

    fn try_from(value: &Value) -> Result<Self, ()> {
        if let Value::Bool(x) = value {
            Ok(*x)
        } else {
            Err(())
        }
    }
}

impl From<i32> for Value {
    fn from(x: i32) -> Self {
        Self::I64(x as i64)
    }
}

impl From<u32> for Value {
    fn from(x: u32) -> Self {
        Self::I64(x as i64)
    }
}

impl From<i64> for Value {
    fn from(x: i64) -> Self {
        Self::I64(x)
    }
}

impl TryFrom<&Value> for i64 {
    type Error = ();

    fn try_from(value: &Value) -> Result<Self, ()> {
        if let Value::I64(x) = value {
            Ok(*x)
        } else {
            Err(())
        }
    }
}

impl From<u64> for Value {
    fn from(x: u64) -> Self {
        Self::U64(x)
    }
}

impl TryFrom<&Value> for u64 {
    type Error = ();

    fn try_from(value: &Value) -> Result<Self, ()> {
        if let Value::U64(x) = value {
            Ok(*x)
        } else {
            Err(())
        }
    }
}

impl From<usize> for Value {
    fn from(x: usize) -> Self {
        Self::U64(x as u64)
    }
}

impl From<f64> for Value {
    fn from(x: f64) -> Self {
        Self::F64(x)
    }
}

impl TryFrom<&Value> for f64 {
    type Error = ();

    fn try_from(value: &Value) -> Result<Self, ()> {
        if let Value::F64(x) = value {
            Ok(*x)
        } else {
            Err(())
        }
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

impl TryFrom<&Value> for String {
    type Error = ();

    fn try_from(value: &Value) -> Result<Self, ()> {
        if let Value::String(s) = value {
            Ok(s.to_string())
        } else {
            Err(())
        }
    }
}

impl From<&String> for Value {
    fn from(s: &String) -> Self {
        Self::String(s.clone())
    }
}

impl<V: Into<Value>> From<Vec<V>> for Value {
    fn from(values: Vec<V>) -> Self {
        let values = values.into_iter().map(|v| v.into()).collect();
        Self::Array(Values { values })
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

/// An emitter for indicio that emits values.
pub trait Emitter: Send {
    /// Emit the provided value at the specified file/line.
    fn emit(&self, file: &str, line: u32, level: u64, value: Value);
    /// Flush the emitter with whatever semantics the emitter chooses.
    fn flush(&self) {}
}

impl<E: Emitter + Sync> Emitter for Arc<E> {
    fn emit(&self, file: &str, line: u32, level: u64, value: Value) {
        <E as Emitter>::emit(self, file, line, level, value)
    }

    fn flush(&self) {
        <E as Emitter>::flush(self)
    }
}

/// An emitter that filters log messages above a log level.
pub struct LevelFilter<E: Emitter> {
    pub level: u64,
    pub wrap: E,
}

impl<E: Emitter> Emitter for LevelFilter<E> {
    fn emit(&self, file: &str, line: u32, level: u64, value: Value) {
        if level <= self.level {
            self.wrap.emit(file, line, level, value);
        }
    }

    fn flush(&self) {
        self.wrap.flush()
    }
}

macro_rules! impl_tuple_emitter {
    ($($name:ident)+) => {
        #[allow(non_snake_case)]
        impl<$($name: Emitter),+> Emitter for ($($name,)+) {
            fn emit(&self, file: &str, line: u32, level: u64, value: Value) {
                let ($(ref $name,)+) = *self;
                $(<$name as Emitter>::emit($name, file, line, level, value.clone());)+
            }

            fn flush(&self) {
                let ($(ref $name,)+) = *self;
                $(<$name as Emitter>::flush($name);)+
            }
        }
    };
}

impl_tuple_emitter! { A }
impl_tuple_emitter! { A B }
impl_tuple_emitter! { A B C }
impl_tuple_emitter! { A B C D }
impl_tuple_emitter! { A B C D E }
impl_tuple_emitter! { A B C D E F }
impl_tuple_emitter! { A B C D E F G }
impl_tuple_emitter! { A B C D E F G H }
impl_tuple_emitter! { A B C D E F G H I }
impl_tuple_emitter! { A B C D E F G H I J }
impl_tuple_emitter! { A B C D E F G H I J K }
impl_tuple_emitter! { A B C D E F G H I J K L }
impl_tuple_emitter! { A B C D E F G H I J K L M }
impl_tuple_emitter! { A B C D E F G H I J K L M N }
impl_tuple_emitter! { A B C D E F G H I J K L M N O }
impl_tuple_emitter! { A B C D E F G H I J K L M N O P }
impl_tuple_emitter! { A B C D E F G H I J K L M N O P Q }
impl_tuple_emitter! { A B C D E F G H I J K L M N O P Q R }
impl_tuple_emitter! { A B C D E F G H I J K L M N O P Q R S }
impl_tuple_emitter! { A B C D E F G H I J K L M N O P Q R S T }
impl_tuple_emitter! { A B C D E F G H I J K L M N O P Q R S T U }
impl_tuple_emitter! { A B C D E F G H I J K L M N O P Q R S T U V }
impl_tuple_emitter! { A B C D E F G H I J K L M N O P Q R S T U V W }
impl_tuple_emitter! { A B C D E F G H I J K L M N O P Q R S T U V W X }
impl_tuple_emitter! { A B C D E F G H I J K L M N O P Q R S T U V W X Y }
impl_tuple_emitter! { A B C D E F G H I J K L M N O P Q R S T U V W X Y Z }

///////////////////////////////////////////// Collector ////////////////////////////////////////////

/// A collector is meant to be a static singleton that conditionally logs.
pub struct Collector {
    should_log: AtomicBool,
    verbosity: AtomicU64,
    emitter: Mutex<Option<Box<dyn Emitter>>>,
}

impl Collector {
    /// Create a new collector.
    pub const fn new() -> Self {
        Self {
            should_log: AtomicBool::new(false),
            verbosity: AtomicU64::new(0),
            emitter: Mutex::new(None),
        }
    }

    /// True iff the collector is actively logging.
    pub fn is_logging(&self) -> bool {
        self.should_log.load(Ordering::Relaxed)
    }

    /// The verbosity of the log.  0 is least verbose, N is N levels of verbosity.
    pub fn verbosity(&self) -> u64 {
        self.verbosity.load(Ordering::Relaxed)
    }

    /// Set the verbosity of the log.
    pub fn set_verbosity(&self, verbosity: u64) {
        self.verbosity.store(verbosity, Ordering::Relaxed);
    }

    /// Emit the value via the collector if and only if it is logging and has an emitter
    /// configured.
    pub fn emit(&self, file: &str, line: u32, level: u64, value: Value) {
        if self.is_logging() && self.verbosity() >= level {
            let mut emitter = self.emitter.lock().unwrap();
            if let Some(emitter) = emitter.as_deref_mut() {
                emitter.emit(file, line, level, value);
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

impl Default for Collector {
    fn default() -> Self {
        Self::new()
    }
}

/////////////////////////////////////////////// Clue ///////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "prototk", derive(prototk_derive::Message))]
pub struct Clue {
    #[cfg_attr(feature = "prototk", prototk(1, string))]
    pub file: String,
    #[cfg_attr(feature = "prototk", prototk(2, uint32))]
    pub line: u32,
    #[cfg_attr(feature = "prototk", prototk(3, uint64))]
    pub level: u64,
    #[cfg_attr(feature = "prototk", prototk(4, uint64))]
    pub timestamp: u64,
    #[cfg_attr(feature = "prototk", prototk(5, message))]
    pub value: Value,
}

impl Display for Clue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "{}:{} {} {} {}",
            self.file, self.line, self.level, self.timestamp, self.value
        )
    }
}

////////////////////////////////////////// the clue macro //////////////////////////////////////////

/// Emit the specified value if and only if the collector is logging.
///
/// This will be lazy, and only evaluate the key-value pair if the collector is logging.
#[macro_export]
macro_rules! clue {
    ($collector:path, $level:expr, { $($value:tt)* }) => {
        if $collector.is_logging() && $collector.verbosity() >= $level {
            let loc = concat!(module_path!(), " ", file!());
            $collector.emit(loc, line!(), $level, $crate::value!({ $($value)* }));
        }
    };
}

////////////////////////////////////// the puzzle_piece macro //////////////////////////////////////

/// Match a "shape" of a clue and flatten it.
#[macro_export]
macro_rules! puzzle_piece {
    ($type_name:ident { $($tt:tt)* }) => {
        puzzle_piece!(@struct [$type_name] {} ($($tt)*));
        impl $type_name {
            pub fn extract(value: &indicio::Value) -> Option<Self> {
                let mut this = Self::default();
                puzzle_piece!(@extract this value [] ($($tt)*));
                Some(this)
            }
        }
    };

    (@struct [$type_name:ident] { $($field:ident: $ty:path,)* } ()) => {
        #[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
        pub struct $type_name {
            $($field: $ty,)*
        }
    };

    (@struct [$type_name:ident] { $($field:ident: $ty:path,)* } ($new_field:ident: $new_ty:path, $($tt:tt)*)) => {
        puzzle_piece!(@struct [$type_name] { $($field: $ty,)* $new_field: $new_ty, } ($($tt)*));
    };

    (@struct [$type_name:ident] { $($field:ident: $ty:path,)* } ($new_field:ident: { $($tt1:tt)* })) => {
        puzzle_piece!(@struct [$type_name] { $($field: $ty,)* } ($($tt1)*));
    };

    (@struct [$type_name:ident] { $($field:ident: $ty:path,)* } ($new_field:ident: { $($tt1:tt)* },)) => {
        puzzle_piece!(@struct [$type_name] { $($field: $ty,)* } ($($tt1)*));
    };

    (@struct [$type_name:ident] { $($field:ident: $ty:path,)* } ($new_field:ident: { $($tt1:tt)* }, $($tt2:tt)*)) => {
        puzzle_piece!(@struct [$type_name] { $($field: $ty,)* } ($($tt1)* $($tt2)*));
    };

    (@extract $this:ident $value:ident [] ()) => {
    };

    (@extract $this:ident $value:ident [$($key:ident)*] $new_field:ident) => {
        let v = $value;
        $(let v = v.lookup(stringify!($key))?;)*
        $this.$new_field = v.try_into().ok()?;
    };

    (@extract $this:ident $value:ident [$($key:ident)*] ($new_field:ident: $new_ty:path,)) => {
        puzzle_piece!(@extract $this $value [$($key)* $new_field] $new_field);
    };

    (@extract $this:ident $value:ident [$($key:ident)*] ($new_field:ident: $new_ty:path, $($tt:tt)*)) => {
        puzzle_piece!(@extract $this $value [$($key)* $new_field] $new_field);
        puzzle_piece!(@extract $this $value [$($key)*] ($($tt)*));
    };

    (@extract $this:ident $value:ident [$($key:ident)*] ($new_field:ident: { $($tt1:tt)* }, $($tt2:tt)*)) => {
        puzzle_piece!(@extract $this $value [$($key)* $new_field] ($($tt1)*));
        puzzle_piece!(@extract $this $value [$($key)*] ($($tt2)*));
    };

    (@extract $this:ident $value:ident [$($key:ident)*] ($new_field:ident: { $($tt1:tt)* })) => {
        puzzle_piece!(@extract $this $value [$($key)* $new_field] ($($tt)*));
    };

    (@extract $this:ident $value:ident [$($key:ident)*] ($new_field:ident: { $($tt1:tt)* },)) => {
        puzzle_piece!(@extract $this $value [$($key)* $new_field] ($($tt)*));
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
            Value::Array(
                vec![
                    Value::Bool(false),
                    Value::Bool(true),
                    Value::Array(
                        vec![
                            Value::String("hello".to_string()),
                            Value::String("world".to_string())
                        ]
                        .into()
                    )
                ]
                .into()
            ),
            value!([false, true, ["hello", "world"]])
        );
    }

    #[test]
    fn value_object() {
        assert_eq!(Value::Object(vec![].into_iter().collect()), value!({}));
        assert_eq!(
            Value::Object(
                vec![
                    ("hello".to_string(), Value::String("world".to_string())),
                    (
                        "consts".to_string(),
                        Value::Array(
                            vec![Value::F64(2.718281828459045), Value::F64(3.141592653589793)]
                                .into()
                        )
                    ),
                    (
                        "recursive".to_string(),
                        Value::Object(
                            vec![
                                ("hello".to_string(), Value::String("world".to_string())),
                                (
                                    "consts".to_string(),
                                    Value::Array(
                                        vec![
                                            Value::F64(2.718281828459045),
                                            Value::F64(3.141592653589793)
                                        ]
                                        .into()
                                    )
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
