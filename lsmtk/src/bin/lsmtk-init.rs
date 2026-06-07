use arrrg::CommandLine;

use lsmtk::{LsmTree, LsmtkOptions};

fn main() {
    let (options, free) = LsmtkOptions::from_command_line("USAGE: lsmtk-init [OPTIONS]");
    if !free.is_empty() {
        eprintln!("expected no positional arguments");
        std::process::exit(1);
    }
    LsmTree::open(options).unwrap_or_else(|err| panic!("{err}"));
}
