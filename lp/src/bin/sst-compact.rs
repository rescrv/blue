use clap::{App, Arg};

use sst::merging_cursor::MergingCursor;
use sst::{SST, SSTBuilderOptions};
use sst::Cursor;

use lp::cli::{sst_args, parse_sst_args};
use lp::db::CompactionOptions;
use lp::db::compaction::{losslessly_compact, Compaction};

fn main() {
    let app = App::new("zataods-lp-sst-compact")
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
    let app = sst_args(app, 1);

    // parse
    let args = app.get_matches();
    let sst_options = SSTBuilderOptions::default()
        .target_file_size(args.value_of("sst-size").unwrap_or("4194304").parse::<u32>().unwrap_or(4194304));
    let options = CompactionOptions {
        max_compaction_bytes: 1<<63,
        sst_options,
    };
    let output_prefix = args.value_of("output").unwrap_or("compacted_").to_string();
    let ssts: Vec<_> = parse_sst_args(&args);

    // compact
    let mut metadatas = Vec::new();
    let mut cursors: Vec<Box<dyn Cursor>> = Vec::new();
    for input in ssts {
        let sst = SST::new(input).expect("open sst");
        metadatas.push(sst.metadata().expect("sst metadata"));
        cursors.push(Box::new(sst.cursor()));
    }
    let compaction = Compaction::from_inputs(options, metadatas, 0, Vec::new());
    let cursor = MergingCursor::new(cursors).expect("compaction");
    losslessly_compact(cursor, compaction, output_prefix).expect("compaction");
}
