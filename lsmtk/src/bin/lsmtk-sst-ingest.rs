use arrrg::CommandLine;

use lsmtk::{IoToZ, LsmTree, LsmtkOptions};

use std::path::PathBuf;

fn main() {
    let (options, free) =
        LsmtkOptions::from_command_line("USAGE: lsmtk-sst-ingest [OPTIONS] <sst> ...");
    let ssts: Vec<PathBuf> = free.into_iter().map(PathBuf::from).collect();
    let tree = LsmTree::open(options).as_z().pretty_unwrap();
    for sst in ssts.into_iter() {
        tree.ingest(sst).as_z().pretty_unwrap();
    }
}
