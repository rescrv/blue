use std::fs::File;
use std::io::{BufRead, BufReader};

use arrrg::CommandLine;
use arrrg_derive::CommandLine;

use sst::{Builder, LogBuilder, LogOptions};

#[derive(CommandLine, Debug, Eq, PartialEq)]
struct LogFromPlaintextOptions {
    #[arrrg(required, "Input file in plaintext \"<KEY> <VALUE>\\n\" formatting.")]
    plaintext: String,
    #[arrrg(required, "Output file in log format.")]
    output: String,
    #[arrrg(nested)]
    log: LogOptions,
}

impl Default for LogFromPlaintextOptions {
    fn default() -> Self {
        Self {
            plaintext: "/dev/stdin".to_string(),
            output: "plaintext.log".to_string(),
            log: LogOptions::default(),
        }
    }
}

fn main() {
    let (cmdline, _) = LogFromPlaintextOptions::from_command_line("Usage: log-from-plaintext --plaintext <FILE> --log <FILE>");
    // setup fin
    let plaintext = File::open(cmdline.plaintext).expect("could not open plaintext");
    let plaintext = BufReader::new(plaintext);
    // setup log out
    let mut log = LogBuilder::new(cmdline.log, cmdline.output).expect("could not open log");

    for (idx, line) in plaintext.lines().enumerate() {
        let line = &line.expect("could not parse line");
        let split: Vec<&str> = line.split_whitespace().collect();
        if split.len() != 2 {
            panic!("Invalid line: {}", line);
        }
        log.put(split[0].as_bytes(), idx as u64, split[1].as_bytes())
            .expect("could not put key-value pair");
    }

    log.seal().expect("could not seal the log");
}
