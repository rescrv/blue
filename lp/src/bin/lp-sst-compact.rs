use clap::{App, Arg};

use lp::db::compaction::losslessly_compact;
use lp::sst::SSTBuilderOptions;

fn main() {
    let app = App::new("lp-sst-compact")
        .version("0.1.0")
        .about("Compact the listed SSTs to a new set of SSTs without dropping anything.");
    let app = app.arg(
        Arg::with_name("output")
            .long("output-prefix")
            .takes_value(true)
            .help("Output all tables with this prefix."));
    let app = app.arg(
        Arg::with_name("sst-size")
            .long("sst-size")
            .takes_value(true)
            .help("Output file size (not a limit; creates next file when size exceeded)."));
    let app = app.arg(
        Arg::with_name("ssts")
            .index(1)
            .multiple(true)
            .help("List of ssts to compact."));

    // parse
    let args = app.get_matches();
    let options = SSTBuilderOptions::default()
        .target_file_size(args.value_of("sst-size").unwrap_or("4194304").parse::<u32>().unwrap_or(4194304));
    let output_prefix = args.value_of("output").unwrap_or("compacted_").to_string();
    let ssts: Vec<_> = args.values_of("ssts").unwrap().collect();
    losslessly_compact(options, output_prefix, ssts).expect("compaction");
}
