use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::{SystemTime, UNIX_EPOCH};

use arrrg::CommandLine;
use arrrg_derive::CommandLine;

use sst::{Builder, SstBuilder, SstOptions};


#[derive(CommandLine, Debug, Eq, PartialEq)]
struct SstFromPlaintextOptions {
    #[arrrg(required, "Input file in plaintext \"<KEY> <VALUE>\\n\" formatting.")]
    plaintext: String,
    #[arrrg(required, "Output file in SST format.")]
    output: String,
    #[arrrg(flag, "Use now::micros() instead of line number for timestamps")]
    timestamp: bool,
    #[arrrg(nested)]
    sst: SstOptions,
}

impl Default for SstFromPlaintextOptions {
    fn default() -> Self {
        Self {
            plaintext: "/dev/stdin".to_string(),
            output: "plaintext.sst".to_string(),
            timestamp: false,
            sst: SstOptions::default(),
        }
    }
}

fn main() {
    let (cmdline, _) = SstFromPlaintextOptions::from_command_line("Usage: sst-from-plaintext --plaintext <FILE> --output <FILE>");
    // setup fin
    let plaintext = File::open(cmdline.plaintext).expect("could not open plaintext");
    let plaintext = BufReader::new(plaintext);
    // setup sst out
    let mut sst = SstBuilder::new(cmdline.sst, cmdline.output).expect("could not open sst");

    for (idx, line) in plaintext.lines().enumerate() {
        let line = &line.expect("could not parse line");
        let split: Vec<&str> = line.split_whitespace().collect();
        if split.len() != 2 {
            panic!("Invalid line: {}", line);
        }
        let ts = if cmdline.timestamp {
            (SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time before UNIX epoch")
                .as_secs_f64()
                * 1_000_000.0) as u64
        } else {
            idx as u64
        };
        sst.put(split[0].as_bytes(), ts, split[1].as_bytes())
            .expect("could not put key-value pair");
    }

    sst.seal().expect("could not seal the sst");
}
