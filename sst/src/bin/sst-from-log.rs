//! Convert a log file to sst.

use arrrg::CommandLine;
use arrrg_derive::CommandLine;

use sst::log::log_to_builder;
use sst::{LogOptions, SstBuilder, SstOptions};

#[derive(CommandLine, Debug, Eq, PartialEq)]
struct SstFromLogOptions {
    #[arrrg(required, "Input file in log formatting.")]
    input: String,
    #[arrrg(nested)]
    log: LogOptions,
    #[arrrg(required, "Output file in SST format.")]
    output: String,
    #[arrrg(nested)]
    sst: SstOptions,
}

impl Default for SstFromLogOptions {
    fn default() -> Self {
        Self {
            input: "table.log".to_string(),
            log: LogOptions::default(),
            output: "table.sst".to_string(),
            sst: SstOptions::default(),
        }
    }
}

fn main() {
    let (cmdline, _) = SstFromLogOptions::from_command_line(
        "Usage: sst-from-plaintext --plaintext <FILE> --sst <FILE>",
    );
    let sst = SstBuilder::new(cmdline.sst, cmdline.output).expect("could not open sst");
    log_to_builder(cmdline.log, cmdline.input, sst).expect("could not translate log to sst");
}
