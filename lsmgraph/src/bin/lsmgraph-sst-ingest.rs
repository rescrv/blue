use arrrg::CommandLine;

use lsmgraph::LsmOptions;

use std::path::PathBuf;

fn main() {
    let (options, free) = LsmOptions::from_command_line("USAGE: lsmgraph-sst-ingest [OPTIONS] <sst> ...");
    let ssts: Vec<PathBuf> = free.into_iter().map(PathBuf::from).collect();
    let db = options.open().expect("opening graph");
    db.ingest(&ssts).expect("ingesting SSTs");
}
