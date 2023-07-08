//! arrrg provides an opinionated [CommandLine] parser.
//!
//! For example, let's consider the parser specified here using the derive syntax:
//!
//! ```
//! use arrrg_derive::CommandLine;
//!
//! #[derive(CommandLine, Debug, Default, Eq, PartialEq)]
//! struct Options {
//!     #[arrrg(optional, "this is the help text", "PLACEHOLDER")]
//!     some_string: String,
//!     #[arrrg(nested)]
//!     some_prefix: SomeOptions,
//! }
//!
//! #[derive(CommandLine, Debug, Default, Eq, PartialEq)]
//! struct SomeOptions {
//!     #[arrrg(required, "this is the help text", "PLACEHOLDER")]
//!     a: String,
//!     #[arrrg(optional, "this is the help text", "PLACEHOLDER")]
//!     b: String,
//! }
//! ```
//!
//! This will provide the options to getopts of `--some-string`, `--some-prefix-a`,
//! `--some-prefix-b`.  In general the rule is to derive the flag names from the identifiers of
//! struct members.  When nesting the name will be the concatenation of the prefix from the parent
//! struct and the member identifier from the child struct.  Unlimited nesting is possible.
//!
//! This library takes an opinionated stance on the command line.  There should be exactly one
//! canonical argument order on the command-line and all applications must be built with this in
//! mind.  Users of the library can call [from_command_line_relaxed] to disable this checking.

use std::str::FromStr;

use getopts::Fail;

//////////////////////////////////////////// CommandLine ///////////////////////////////////////////

/// [CommandLine] creates a command line parser for anyone who implements [CommandLine::add_opts],
/// [CommandLine::matches] and [CommandLine::canonical_command_line].  This is a wrapper around
/// getopts to tie together options and matches.
pub trait CommandLine: Sized + Default + Eq + PartialEq {
    /// Add options to the getopts parser.
    fn add_opts(&self, prefix: Option<&str>, opts: &mut getopts::Options);

    /// Assign values to self using the provided getopts matches.
    fn matches(&mut self, prefix: Option<&str>, matches: &getopts::Matches);

    /// Return the canonical command line for this [CommandLine].
    fn canonical_command_line(&self, prefix: Option<&str>) -> Vec<String>;

    /// Parse from the command line.  This function will panic if a non-canonical command line is
    /// provided.
    fn from_command_line(usage: &str) -> (Self, Vec<String>) {
        let args: Vec<String> = std::env::args().collect();
        let args: Vec<&str> = args.iter().map(AsRef::as_ref).collect();
        Self::from_arguments(usage, &args[1..])
    }

    /// Parse from the command line.  This function will allow a non-canonical command line to
    /// execute.
    fn from_command_line_relaxed(usage: &str) -> (Self, Vec<String>) {
        let args: Vec<String> = std::env::args().collect();
        let args: Vec<&str> = args.iter().map(AsRef::as_ref).collect();
        Self::from_arguments_relaxed(usage, &args[1..])
    }

    /// Parse from the provided arguments.  This function will panic if a non-canonical command
    /// line is provided.
    fn from_arguments(usage: &str, args: &[&str]) -> (Self, Vec<String>) {
        let (command_line, free) = Self::from_arguments_relaxed(usage, args);
        let mut reconstructed_args = command_line.canonical_command_line(None);
        let mut free_p = free.clone();
        reconstructed_args.append(&mut free_p);
        if args != reconstructed_args {
            panic!("non-canonical commandline specified:
provided: {:?}
expected: {:?}
check argument order amongst other differences",
                   &args, reconstructed_args);
        }
        (command_line, free)
    }

    /// Parse from the provided arguments.  This function will allow a non-canonical command line to
    /// execute.
    fn from_arguments_relaxed(usage: &str, args: &[&str]) -> (Self, Vec<String>) {
        let mut command_line = Self::default();
        let mut opts = getopts::Options::new();
        opts.parsing_style(getopts::ParsingStyle::StopAtFirstFree);
        opts.long_only(true);
        opts.optflag("h", "help", "Print this help menu.");
        command_line.add_opts(None, &mut opts);

        let matches = match opts.parse(args) {
            Ok(matches) => { matches },
            Err(Fail::OptionMissing(which)) => {
                eprintln!("missing argument: --{}", which);
                command_line.usage(opts, usage);
            },
            Err(err) => {
                eprintln!("could not parse command line: {}", err);
                std::process::exit(64);
            },
        };
        if matches.opt_present("h") {
            command_line.usage(opts, usage);
        }
        command_line.matches(None, &matches);
        let free: Vec<String> = matches.free.to_vec();

        (command_line, free)
    }

    /// Display the usage and exit 1.
    fn usage(&mut self, opts: getopts::Options, brief: &str) -> ! {
        print!("{}", opts.usage(brief));
        std::process::exit(1);
    }
}

//////////////////////////////////////////// macro utils ///////////////////////////////////////////

#[doc(hidden)]
pub fn getopt_str(prefix: Option<&str>, field_arg: &str) -> String {
    match prefix {
        Some(prefix) => { format!("{}-{}", prefix, field_arg) },
        None => { field_arg.to_string() },
    }
}

#[doc(hidden)]
pub fn dashed_str(prefix: Option<&str>, field_arg: &str) -> String {
    format!("--{}", getopt_str(prefix, field_arg))
}

#[doc(hidden)]
pub fn parse_field<T>(arg_str: &str, s: &str) -> T
where
    T: FromStr,
    <T as FromStr>::Err: std::fmt::Display,
{
    match s.parse::<T>() {
        Ok(t) => t,
        Err(err) => {
            panic!("field --{} is unparseable: {}", arg_str, err);
        },
    }
}
