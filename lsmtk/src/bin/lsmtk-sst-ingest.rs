use arrrg::CommandLine;

use lsmtk::{IoToZ, LsmOptions};

use std::path::PathBuf;

fn main() {
    let (options, free) = LsmOptions::from_command_line("USAGE: lsmtk-sst-ingest [OPTIONS] <sst> ...");
    let ssts: Vec<PathBuf> = free.into_iter().map(PathBuf::from).collect();
    let db = options.open().as_z().pretty_unwrap();
    db.ingest(&ssts).as_z().pretty_unwrap();
}
