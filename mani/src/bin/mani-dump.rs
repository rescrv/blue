use arrrg::CommandLine;

use mani::{ManifestIterator, ManifestOptions};

fn main() {
    let (_, files) = ManifestOptions::from_command_line("USAGE: mani-dump [OPTIONS] [<file> ...]");
    for file in files {
        let iter = ManifestIterator::open(file).expect("could not open");
        for edit in iter {
            let edit = edit.expect("could not read edit");
            println!("{:#?}", edit);
        }
    }
}
