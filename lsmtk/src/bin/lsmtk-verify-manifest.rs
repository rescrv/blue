use std::path::PathBuf;

use arrrg::CommandLine;

use lsmtk::ManifestVerifier;

#[derive(Clone, Debug, Default, Eq, PartialEq, arrrg_derive::CommandLine)]
pub struct Options {}

fn main() {
    let (_, free) = Options::from_command_line("USAGE: lsmtk-verify-manifest [MANIFEST]");
    let verifier = ManifestVerifier::open().unwrap_or_else(|err| panic!("{err}"));
    for mani in free.into_iter() {
        verifier
            .verify(&PathBuf::from(mani))
            .unwrap_or_else(|err| panic!("{err}"));
    }
}
