use std::fs::File;
use std::io::{BufRead, BufReader};

use util::time::now;

use arrrg::CommandLine;
use arrrg_derive::CommandLine;

use sst::{Builder, SSTBuilder, SSTBuilderOptions};

#[derive(CommandLine, Debug)]
struct SstFromPlaintextCommandLine {
    #[arrrg(required, "Input file in plaintext \"<KEY> <VALUE>\\n\" formatting.")]
    plaintext: String,
    #[arrrg(required, "Output file in SST format.")]
    sst: String,
}

impl Default for SstFromPlaintextCommandLine {
    fn default() -> Self {
        Self {
            plaintext: "/dev/stdin".to_string(),
            sst: "plaintext.sst".to_string(),
        }
    }
}

fn main() {
    let (cmdline, _) = SstFromPlaintextCommandLine::from_command_line();
    // setup fin
    let plaintext = File::open(cmdline.plaintext).expect("could not open plaintext");
    let plaintext = BufReader::new(plaintext);
    // setup sst out
    let opts = SSTBuilderOptions::default();
    let mut sst = SSTBuilder::new(cmdline.sst, opts).expect("could not open sst");

    for line in plaintext.lines() {
        let line = &line.expect("could not parse line");
        let split: Vec<&str> = line.split_whitespace().collect();
        if split.len() != 2 {
            panic!("Invalid line: {}", line);
        }
        sst.put(split[0].as_bytes(), now::micros(), split[1].as_bytes())
            .expect("could not put key-value pair");
    }

    sst.seal().expect("could not seal the sst");
}
