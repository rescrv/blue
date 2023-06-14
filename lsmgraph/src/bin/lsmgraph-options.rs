use arrrg::CommandLine;

use lsmgraph::LsmOptions;

fn main() {
    let (options, free) = LsmOptions::from_command_line("USAGE: lsmgraph-options [OPTIONS]");
    if !free.is_empty() {
        eprintln!("expected no positional arguments");
        std::process::exit(1);
    }
    println!("{:#?}", options);
}