use arrrg::CommandLine;

use lsmgraph::LsmOptions;

fn main() {
    let (options, ssts) = LsmOptions::from_command_line("USAGE: lsmgraph-compaction [OPTIONS] <sst> ...");
    let db = options.open().expect("opening graph");
    db.compact(&ssts).expect("performing compaction");
}
