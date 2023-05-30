use std::str::FromStr;

//////////////////////////////////////////// CommandLine ///////////////////////////////////////////

pub trait CommandLine: Sized + Default {
    fn add_opts(&self, prefix: Option<&str>, opts: &mut getopts::Options);
    fn matches(&mut self, prefix: Option<&str>, matches: &getopts::Matches);
    fn canonical_command_line(&self, prefix: Option<&str>) -> Vec<String>;

    fn from_command_line() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let args: Vec<&str> = args.iter().map(AsRef::as_ref).collect();
        Self::from_arguments(&args[1..])
    }

    fn from_command_line_relaxed() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let args: Vec<&str> = args.iter().map(AsRef::as_ref).collect();
        Self::from_arguments_relaxed(&args[1..])
    }

    fn from_arguments(args: &[&str]) -> Self {
        let command_line = Self::from_arguments_relaxed(args);
        let reconstructed_args = command_line.canonical_command_line(None);
        if args != reconstructed_args {
            panic!("non-canonical commandline specified:
provided: {:?}
expected: {:?}
check argument order amongst other differences",
                   &args, reconstructed_args);
        }
        command_line
    }

    fn from_arguments_relaxed(args: &[&str]) -> Self {
        let mut command_line = Self::default();
        let mut opts = getopts::Options::new();
        opts.parsing_style(getopts::ParsingStyle::StopAtFirstFree);
        opts.long_only(true);
        command_line.add_opts(None, &mut opts);

        let matches = match opts.parse(args) {
            Ok(matches) => { matches },
            Err(err) => {
                panic!("could not parse command line: {}", err);
            }
        };
        command_line.matches(None, &matches);

        command_line
    }
}

//////////////////////////////////////////// macro utils ///////////////////////////////////////////

pub fn getopt_str(prefix: Option<&str>, field_arg: &str) -> String {
    match prefix {
        Some(prefix) => { format!("{}-{}", prefix, field_arg) },
        None => { field_arg.to_string() },
    }
}

pub fn dashed_str(prefix: Option<&str>, field_arg: &str) -> String {
    format!("--{}", getopt_str(prefix, field_arg))
}

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
