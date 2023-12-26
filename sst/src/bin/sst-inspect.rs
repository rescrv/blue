use arrrg::CommandLine;

use sst::{Sst, SstOptions};

fn main() {
    let (opts, args) = SstOptions::from_command_line("Usage: sst-dump [OPTIONS] [SSTs]");
    // parse
    for sst in args {
        let sst = Sst::new(opts.clone(), sst).expect("could not open sst");
        sst.inspect().expect("inspect should always succeed");
    }
}
