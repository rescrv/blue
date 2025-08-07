#![doc = include_str!("../README.md")]

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
        let mut args = args.to_vec();
        args.retain(|a| *a != "--");
        reconstructed_args.retain(|a| *a != "--");
        if args != reconstructed_args {
            panic!(
                "non-canonical commandline specified:
provided: {:?}
expected: {:?}
check argument order amongst other differences",
                &args, reconstructed_args
            );
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
            Ok(matches) => matches,
            Err(Fail::OptionMissing(which)) => {
                Self::error(&mut command_line, format!("missing argument: --{which}"));
                Self::usage(&mut command_line, opts, usage);
                return (command_line, vec![]);
            }
            Err(err) => {
                Self::error(
                    &mut command_line,
                    format!("could not parse command line: {err}"),
                );
                Self::exit(&mut command_line, 64);
                return (command_line, vec![]);
            }
        };
        if matches.opt_present("h") {
            Self::usage(&mut command_line, opts, usage);
            return (command_line, vec![]);
        }
        command_line.matches(None, &matches);
        let free: Vec<String> = matches.free.to_vec();
        (command_line, free)
    }

    /// Display the usage and exit 1.
    fn usage(&mut self, opts: getopts::Options, brief: &str) {
        self.error(opts.usage(brief));
        self.exit(1);
    }

    /// Report an error.
    fn error(&mut self, msg: impl AsRef<str>) {
        eprintln!("{}", msg.as_ref());
    }

    /// Exit with the provided status.
    fn exit(&mut self, status: i32) {
        std::process::exit(status);
    }
}

///////////////////////////////////////// NoExitCommandLine ////////////////////////////////////////

/// A non-exiting wrapper for command line parsing.  Will store command line in 0, messages in
/// element 1, exit status in 2.
#[derive(Default, Eq, PartialEq)]
pub struct NoExitCommandLine<T: CommandLine>(T, Vec<String>, i32);

impl<T: CommandLine> AsRef<T> for NoExitCommandLine<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T: CommandLine> NoExitCommandLine<T> {
    pub fn into_inner(self) -> T {
        self.0
    }

    pub fn into_parts(self) -> (T, Vec<String>, i32) {
        (self.0, self.1, self.2)
    }
}

impl<T: CommandLine> CommandLine for NoExitCommandLine<T> {
    fn add_opts(&self, prefix: Option<&str>, opts: &mut getopts::Options) {
        self.0.add_opts(prefix, opts);
    }

    fn matches(&mut self, prefix: Option<&str>, matches: &getopts::Matches) {
        self.0.matches(prefix, matches);
    }

    fn canonical_command_line(&self, prefix: Option<&str>) -> Vec<String> {
        self.0.canonical_command_line(prefix)
    }

    fn error(&mut self, msg: impl AsRef<str>) {
        self.1.push(msg.as_ref().to_string());
    }

    fn exit(&mut self, status: i32) {
        self.2 = status;
    }
}

//////////////////////////////////////////// macro utils ///////////////////////////////////////////

#[doc(hidden)]
pub fn getopt_str(prefix: Option<&str>, field_arg: &str) -> String {
    match prefix {
        Some(prefix) => {
            format!("{prefix}-{field_arg}")
        }
        None => field_arg.to_string(),
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
            panic!("field --{arg_str} is unparseable: {err}");
        }
    }
}
