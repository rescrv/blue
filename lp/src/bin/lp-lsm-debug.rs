use std::env::args;

use lp::lsm::{LSMOptions, LSMTree};

fn main() {
    let args: Vec<String> = args().collect();
    if args.len() != 2 {
        eprintln!("lp-lsm-debug [lsm tree location]");
        std::process::exit(-1);
    }
    let opts = LSMOptions::default();
    let tree = LSMTree::open(opts, args[1].clone()).expect("could not open LSM tree");
    tree.debug_dump();
}
