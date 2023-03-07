use std::fs::File;
use std::io::{BufRead, BufReader};

use clap::{App, Arg};

use util::time::now;

use lp::sst::{SSTBuilder, SSTBuilderOptions};
use lp::Builder;

fn main() {
    let app = App::new("zataods-lp-sst-from-plaintext")
        .version("0.1.0")
        .about("Convert a plaintext \"table\" to an SST.");
    let app = app.arg(
        Arg::with_name("input")
            .long("input")
            .takes_value(true)
            .help("Number of strings to generate."),
    );
    let app = app.arg(
        Arg::with_name("output")
            .long("output")
            .takes_value(true)
            .help("Name of the SST to create."),
    );

    // parse
    let args = app.get_matches();
    let input = args.value_of("input").unwrap_or("/dev/stdin");
    let output = args.value_of("output").unwrap_or("plaintext.sst");

    // setup fin
    let fin = File::open(input).expect("could not open input");
    let fin = BufReader::new(fin);
    // setup sst out
    let opts = SSTBuilderOptions::default();
    let mut sst = SSTBuilder::new(output, opts).expect("could not open sst");

    for line in fin.lines() {
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
