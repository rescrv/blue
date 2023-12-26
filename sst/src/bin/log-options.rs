//! Pretty-print how the command-line is interpereted.

use arrrg::CommandLine;

use sst::LogOptions;

fn main() {
    let (options, free) = LogOptions::from_command_line("USAGE: log-options [OPTIONS]");
    if !free.is_empty() {
        eprintln!("expected no positional arguments");
        std::process::exit(1);
    }
    println!("{:#?}", options);
}
