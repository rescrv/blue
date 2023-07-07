use arrrg::CommandLine;

use lsmgraph::LsmOptions;

fn main() {
    let (options, free) = LsmOptions::from_command_line("USAGE: lsmgraph-compactions [OPTIONS]");
    if !free.is_empty() {
        eprintln!("command takes no arguments");
        std::process::exit(1);
    }
    let db = options.open().expect("opening graph");
    for compaction in db.compactions().expect("compacting SSTs") {
        for input in compaction.inputs {
            print!("{} ", input.setsum())
        }
        println!();
    }
}
