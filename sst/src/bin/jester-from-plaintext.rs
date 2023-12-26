//! Make a jester that converts files to ssts.

use std::fs::File;
use std::io::{BufRead, BufReader};

use arrrg::CommandLine;

use sst::ingest::{IngestOptions, Jester};
use sst::Builder;

#[derive(Clone, Debug, Default, Eq, PartialEq, arrrg_derive::CommandLine)]
struct JesterFromPlaintextOptions {
    #[arrrg(nested)]
    ingest: IngestOptions,
}

fn main() {
    let (opts, inputs) =
        JesterFromPlaintextOptions::from_command_line("Usage: jester-from-plaintext [OPTIONS]");
    let mut jester = Jester::new(opts.ingest.clone());
    for input in inputs.into_iter() {
        let plaintext = File::open(input).expect("could not open plaintext");
        let plaintext = BufReader::new(plaintext);
        for (idx, line) in plaintext.lines().enumerate() {
            let line = &line.expect("could not parse line");
            let split: Vec<&str> = line.split_whitespace().collect();
            if split.len() != 2 {
                panic!("Invalid line: {}", line);
            }
            jester
                .put(split[0].as_bytes(), idx as u64, split[1].as_bytes())
                .expect("could not put key-value pair");
        }
    }
    jester.seal().expect("could not seal jester");
}
