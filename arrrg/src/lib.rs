use std::str::FromStr;

use getopts::Fail;

//////////////////////////////////////////// CommandLine ///////////////////////////////////////////

pub trait CommandLine: Sized + Default + Eq + PartialEq {
    fn add_opts(&self, prefix: Option<&str>, opts: &mut getopts::Options);
    fn matches(&mut self, prefix: Option<&str>, matches: &getopts::Matches);
    fn canonical_command_line(&self, prefix: Option<&str>) -> Vec<String>;

    fn from_command_line(usage: &str) -> (Self, Vec<String>) {
        let args: Vec<String> = std::env::args().collect();
        let args: Vec<&str> = args.iter().map(AsRef::as_ref).collect();
        Self::from_arguments(usage, &args[1..])
    }

    fn from_command_line_relaxed(usage: &str) -> (Self, Vec<String>) {
        let args: Vec<String> = std::env::args().collect();
        let args: Vec<&str> = args.iter().map(AsRef::as_ref).collect();
        Self::from_arguments_relaxed(usage, &args[1..])
    }

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

    fn usage(&mut self, opts: getopts::Options, brief: &str) -> ! {
        print!("{}", opts.usage(brief));
        std::process::exit(1);
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
