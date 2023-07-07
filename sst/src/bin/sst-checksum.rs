use arrrg::CommandLine;
use arrrg_derive::CommandLine;

use sst::setsum::Setsum;
use sst::{Cursor, Sst};

fn fast_setsum(sst: &str) -> String {
    let sst = Sst::new(sst).expect("open Sst");
    sst.setsum().hexdigest()
}

fn slow_setsum(sst: &str) -> String {
    let sst = Sst::new(sst).expect("open Sst");
    let mut cursor = sst.cursor();
    cursor.seek_to_first().expect("seek Sst");
    let mut setsum = Setsum::default();
    loop {
        cursor.next().expect("next");
        if let Some(kvr) = cursor.value() {
            match kvr.value {
                Some(v) => { setsum.put(kvr.key, kvr.timestamp, v); },
                None => { setsum.del(kvr.key, kvr.timestamp); },
            }
        } else {
            break;
        }
    }
    setsum.hexdigest()
}

#[derive(CommandLine, Debug, Default, Eq, PartialEq)]
struct SstChecksumCommandLine {
    #[arrrg(flag, "Report checksum from file footer rather than by computation.")]
    fast: bool,
}

fn main() {
    let (cmdline, args) = SstChecksumCommandLine::from_command_line("Usage: sst-checksum [OPTIONS] [SSTs]");
    for sst in args {
        if cmdline.fast {
            println!("{} {}", fast_setsum(&sst), sst);
        } else {
            println!("{} {}", slow_setsum(&sst), sst);
        }
    }
}
