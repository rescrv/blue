use arrrg::CommandLine;

use lsmtk::LsmOptions;

fn main() {
    let (options, ssts) = LsmOptions::from_command_line("USAGE: lsmtk-compaction [OPTIONS] <sst> ...");
    let db = options.open().expect("opening graph");
    db.compact(&ssts).expect("performing compaction");
}
