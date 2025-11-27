//! Outut the setsum associated with each log.

use arrrg::CommandLine;
use arrrg_derive::CommandLine;

use sst::setsum::Setsum;
use sst::{LogIterator, LogOptions};

fn setsum(opts: LogOptions, log: &str) -> String {
    let mut log = LogIterator::new(opts, log).expect("open log");
    let mut setsum = Setsum::default();
    while let Some(kvr) = log.next().unwrap() {
        match kvr.value {
            Some(v) => {
                setsum.put(kvr.key, kvr.timestamp, v);
            }
            None => {
                setsum.del(kvr.key, kvr.timestamp);
            }
        }
    }
    setsum.hexdigest()
}

#[derive(CommandLine, Debug, Default, Eq, PartialEq)]
struct LogChecksumOptions {
    #[arrrg(flag, "Report checksum from file footer rather than by computation.")]
    fast: bool,
    #[arrrg(nested)]
    log: LogOptions,
}

fn main() {
    let (cmdline, args) =
        LogChecksumOptions::from_command_line("Usage: log-checksum [OPTIONS] [SSTs]");
    for log in args {
        println!("{} {}", setsum(cmdline.log.clone(), &log), log);
    }
}
