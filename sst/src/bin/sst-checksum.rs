use arrrg::CommandLine;
use arrrg_derive::CommandLine;

use sst::setsum::Setsum;
use sst::{Cursor, Sst, SstOptions};

fn fast_setsum(opts: SstOptions, sst: &str) -> String {
    let sst = Sst::new(opts, sst).expect("open Sst");
    sst.fast_setsum().hexdigest()
}

fn slow_setsum(opts: SstOptions, sst: &str) -> String {
    let sst = Sst::new(opts, sst).expect("open Sst");
    let mut cursor = sst.cursor();
    cursor.seek_to_first().expect("seek Sst");
    let mut setsum = Setsum::default();
    loop {
        cursor.next().expect("next");
        if let Some(kvr) = cursor.value() {
            match kvr.value {
                Some(v) => {
                    setsum.put(kvr.key, kvr.timestamp, v);
                }
                None => {
                    setsum.del(kvr.key, kvr.timestamp);
                }
            }
        } else {
            break;
        }
    }
    setsum.hexdigest()
}

#[derive(CommandLine, Debug, Default, Eq, PartialEq)]
struct SstChecksumOptions {
    #[arrrg(flag, "Report checksum from file footer rather than by computation.")]
    fast: bool,
    #[arrrg(nested)]
    sst: SstOptions,
}

fn main() {
    let (cmdline, args) =
        SstChecksumOptions::from_command_line("Usage: sst-checksum [OPTIONS] [SSTs]");
    for sst in args {
        if cmdline.fast {
            println!("{} {}", fast_setsum(cmdline.sst.clone(), &sst), sst);
        } else {
            println!("{} {}", slow_setsum(cmdline.sst.clone(), &sst), sst);
        }
    }
}
