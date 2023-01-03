use std::env::args;

use lp::lsm::{LSMOptions, LSMTree};

fn main() {
    let args: Vec<String> = args().collect();
    if args.len() < 2 {
        eprintln!("lp-lsm-sst-ingest [lsm tree location] [sst] ...");
        std::process::exit(-1);
    }
    let lsm = &args[1];
    let ssts = &args[2..];
    let opts = LSMOptions::default();
    let tree = LSMTree::open(opts, lsm).expect("could not open LSM tree");
    tree.ingest_ssts(ssts).expect("could not ingest SSTs");
}
