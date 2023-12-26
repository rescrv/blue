use rocksdb::DB;

use arrrg::CommandLine;
use keyvalint::rocksdb::KeyValueStore;

use keyvalint_bench::workload;

const USAGE: &str =
    "USAGE: keyvalint-bench-rocksdb [--rocksdb-options] workload [--workload-options]";

////////////////////////////////////////// RocksDbOptions //////////////////////////////////////////

#[derive(Debug, Default, Eq, PartialEq, arrrg_derive::CommandLine)]
struct RocksDbOptions {
    #[arrrg(required, "Path to the database.")]
    path: String,
}

/////////////////////////////////////////////// main ///////////////////////////////////////////////

fn main() {
    let (options, free) = RocksDbOptions::from_command_line_relaxed(USAGE);
    if free.is_empty() {
        eprintln!("missing workload");
        eprintln!("{}", USAGE);
        std::process::exit(1);
    }
    let db = DB::open_default(options.path).expect("db should open");
    let kvs = KeyValueStore::from(db);
    let mut workload = workload::from_command_line(USAGE, &free);
    workload.run(kvs);
}
