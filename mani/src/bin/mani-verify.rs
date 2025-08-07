//! Verify the manifest's properties.

use arrrg::CommandLine;

use mani::{Manifest, ManifestOptions};

fn main() {
    let (options, roots) =
        ManifestOptions::from_command_line("USAGE: mani-verify [OPTIONS] <root>");
    if roots.len() != 1 {
        eprintln!("must provide exactly one manifest root");
        std::process::exit(1);
    }
    for err in Manifest::verify(options, &roots[0]) {
        println!("{err}");
    }
}
