//! Cat the strings that are added and not subsequently removed.

use arrrg::CommandLine;

use mani::{Manifest, ManifestOptions};

fn main() {
    let (options, roots) = ManifestOptions::from_command_line("USAGE: mani-cat [OPTIONS] <root>");
    if roots.len() != 1 {
        eprintln!("must provide exactly one manifest root");
        std::process::exit(1);
    }
    let manifest = Manifest::open(options, &roots[0]).expect("could not open");
    for s in manifest.strs() {
        println!("{}", s);
    }
}
