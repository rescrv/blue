use arrrg::CommandLine;

use sst::{LogIterator, LogOptions};

fn main() {
    let (opts, args) = LogOptions::from_command_line("Usage: log-dump [OPTIONS] [SSTs]");
    // parse
    for log in args {
        let mut log = LogIterator::new(opts.clone(), log).unwrap();
        while let Some(kvr) = log.next().unwrap() {
            match kvr.value {
                Some(v) => {
                    let key = String::from_utf8(kvr.key.iter().flat_map(|b| std::ascii::escape_default(*b)).collect::<Vec<u8>>()).unwrap();
                    let value = String::from_utf8(v.iter().flat_map(|b| std::ascii::escape_default(*b)).collect::<Vec<u8>>()).unwrap();
                    println!("\"{}\" @ {} -> \"{}\"", key, kvr.timestamp, value);
                },
                None => {
                    let key = String::from_utf8(kvr.key.iter().flat_map(|b| std::ascii::escape_default(*b)).collect::<Vec<u8>>()).unwrap();
                    println!("\"{}\" @ {} -> <TOMBSTONE>", key, kvr.timestamp);
                },
            }
        }
    }
}
