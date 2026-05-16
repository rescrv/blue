#![doc = include_str!("../README.md")]

use std::collections::HashSet;
use std::fmt;
use std::str::FromStr;

use getopts::Fail;

//////////////////////////////////////////// subcommands ///////////////////////////////////////////

/// Error returned when subcommand dispatch cannot select a command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubcommandError {
    command: Option<String>,
    expected: Vec<String>,
}

impl SubcommandError {
    /// Create a missing-subcommand error.
    pub fn missing(expected: Vec<String>) -> Self {
        Self {
            command: None,
            expected,
        }
    }

    /// Create an unknown-subcommand error.
    pub fn unknown(command: String, expected: Vec<String>) -> Self {
        Self {
            command: Some(command),
            expected,
        }
    }

    /// Return the unknown subcommand, or `None` when no subcommand was provided.
    pub fn command(&self) -> Option<&str> {
        self.command.as_deref()
    }

    /// Return the expected command names.
    pub fn expected(&self) -> &[String] {
        &self.expected
    }
}

impl fmt::Display for SubcommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(command) = &self.command {
            write!(f, "unknown subcommand {command:?}")?;
        } else {
            write!(f, "missing subcommand")?;
        }
        write_expected_subcommands(f, &self.expected)
    }
}

impl std::error::Error for SubcommandError {}

fn write_expected_subcommands(f: &mut fmt::Formatter<'_>, expected: &[String]) -> fmt::Result {
    if expected.is_empty() {
        return Ok(());
    }
    write!(f, "; expected one of: {}", expected.join(", "))
}

/// Split a command line's free arguments into `(subcommand, remaining_args)`.
///
/// arrrg parsers use `getopts::ParsingStyle::StopAtFirstFree`, so recursive subcommand parsing is
/// naturally represented as:
///
/// 1. parse the current command's options,
/// 2. split the first free argument as the subcommand name,
/// 3. parse the remaining free arguments with the selected subcommand's [CommandLine].
pub fn split_subcommand(
    free: Vec<String>,
    command_names: &[&str],
) -> Result<(String, Vec<String>), SubcommandError> {
    let mut free = free.into_iter();
    let expected = command_names
        .iter()
        .map(|command| command.to_string())
        .collect::<Vec<_>>();
    let Some(command) = free.next() else {
        return Err(SubcommandError::missing(expected));
    };
    if !command_names.iter().any(|name| *name == command) {
        return Err(SubcommandError::unknown(command, expected));
    }
    Ok((command, free.collect()))
}

/// Return the default usage string used by the subcommand dispatch macros.
pub fn usage_for_subcommand(command: &str) -> String {
    format!("Usage: {command} [OPTIONS]")
}

/// Dispatch a `Vec<String>` of free arguments to a named subcommand.
///
/// Each branch is a command-name, [CommandLine] pair plus a typed handler.  The selected branch gets
/// a concrete command-line value and the remaining free arguments.  Branch handlers return
/// `Result<T, SubcommandError>`, which lets the macro compose recursively without enums or
/// downcasts.
///
/// ```rust
/// # use arrrg::CommandLine;
/// # #[derive(Default, Eq, PartialEq)]
/// # struct Top;
/// # impl CommandLine for Top {
/// #     fn add_opts(&self, _: Option<&str>, _: &mut getopts::Options) {}
/// #     fn matches(&mut self, _: Option<&str>, _: &getopts::Matches) {}
/// #     fn canonical_command_line(&self, _: Option<&str>) -> Vec<String> { vec![] }
/// # }
/// # #[derive(Default, Eq, PartialEq)]
/// # struct Get;
/// # impl CommandLine for Get {
/// #     fn add_opts(&self, _: Option<&str>, _: &mut getopts::Options) {}
/// #     fn matches(&mut self, _: Option<&str>, _: &getopts::Matches) {}
/// #     fn canonical_command_line(&self, _: Option<&str>) -> Vec<String> { vec![] }
/// # }
/// # let (_top, free) = Top::from_arguments("Usage: tool [OPTIONS] <command>", &["get"]);
/// let selected = arrrg::dispatch_subcommands!(free, {
///     "get" => Get as get, get_free => {
///         Ok((get, get_free))
///     },
/// });
/// # assert!(selected.is_ok());
/// ```
#[macro_export]
macro_rules! dispatch_subcommands {
    ($free:expr, { $($name:literal => $ty:ty as $cmd:ident, $rest:ident => $body:block),+ $(,)? }) => {{
        $crate::__dispatch_subcommands!(from_arguments, $free, {
            $($name => $ty as $cmd, $rest => $body),+
        })
    }};
}

/// Dispatch subcommands while allowing non-canonical command-line argument order.
///
/// This is the subcommand equivalent of [CommandLine::from_arguments_relaxed].
#[macro_export]
macro_rules! dispatch_subcommands_relaxed {
    ($free:expr, { $($name:literal => $ty:ty as $cmd:ident, $rest:ident => $body:block),+ $(,)? }) => {{
        $crate::__dispatch_subcommands!(from_arguments_relaxed, $free, {
            $($name => $ty as $cmd, $rest => $body),+
        })
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __dispatch_subcommands {
    ($parser:ident, $free:expr, { $($name:literal => $ty:ty as $cmd:ident, $rest:ident => $body:block),+ $(,)? }) => {{
        match $crate::split_subcommand($free, &[$($name),+]) {
            Ok((__arrrg_command, __arrrg_rest)) => {
                match __arrrg_command.as_str() {
                    $(
                        $name => {
                            let __arrrg_args = __arrrg_rest
                                .iter()
                                .map(::std::string::String::as_str)
                                .collect::<::std::vec::Vec<_>>();
                            let __arrrg_usage = $crate::usage_for_subcommand($name);
                            let ($cmd, $rest) =
                                <$ty as $crate::CommandLine>::$parser(&__arrrg_usage, &__arrrg_args);
                            $body
                        }
                    )+
                    _ => unreachable!("split_subcommand returned an unexpected command"),
                }
            }
            Err(__arrrg_err) => Err(__arrrg_err),
        }
    }};
}

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
        opts.optflag(
            "",
            "completions",
            "Print zsh completion script for this command.",
        );
        command_line.add_opts(None, &mut opts);

        if args
            .iter()
            .any(|arg| arg == &"--completions" || arg == &"-completions")
        {
            Self::completions(
                &mut command_line,
                &opts,
                usage,
                &std::env::args()
                    .next()
                    .unwrap_or_else(|| "command".to_string()),
            );
            return (command_line, vec![]);
        }

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
        if matches.opt_present("completions") {
            Self::completions(
                &mut command_line,
                &opts,
                usage,
                &std::env::args()
                    .next()
                    .unwrap_or_else(|| "command".to_string()),
            );
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

    /// Emit a zsh completion script for the current command line and terminate successfully.
    ///
    /// The function derives the completion entries from the full option set already defined on `opts`
    /// and prints a minimal zsh completion function scoped to the executable in `command_path`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use arrrg::CommandLine;
    ///
    /// #[derive(Default, Eq, PartialEq)]
    /// struct MyCmd;
    ///
    /// impl CommandLine for MyCmd {
    ///     fn add_opts(&self, _prefix: Option<&str>, opts: &mut getopts::Options) {
    ///         opts.optflag("h", "help", "print help");
    ///     }
    ///
    ///     fn matches(&mut self, _prefix: Option<&str>, _matches: &getopts::Matches) {}
    ///
    ///     fn canonical_command_line(&self, _prefix: Option<&str>) -> Vec<String> {
    ///         vec!["my-cmd".to_string()]
    ///     }
    /// }
    /// ```
    /// In normal usage, invoking `--completions` with a live implementation will print text and
    /// exit the process with code `0`.
    fn completions(&mut self, opts: &getopts::Options, usage: &str, command_path: &str) {
        print!("{}", zsh_completions(opts, usage, command_path));
        self.exit(0);
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

fn zsh_completion_entries(opts: &getopts::Options, usage: &str) -> Vec<String> {
    let usage = opts.usage(usage);
    let mut options = Vec::new();
    let mut collecting = false;
    for line in usage.lines() {
        if line.starts_with("Options:") {
            collecting = true;
            continue;
        }
        if !collecting {
            continue;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !trimmed.starts_with('-') {
            continue;
        }
        let mut split_idx = None;
        for (i, b) in trimmed.bytes().enumerate() {
            if b == b'\t' {
                split_idx = Some(i);
                break;
            }
            if b.is_ascii_whitespace()
                && i + 1 < trimmed.len()
                && trimmed.as_bytes()[i + 1].is_ascii_whitespace()
            {
                split_idx = Some(i);
                break;
            }
        }
        let flag_part = if let Some(i) = split_idx {
            trimmed[..i].trim().to_string()
        } else {
            trimmed.to_string()
        };
        if flag_part.is_empty() {
            continue;
        }
        let desc_part = if let Some(i) = split_idx {
            trimmed[i..].trim()
        } else {
            ""
        };
        for name in parse_option_names(&flag_part) {
            options.push(format_completion_option(&name, desc_part));
        }
    }
    options
}

fn parse_option_names(flag_part: &str) -> Vec<String> {
    let mut names = Vec::new();
    for raw_name in flag_part.split(',') {
        let mut pieces = raw_name.split_whitespace().filter(|x| !x.is_empty());
        if let Some(name) = pieces.next()
            && name.starts_with('-')
        {
            names.push(name.to_string());
        }
    }
    names
}

fn format_completion_option(name: &str, desc: &str) -> String {
    let desc = desc.replace('\'', "'\"'\"'");
    format!("{}[{}]", name, desc)
}

fn zsh_completions(opts: &getopts::Options, usage: &str, command_path: &str) -> String {
    let opts = zsh_completion_entries(opts, usage);
    let command = command_path
        .rsplit('/')
        .next()
        .unwrap_or(command_path)
        .to_string();
    let mut function: String = command
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    if function.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        function.insert(0, '_');
    }

    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    for opt in opts {
        if seen.insert(opt.clone()) {
            entries.push(format!("        '{opt}'"));
        }
    }

    let mut lines = vec![format!("#compdef {command}"), format!("_{function}() {{")];
    if entries.is_empty() {
        lines.push("    _arguments".to_string());
    } else {
        lines.push("    _arguments -s \\".to_string());
        for (i, entry) in entries.iter().enumerate() {
            let continuation = if i + 1 == entries.len() { "" } else { " \\" };
            lines.push(format!("{entry}{continuation}"));
        }
    }
    lines.extend_from_slice(&["}".to_string(), format!("compdef _{function} {command}")]);
    lines.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zsh_completions_have_help_and_completions() {
        let mut opts = getopts::Options::new();
        opts.optflag("h", "help", "Print this help menu.");
        opts.optflag(
            "",
            "completions",
            "Print zsh completion script for this command.",
        );
        let script = zsh_completions(&opts, "Usage: test", "test-binary");
        assert!(script.contains("#compdef test-binary"));
        assert!(script.contains("_test_binary()"));
        assert!(script.contains("'--help[Print this help menu.]'"));
        assert!(script.contains("'--completions[Print zsh completion script for this command.]'"));
    }

    #[test]
    fn zsh_completion_entries_include_short_and_long_names() {
        let mut opts = getopts::Options::new();
        opts.optflag("h", "help", "Print this help menu.");
        opts.optflag("v", "verbose", "Print verbose output.");
        let entries = zsh_completion_entries(&opts, "Usage: test");

        assert!(entries.contains(&"-h[Print this help menu.]".to_string()));
        assert!(entries.contains(&"--help[Print this help menu.]".to_string()));
        assert!(entries.contains(&"-v[Print verbose output.]".to_string()));
        assert!(entries.contains(&"--verbose[Print verbose output.]".to_string()));
    }

    #[test]
    fn zsh_completion_entries_escapes_single_quotes() {
        let mut opts = getopts::Options::new();
        opts.optflag("", "with-space", "It'll parse correctly.");
        let entries = zsh_completion_entries(&opts, "Usage: test");

        assert!(entries.contains(&"--with-space[It'\"'\"'ll parse correctly.]".to_string()));
    }

    #[test]
    fn zsh_completions_normalize_function_name() {
        let mut opts = getopts::Options::new();
        opts.optflag("h", "help", "Print this help menu.");
        let script = zsh_completions(&opts, "Usage: test", "./bin/my-cmd");

        assert!(script.contains("#compdef my-cmd"));
        assert!(script.contains("_my_cmd()"));
        assert!(script.contains("compdef _my_cmd my-cmd"));
    }

    #[derive(Default, Eq, PartialEq)]
    struct TestCommandLine {
        required: String,
    }

    impl CommandLine for TestCommandLine {
        fn add_opts(&self, _prefix: Option<&str>, opts: &mut getopts::Options) {
            opts.reqopt("", "chooser-mode", "required mode", "METHOD");
        }

        fn matches(&mut self, _prefix: Option<&str>, matches: &getopts::Matches) {
            if let Some(mode) = matches.opt_str("chooser-mode") {
                self.required = mode;
            }
        }

        fn canonical_command_line(&self, _prefix: Option<&str>) -> Vec<String> {
            let mut result = vec!["test".to_string()];
            if !self.required.is_empty() {
                result.push("--chooser-mode".to_string());
                result.push(self.required.to_string());
            }
            result
        }
    }

    #[test]
    fn completions_bypass_missing_required_args() {
        let (command_line, _) = NoExitCommandLine::<TestCommandLine>::from_arguments_relaxed(
            "Usage: test",
            &["--completions"],
        );
        let (_, _, status) = command_line.into_parts();
        assert_eq!(status, 0);
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
