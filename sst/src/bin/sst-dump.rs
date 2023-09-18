use arrrg::CommandLine;

use sst::{Cursor, Sst, SstOptions};

fn main() {
    let (opts, args) = SstOptions::from_command_line("Usage: sst-dump [OPTIONS] [SSTs]");
    // parse
    for sst in args {
        let sst = Sst::new(opts.clone(), sst).expect("could not open sst");
        let mut cursor = sst.cursor();
        cursor.seek_to_first().expect("could not seek to first");
        cursor.next().expect("cursor::next");
        while cursor.value().is_some() {
            println!("{}", cursor.value().unwrap());
            cursor.next().expect("cursor::next");
        }
    }
}
