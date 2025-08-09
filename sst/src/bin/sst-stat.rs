//! Show the metadata for each sst listed on the command-line.

use arrrg::CommandLine;
use arrrg_derive::CommandLine;

use sst::file_manager::FileHandle;
use sst::{Sst, SstOptions};

#[derive(CommandLine, Debug, Default, Eq, PartialEq)]
struct SstStatOptions {
    #[arrrg(nested)]
    sst: SstOptions,
}

fn main() {
    let (cmdline, args) = SstStatOptions::from_command_line("Usage: sst-stat [OPTIONS] [SSTs]");
    for path in args {
        let sst = Sst::<FileHandle>::new(cmdline.sst.clone(), &path).expect("sst should open");
        println!(
            "{} size={} metadata={:?}",
            path,
            sst.approximate_size(),
            sst.metadata().expect("metadata call should succeed")
        );
    }
}
