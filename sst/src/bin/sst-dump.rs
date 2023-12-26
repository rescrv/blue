//! Dump key-value pairs from an sst to stdout.

use arrrg::CommandLine;
use keyvalint::Cursor;

use sst::{Sst, SstOptions};

fn main() {
    let (opts, args) = SstOptions::from_command_line("Usage: sst-dump [OPTIONS] [SSTs]");
    // parse
    for sst in args {
        let sst = Sst::new(opts.clone(), sst).expect("could not open sst");
        let mut cursor = sst.cursor();
        cursor.seek_to_first().expect("could not seek to first");
        cursor.next().expect("cursor::next");
        while cursor.value().is_some() {
            println!("{}", cursor.key_value().unwrap());
            cursor.next().expect("cursor::next");
        }
    }
}
