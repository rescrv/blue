//! Pretty-print how the command-line is interpereted.

use arrrg::CommandLine;

use sst::SstOptions;

fn main() {
    let (options, free) = SstOptions::from_command_line("USAGE: sst-options [OPTIONS]");
    if !free.is_empty() {
        eprintln!("expected no positional arguments");
        std::process::exit(1);
    }
    println!("{options:#?}");
}
