use arrrg::CommandLine;
use arrrg_derive::CommandLine;

use sst::{Cursor, Sst};

#[derive(CommandLine, Debug, Default, Eq, PartialEq)]
struct SstDumpCommandLine {}

fn main() {
    let (_, args) = SstDumpCommandLine::from_command_line("Usage: sst-dump [OPTIONS] [SSTs]");
    // parse
    for sst in args {
        let sst = Sst::new(sst).expect("could not open sst");
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
