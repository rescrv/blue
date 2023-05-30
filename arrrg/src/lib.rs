use std::str::FromStr;

//////////////////////////////////////////// CommandLine ///////////////////////////////////////////

pub trait CommandLine {
    fn add_opts(&self, prefix: Option<&str>, opts: &mut getopts::Options);
    fn matches(&mut self, prefix: Option<&str>, matches: &getopts::Matches);
    fn canonical_command_line(&self, prefix: Option<&str>) -> Vec<String>;
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
