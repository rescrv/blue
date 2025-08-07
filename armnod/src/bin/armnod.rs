//! Generate random ASCII strings from the provided command-line options.

use std::io::{BufWriter, Write};

use arrrg::CommandLine;

use guacamole::Guacamole;

use armnod::ArmnodOptions;

const USAGE: &str = "Usage: armnod [--options]";

fn main() {
    let (cmdline, free) = ArmnodOptions::from_command_line(USAGE);
    if !free.is_empty() {
        eprintln!("free arguments are not accepted");
        eprintln!("{USAGE}");
        std::process::exit(1);
    }
    let mut armnod = match cmdline.try_parse() {
        Ok(armnod) => armnod,
        Err(err) => {
            eprintln!("invalid command line: {err}");
            eprintln!("{USAGE}");
            std::process::exit(1);
        }
    };
    let mut guac = Guacamole::default();
    let mut fout = BufWriter::new(std::io::stdout());
    while let Some(x) = armnod.choose(&mut guac) {
        writeln!(fout, "{x}").unwrap();
    }
}
