//! Truncate a log file that has the final-partial-frame corruption case present.

use arrrg::CommandLine;
use arrrg_derive::CommandLine;

use sst::{Error, IoToZ, LogOptions};

#[derive(CommandLine, Debug, Default, Eq, PartialEq)]
struct LogTruncateFinalPartialFrameOptions {
    #[arrrg(flag, "Perform the truncation (default: log the offset).")]
    truncate: bool,
    #[arrrg(optional, "Truncate to this length (requires: --truncate).")]
    truncate_to: Option<u64>,
    #[arrrg(nested)]
    log: LogOptions,
}

fn main() {
    let (cmdline, args) = LogTruncateFinalPartialFrameOptions::from_command_line(
        "Usage: log-truncate-final-partial-frame [OPTIONS] <SST>",
    );
    if args.len() != 1 {
        eprintln!("specify exactly one sst");
        std::process::exit(1);
    }
    if let Some(offset) = sst::log::truncate_final_partial_frame(cmdline.log.clone(), &args[0])
        .as_z()
        .pretty_unwrap()
    {
        if cmdline.truncate {
            if let Some(truncate_to) = cmdline.truncate_to {
                if offset != truncate_to || offset > i64::MAX as u64 {
                    eprintln!(
                        "not truncating: required offset={offset}, specified offset={truncate_to}"
                    );
                    std::process::exit(2);
                } else if unsafe { libc::truncate64(args[0].as_ptr() as *const i8, offset as i64) }
                    < 0
                {
                    Err::<(), Error>(std::io::Error::last_os_error().into())
                        .as_z()
                        .pretty_unwrap();
                }
            } else {
                eprintln!("not truncating: specify --truncate-to {offset} to truncate");
                std::process::exit(2);
            }
        } else {
            println!("truncate to {offset} bytes");
        }
    } else {
        println!("truncate cannot help");
    }
}
