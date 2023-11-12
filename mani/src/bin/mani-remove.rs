use arrrg::CommandLine;

use mani::{Edit, Manifest, ManifestOptions};

fn main() {
    let (options, roots) =
        ManifestOptions::from_command_line("USAGE: mani-remove [OPTIONS] <root>");
    if roots.len() != 1 {
        eprintln!("must provide exactly one manifest root");
        std::process::exit(1);
    }
    let mut manifest = Manifest::open(options, &roots[0]).expect("could not open");
    let mut edit = Edit::default();
    for line in std::io::stdin().lines() {
        let line = line.expect("could not parse line on stdin");
        edit.rm(&line).expect("could not rm path");
    }
    manifest.apply(edit).expect("could not apply edit");
}
