use std::path::PathBuf;

use arrrg::CommandLine;

use lsmtk::{IoToZ, ManifestVerifier};

#[derive(Clone, Debug, Default, Eq, PartialEq, arrrg_derive::CommandLine)]
pub struct Options {}

fn main() {
    let (_, free) = Options::from_command_line("USAGE: lsmtk-verify-manifest [MANIFEST]");
    let verifier = ManifestVerifier::open().as_z().pretty_unwrap();
    for mani in free.into_iter() {
        verifier.verify(&PathBuf::from(mani)).as_z().pretty_unwrap();
    }
}
