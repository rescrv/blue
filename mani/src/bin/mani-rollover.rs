use arrrg::CommandLine;

use mani::{Manifest, ManifestOptions};

fn main() {
    let (options, roots) = ManifestOptions::from_command_line("USAGE: mani-rollover [OPTIONS] <root>");
    if roots.len() != 1 {
        eprintln!("must provide exactly one manifest root");
        std::process::exit(1);
    }
    let mut manifest = Manifest::open(options, &roots[0]).expect("could not open");
    manifest.rollover().expect("could not rollover manifest");
}
