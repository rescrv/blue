use arrrg::CommandLine;

use lsmtk::{LsmTree, LsmtkOptions};

use std::path::PathBuf;

fn main() {
    let (options, free) =
        LsmtkOptions::from_command_line("USAGE: lsmtk-sst-ingest [OPTIONS] <sst> ...");
    let ssts: Vec<PathBuf> = free.into_iter().map(PathBuf::from).collect();
    let tree = LsmTree::open(options).unwrap_or_else(|err| panic!("{err}"));
    for sst in ssts.into_iter() {
        tree.ingest(sst).unwrap_or_else(|err| panic!("{err}"));
    }
}
