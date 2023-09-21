use arrrg::CommandLine;

use lsmtk::{IoToZ, LsmOptions};

fn main() {
    let (options, free) = LsmOptions::from_command_line("USAGE: lsmtk-init [OPTIONS] <db>");
    if !free.is_empty() {
        eprintln!("expected no positional arguments");
        std::process::exit(1);
    }
    options.open().as_z().pretty_unwrap();
}
