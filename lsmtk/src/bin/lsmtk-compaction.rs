use arrrg::CommandLine;

use lsmtk::{IoToZ, LsmOptions};

fn main() {
    let (options, ssts) =
        LsmOptions::from_command_line("USAGE: lsmtk-compaction [OPTIONS] <sst> ...");
    let db = options.open().as_z().pretty_unwrap();
    db.compact(&ssts).as_z().pretty_unwrap();
}
