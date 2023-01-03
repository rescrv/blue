use clap::{App, Arg};

use lp::lsm::{LSMOptions, LSMTree};

fn main() {
    let app = App::new("lp-lsm-debug")
        .version("0.1.0")
        .about("Ingest a set of SSTs into an LSM tree.");
    let app = app.arg(
        Arg::with_name("lsm")
            .long("lsm")
            .takes_value(true)
            .help("Path to the lsm tree."));

    // parse
    let args = app.get_matches();
    let tree = args.value_of("lsm").unwrap_or("lsm");

    let opts = LSMOptions::default();
    let tree = LSMTree::open(opts, tree).expect("could not open LSM tree");
    tree.debug_dump();
}
