use arrrg::CommandLine;
use arrrg_derive::CommandLine;

use gremlins::{Harness, HarnessOptions};

#[derive(CommandLine, Default, Eq, PartialEq)]
struct Options {
    #[arrrg(nested)]
    harness: HarnessOptions,
}

fn main() {
    let (options, free) = Options::from_command_line("Usage: gremlin-example-harness [OPTIONS]");
    if !free.is_empty() {
        eprintln!("no positional arguments allowed");
        std::process::exit(1);
    }
    let harness = Harness::new(options.harness).expect("new harness");
    harness.serve();
}
