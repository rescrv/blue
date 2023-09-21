use arrrg::CommandLine;

use lsmtk::{IoToZ, LsmOptions};

fn main() {
    let (options, free) = LsmOptions::from_command_line("USAGE: lsmtk-compactions [OPTIONS]");
    if !free.is_empty() {
        eprintln!("command takes no arguments");
        std::process::exit(1);
    }
    let db = options.open().as_z().pretty_unwrap();
    for compaction in db.compactions().as_z().pretty_unwrap() {
        for input in compaction.inputs {
            print!("{} ", input.setsum())
        }
        println!();
    }
}
