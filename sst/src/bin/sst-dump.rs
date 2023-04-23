use clap::{App, Arg};

use sst::{Cursor, SST};

fn main() {
    let app = App::new("zataods-lp-sst-dump")
        .version("0.1.0")
        .about("Dump an SST to stdout using escaped strings.");
    let app = app.arg(
        Arg::with_name("sst")
            .index(1)
            .multiple(true)
            .help("List of ssts to dump."));

    // parse
    let args = app.get_matches();
    let ssts: Vec<_> = args.values_of("sst").unwrap().collect();
    for sst in ssts {
        let sst = SST::new(sst).expect("could not open sst");
        let mut cursor = sst.cursor();
        cursor.seek_to_first().expect("could not seek to first");
        cursor.next().expect("cursor::next");
        while cursor.value().is_some() {
            let kvr = cursor.value().unwrap();
            if kvr.value.is_some() {
                let key = String::from_utf8(kvr.key.iter().flat_map(|b| std::ascii::escape_default(*b)).collect::<Vec<u8>>()).unwrap();
                let value = String::from_utf8(kvr.value.unwrap().iter().flat_map(|b| std::ascii::escape_default(*b)).collect::<Vec<u8>>()).unwrap();
                println!("\"{}\" @ {} -> \"{}\"", key, kvr.timestamp, value);
            } else {
                let key = String::from_utf8(kvr.key.iter().flat_map(|b| std::ascii::escape_default(*b)).collect::<Vec<u8>>()).unwrap();
                println!("\"{}\" @ {} -> <TOMBSTONE>", key, kvr.timestamp);
            }
            cursor.next().expect("cursor::next");
        }
    }
}
