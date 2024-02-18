#![doc = include_str!("../README.md")]

use std::cmp::Ordering;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::str::FromStr;
use std::time::{Duration, Instant};

mod moments;
mod t_test;

pub use moments::Moments;
pub use t_test::{compute_difference, summarize};

/////////////////////////////////////////////// Type ///////////////////////////////////////////////

/// Type captures the type of a [Parameter].
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Type {
    Unit,
    Integer,
    Float,
    Bool,
    Text,
}

impl Display for Type {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Self::Unit => write!(f, "unit"),
            Self::Integer => write!(f, "int"),
            Self::Float => write!(f, "float"),
            Self::Bool => write!(f, "bool"),
            Self::Text => write!(f, "text"),
        }
    }
}

impl FromStr for Type {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "unit" => Ok(Type::Unit),
            "int" => Ok(Type::Integer),
            "integer" => Ok(Type::Integer),
            "u64" => Ok(Type::Integer),
            "float" => Ok(Type::Float),
            "f64" => Ok(Type::Float),
            "bool" => Ok(Type::Bool),
            "text" => Ok(Type::Text),
            "str" => Ok(Type::Text),
            "string" => Ok(Type::Text),
            _ => Err(format!("invalid type: {s}")),
        }
    }
}

///////////////////////////////////////////// Parameter ////////////////////////////////////////////

/// Parameter binds a typed value to an experiment.
#[derive(Clone, Debug)]
pub enum Parameter {
    Unit,
    Integer(u64),
    Float(f64),
    Bool(bool),
    Text(String),
}

impl Parameter {
    pub fn ty(&self) -> Type {
        match self {
            Self::Unit => Type::Unit,
            Self::Integer(_) => Type::Integer,
            Self::Float(_) => Type::Float,
            Self::Bool(_) => Type::Bool,
            Self::Text(_) => Type::Text,
        }
    }

    pub fn cast_float(&self) -> Parameter {
        match self {
            Self::Integer(x) => Self::Float(*x as f64),
            Self::Float(x) => Self::Float(*x),
            Self::Unit => self.clone(),
            Self::Bool(_) => self.clone(),
            Self::Text(_) => self.clone(),
        }
    }
}

impl Display for Parameter {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Self::Unit => Ok(()),
            Self::Integer(i) => write!(fmt, "{i}"),
            Self::Float(f) => write!(fmt, "{f}"),
            Self::Bool(b) => write!(fmt, "{}", if *b { "true" } else { "false" }),
            Self::Text(t) => write!(fmt, "{t}"),
        }
    }
}

impl Eq for Parameter {}

impl PartialEq for Parameter {
    fn eq(&self, other: &Parameter) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Ord for Parameter {
    fn cmp(&self, other: &Parameter) -> Ordering {
        let ty_cmp = self.ty().cmp(&other.ty());
        if !ty_cmp.is_eq() {
            ty_cmp
        } else {
            match (self, other) {
                (Self::Unit, Self::Unit) => Ordering::Equal,
                (Self::Integer(x), Self::Integer(y)) => x.cmp(y),
                (Self::Float(x), Self::Float(y)) => x.total_cmp(y),
                (Self::Bool(x), Self::Bool(y)) => x.cmp(y),
                (Self::Text(x), Self::Text(y)) => x.cmp(y),
                _ => unreachable!(),
            }
        }
    }
}

impl PartialOrd for Parameter {
    fn partial_cmp(&self, other: &Parameter) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl From<()> for Parameter {
    fn from(_: ()) -> Self {
        Self::Unit
    }
}

impl From<u64> for Parameter {
    fn from(x: u64) -> Self {
        Self::Integer(x)
    }
}

impl From<f64> for Parameter {
    fn from(x: f64) -> Self {
        Self::Float(x)
    }
}

impl From<bool> for Parameter {
    fn from(x: bool) -> Self {
        Self::Bool(x)
    }
}

impl From<String> for Parameter {
    fn from(x: String) -> Self {
        Self::Text(x)
    }
}

impl FromStr for Parameter {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            Ok(Parameter::Unit)
        } else if let Ok(i) = u64::from_str(s) {
            Ok(Parameter::Integer(i))
        } else if let Ok(f) = f64::from_str(s) {
            Ok(Parameter::Float(f))
        } else if s == "true" || s == "yes" {
            Ok(Parameter::Bool(true))
        } else if s == "false" || s == "no" {
            Ok(Parameter::Bool(false))
        } else {
            Ok(Parameter::Text(s.to_string()))
        }
    }
}

//////////////////////////////////////////// Parameters ////////////////////////////////////////////

/// Parameters provides the parameters for an experiment/benchmark.
pub trait Parameters: Default {
    /// Return the parameters as a list of name, parameter.
    fn params(&self) -> Vec<(&'static str, Parameter)>;

    /// Return the canonical parameter string for these parameters, suitable for passing to
    /// UntypedParameters::from_str.
    fn parameter_string(&self) -> String {
        let mut s = String::new();
        for (name, param) in self.params() {
            if !s.is_empty() {
                s.push(',');
            }
            s += &format!("{}={}", name, param);
        }
        s
    }
}

///////////////////////////////////////// UnboundParameters ////////////////////////////////////////

/// Unbound parameters convers from a string like foo,bar,baz,quux.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UnboundParameters {
    values: Vec<String>,
}

impl UnboundParameters {
    /// Whether there are parameters.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Number of bound parameters.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Iterate over the values.
    pub fn iter(&self) -> impl Iterator<Item = &'_ str> + '_ {
        self.values.iter().map(|s| s.as_str())
    }

    /// Has a parameter.
    pub fn has(&self, name: &str) -> bool {
        self.values.iter().any(|v| *v == name)
    }

    /// Project the provided UntypedParameters to an UntypedParameters that contains the same
    /// parameters as this struct, in order.
    pub fn project(&self, untyped: &UntypedParameters) -> Result<UntypedParameters, String> {
        let mut projected = vec![];
        for name in self.values.iter() {
            if let Some(p) = untyped.get(name) {
                projected.push((name.clone(), p));
            } else {
                return Err(format!(
                    "cannot unify {self} with {untyped}: missing {name}"
                ));
            }
        }
        Ok(UntypedParameters { values: projected })
    }

    /// Add the unbound parameter to this.
    pub fn push(&mut self, name: String) {
        self.values.push(name);
    }
}

impl Display for UnboundParameters {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "{}", self.values.join(","))
    }
}

impl From<&UntypedParameters> for UnboundParameters {
    fn from(params: &UntypedParameters) -> Self {
        let values = params.values.iter().map(|p| p.0.clone()).collect();
        Self { values }
    }
}

impl FromStr for UnboundParameters {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut values = vec![];
        for x in s.split(',') {
            if x.is_empty() {
                continue;
            }
            values.push(x.to_string())
        }
        Ok(Self { values })
    }
}

///////////////////////////////////////// UntypedParameters ////////////////////////////////////////

/// Untyped parameters converts from a string like foo=1,bar=3.14,baz=true,quux.  The input is
/// untyped, and the result is free-form, as opposed to a type like what implements [Parameters].
#[derive(Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct UntypedParameters {
    values: Vec<(String, Parameter)>,
}

impl UntypedParameters {
    /// Create an untyped parameters list from one element.
    pub fn one(s: String, p: Parameter) -> Self {
        let values = vec![(s, p)];
        Self { values }
    }

    /// Whether there are parameters.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Number of bound parameters.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Iterate over the values.
    pub fn iter(&self) -> impl Iterator<Item = (String, Parameter)> + '_ {
        self.values.iter().cloned()
    }

    /// Get a parameter by name.
    pub fn get(&self, name: &str) -> Option<Parameter> {
        for v in self.values.iter() {
            if v.0 == name {
                return Some(v.1.clone());
            }
        }
        None
    }

    /// Cast all integer parameters to float, leaving the rest as-is.
    pub fn cast_float(&self) -> Self {
        let values = self
            .values
            .iter()
            .map(|p| (p.0.clone(), p.1.cast_float()))
            .collect();
        Self { values }
    }
}

impl Display for UntypedParameters {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        for (idx, value) in self.values.iter().enumerate() {
            if idx > 0 {
                write!(fmt, ",")?;
            }
            if value.1 == Parameter::Unit {
                write!(fmt, "{}", value.0)?;
            } else {
                write!(fmt, "{}={}", value.0, value.1)?;
            }
        }
        Ok(())
    }
}

impl FromStr for UntypedParameters {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut values = vec![];
        for x in s.split(',') {
            if x.is_empty() {
                continue;
            }
            if x.contains('=') {
                let pieces = x.splitn(2, '=').collect::<Vec<_>>();
                if pieces.len() != 2 {
                    return Err(format!("error parsing {x}: too many = signs"));
                }
                let name = pieces[0];
                let val = Parameter::from_str(pieces[1])
                    .map_err(|err| format!("could not parse parameter {}: {:?}", pieces[1], err))?;
                values.push((name.to_string(), val));
            } else {
                values.push((x.to_string(), Parameter::Unit));
            }
        }
        Ok(Self { values })
    }
}

///////////////////////////////////// experiment_and_parameters ////////////////////////////////////

pub fn experiment_and_parameters(s: &str) -> Result<(&str, UntypedParameters), String> {
    let pieces = s.rsplitn(2, ':').collect::<Vec<_>>();
    if pieces.len() != 2 {
        return Err(format!("don't know how to make cdf: {}", s));
    }
    let params = UntypedParameters::from_str(pieces[0]).expect("don't know how to make cdf");
    let pieces = pieces[1].rsplitn(2, '/').collect::<Vec<_>>();
    if pieces.is_empty() {
        return Err(format!("don't know how to make cdf: {}", s));
    }
    Ok((pieces[0], params))
}

///////////////////////////////////////////// black_box ////////////////////////////////////////////

/// Try to prevent the compiler from eliminating code.
// Copied from criterion under the Apache 2.0 or MIT licenses.
pub fn black_box<T>(dummy: T) -> T {
    unsafe {
        let ret = std::ptr::read_volatile(&dummy);
        std::mem::forget(dummy);
        ret
    }
}

///////////////////////////////////////// increment_indices ////////////////////////////////////////

pub fn increment_indices(indices: &mut Vec<usize>, limits: &[usize]) {
    assert_eq!(indices.len(), limits.len());
    for ((x, index), limit) in
        std::iter::zip(indices.iter_mut().enumerate().rev(), limits.iter().rev())
    {
        *index += 1;
        if x > 0 && *index >= *limit {
            *index = 0;
        } else {
            break;
        }
    }
}

//////////////////////////////////////////// benchmark! ////////////////////////////////////////////

/// A macro for defining a benchmark sweep.
#[macro_export]
macro_rules! benchmark {
    (name = $name:ident; $params:ident { $($param:ident in $set:expr,)* } $(,)? $bench:path $(,)?) => {
        fn $name(options: &$crate::BenchmarkOptions, filter: Option<String>) {
            let mut indices = vec![];
            let mut limits = vec![];
            $(
                indices.push(0);
                limits.push($set.iter().count());
                let $param = $set;
            )*
            while !indices.is_empty() && indices[0] < limits[0] {
                let mut count = 0;
                let mut params = $params::default();
                $(
                    params.$param = $param[indices[count]];
                    count += 1;
                )*
                $crate::increment_indices(&mut indices, &limits);
                let benchmark_name = format!("{}:{}", stringify!($name), params.parameter_string());
                if let Some(filter) = filter.as_ref() {
                    if !benchmark_name.contains(filter) {
                        continue;
                    }
                }
                if !options.quiet {
                    eprintln!("executing {}", benchmark_name);
                }
                $crate::benchmark_main(stringify!($name), options, &params, $bench);
            }
        }
    };
    (name = $name:ident; $params:ident { $($param:ident in $set:expr),* } $(,)? $bench:ident $(,)?) => {
        benchmark! { name = $name; $params { $($param in $set),* } $bench }
    };
}

////////////////////////////////////////// benchmark_main //////////////////////////////////////////

pub fn benchmark_main<P: Parameters, F: FnMut(&P, &mut Bencher)>(
    name: &str,
    options: &BenchmarkOptions,
    params: &P,
    mut f: F,
) {
    let output = if options.added_params.is_empty() {
        options.output_prefix.clone() + name + ":" + &params.parameter_string() + ".dat"
    } else {
        options.output_prefix.clone()
            + name
            + ":"
            + &params.parameter_string()
            + ","
            + &options.added_params
            + ".dat"
    };
    let output = PathBuf::from(output);
    let parent = output
        .parent()
        .map(PathBuf::from)
        .unwrap_or(PathBuf::from("."));
    if !parent.exists() {
        eprintln!(
            "output directory does not exist: {}",
            parent.to_string_lossy()
        );
        std::process::exit(1);
    }
    if options.noclobber && output.exists() {
        if !options.quiet {
            eprintln!("benchmark exists; moving on");
        }
        return;
    }
    let mut hist = sig_fig_histogram::Histogram::new(options.sig_figs);
    let mut size = 16;
    const SEED_FACTOR: u64 = 4294967291u64;
    let mut seed = SEED_FACTOR;
    while size < usize::MAX {
        let mut b = Bencher::new(size, seed, false);
        seed = seed.wrapping_mul(SEED_FACTOR);
        f(params, &mut b);
        if b.elapsed > Duration::from_millis(options.target_time) {
            break;
        }
        size = size.saturating_add(size >> 2);
    }
    if size == usize::MAX {
        eprintln!("could not determine an appropriate size for this benchmark");
        std::process::exit(1);
    }
    if !options.quiet {
        eprintln!("sizing benchmark at {}", size);
    }
    let warm_up = Duration::from_secs(options.warm_up);
    if !options.quiet {
        eprintln!("warming up for {}s", options.warm_up);
    }
    let start = Instant::now();
    while start.elapsed() < warm_up {
        let mut b = Bencher::new(size, seed, false);
        seed = seed.wrapping_mul(SEED_FACTOR);
        f(params, &mut b);
    }
    for i in 0..options.iterations {
        if !options.quiet
            && i > 0
            && options.iterations > 100
            && i % (options.iterations / 100) == 0
        {
            eprintln!(
                "done {} iterations ({}%)",
                i,
                i / (options.iterations / 100)
            );
        }
        let mut b = Bencher::new(size, seed, true);
        seed = seed.wrapping_mul(SEED_FACTOR);
        f(params, &mut b);
        let elapsed: u64 = (b.elapsed.as_nanos())
            .try_into()
            .expect("expect operations to take fewer than 834 days");
        assert!(elapsed < 1 << 55);
        hist.observe(elapsed as f64 / size as f64)
            .expect("histogram should never fail");
    }
    let output = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(output)
        .expect("output file should open");
    hist.dump(output).expect("histogram should always dump");
    if !options.quiet {
        eprintln!("done {} iterations (100%)", options.iterations);
    }
}

///////////////////////////////////////////// benchmark ////////////////////////////////////////////

/// A stub used to run the benchmarks, so that profilers can zero counters on entry and count them
/// on exit.
#[inline(never)]
pub fn benchmark<F: FnOnce()>(f: F) {
    f();
}

////////////////////////////////////////////// Bencher /////////////////////////////////////////////

pub struct Bencher {
    size: usize,
    seed: u64,
    real: bool,
    elapsed: Duration,
}

impl Bencher {
    fn new(size: usize, seed: u64, real: bool) -> Self {
        let elapsed = Duration::ZERO;
        Self {
            size,
            seed,
            real,
            elapsed,
        }
    }

    pub fn run<F: FnOnce()>(&mut self, f: F) {
        let start = Instant::now();
        if self.real {
            benchmark(f);
        } else {
            f();
        }
        self.elapsed = start.elapsed();
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn size(&self) -> usize {
        self.size
    }
}

///////////////////////////////////////// BenchmarkOptions /////////////////////////////////////////

/// Options for the benchmark.
#[derive(Debug, Eq, PartialEq, arrrg_derive::CommandLine)]
pub struct BenchmarkOptions {
    /// Run the benchmark.
    #[arrrg(flag, "Run the benchmark.")]
    pub bench: bool,
    /// Run the benchmark quietly.
    #[arrrg(flag, "Run the benchmark quietly.")]
    pub quiet: bool,
    /// Do not overwrite existing results.
    #[arrrg(flag, "Do not overwrite existing results.")]
    pub noclobber: bool,
    /// A seed for random data.
    #[arrrg(optional, "Guacamole seed for random data.")]
    pub seed: u64,
    /// Number of seconds to spend warming up on the benchmark.
    #[arrrg(optional, "Seconds to spend warming up the benchmark.")]
    pub warm_up: u64,
    /// Number of milliseconds to target for each iteration.
    #[arrrg(optional, "Milliseconds to target for each iteration.")]
    pub target_time: u64,
    /// Number of iterations to execute.
    #[arrrg(optional, "Iterations to run.")]
    pub iterations: u64,
    /// Number of significant figures to use in the output.
    #[arrrg(optional, "Significant figures to use.")]
    pub sig_figs: i32,
    /// Added parameters.
    #[arrrg(optional, "Added parameters.")]
    pub added_params: String,
    /// The output prefix for the benchmark.  Will be joined with
    /// "{benchmark_name}:{untyped_parameters}".
    #[arrrg(optional, "Output prefix for histograms.")]
    pub output_prefix: String,
}

impl Default for BenchmarkOptions {
    fn default() -> Self {
        Self {
            bench: false,
            quiet: false,
            noclobber: false,
            seed: 0,
            warm_up: 5,
            target_time: 100,
            iterations: 1000,
            sig_figs: 3,
            added_params: "".to_string(),
            output_prefix: "exp/".to_string(),
        }
    }
}

////////////////////////////////////////// statslicer_main /////////////////////////////////////////

/// The macro for creating main functions.
#[macro_export]
macro_rules! statslicer_main {
    ($($name:ident),* $(,)?) => {
        fn main() {
            use arrrg::CommandLine;
            let mut args: Vec<String> = std::env::args().collect();
            if args.len() > 2 && args[args.len() - 1] == "--bench" {
                args.pop();
                args.insert(1, "--bench".to_string());
            }
            let usage = format!("USAGE: {} [PARAMETERS]", args[0]);
            let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            let (params, mut free) =
                $crate::BenchmarkOptions::from_arguments_relaxed(&usage, &args[1..]);
            if free.len() > 1 {
                eprintln!("benchmark takes at most one positional argument");
                std::process::exit(1);
            }
            let filter = free.pop();
            if !params.bench {
                std::process::exit(0);
            }
            if !(1..=4).contains(&params.sig_figs) {
                eprintln!("significant figures must be [0, 5)");
                std::process::exit(1);
            }
            $($name(&params, filter.clone());)*
        }
    };
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_integer() {
        assert_eq!("int", Type::Integer.to_string());
        assert_eq!(Ok(Type::Integer), Type::from_str("int"));
        assert_eq!(Ok(Type::Integer), Type::from_str("integer"));
        assert_eq!(Ok(Type::Integer), Type::from_str("u64"));
    }

    #[test]
    fn type_float() {
        assert_eq!("float", Type::Float.to_string());
        assert_eq!(Ok(Type::Float), Type::from_str("float"));
        assert_eq!(Ok(Type::Float), Type::from_str("f64"));
    }

    #[test]
    fn type_bool() {
        assert_eq!("bool", Type::Bool.to_string());
        assert_eq!(Ok(Type::Bool), Type::from_str("bool"));
    }

    #[test]
    fn type_text() {
        assert_eq!("text", Type::Text.to_string());
        assert_eq!(Ok(Type::Text), Type::from_str("text"));
        assert_eq!(Ok(Type::Text), Type::from_str("str"));
        assert_eq!(Ok(Type::Text), Type::from_str("string"));
    }

    #[test]
    fn type_error() {
        assert!(Type::from_str("foo").is_err());
    }

    #[test]
    fn param_integer() {
        assert_eq!(Ok(Parameter::Integer(0)), Parameter::from_str("0"));
        assert_eq!(Ok(Parameter::Integer(42)), Parameter::from_str("42"));
    }

    #[test]
    #[allow(clippy::approx_constant)]
    fn param_float() {
        assert_eq!(Ok(Parameter::Float(3.14)), Parameter::from_str("3.14"));
        assert_eq!(Ok(Parameter::Float(2.72)), Parameter::from_str("2.72"));
    }

    #[test]
    fn param_bool() {
        assert_eq!(Ok(Parameter::Bool(true)), Parameter::from_str("true"));
        assert_eq!(Ok(Parameter::Bool(false)), Parameter::from_str("false"));
    }

    #[test]
    fn param_text() {
        assert_eq!(
            Ok(Parameter::Text("foo".to_string())),
            Parameter::from_str("foo")
        );
        assert_eq!(
            Ok(Parameter::Text("bar".to_string())),
            Parameter::from_str("bar")
        );
    }

    #[test]
    fn untyped_parameters() {
        assert_eq!(
            Ok(UntypedParameters {
                values: vec![
                    ("foo".to_string(), Parameter::Integer(42)),
                    ("bar".to_string(), Parameter::Float(2.72)),
                    ("baz".to_string(), Parameter::Bool(true)),
                    ("quux".to_string(), Parameter::Text("ins".to_string())),
                    ("zed".to_string(), Parameter::Unit)
                ]
            }),
            UntypedParameters::from_str("foo=42,bar=2.72,baz=true,quux=ins,zed")
        );
    }
}
