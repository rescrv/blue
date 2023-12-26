use arrrg::CommandLine;

use lsmtk::{IoToZ, LsmTree, LsmtkOptions};

fn main() {
    let (options, free) = LsmtkOptions::from_command_line("USAGE: lsmtk-init [OPTIONS]");
    if !free.is_empty() {
        eprintln!("expected no positional arguments");
        std::process::exit(1);
    }
    LsmTree::open(options).as_z().pretty_unwrap();
}
