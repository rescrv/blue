//! Choose numbers [0, n) from a zipf distribution.

use arrrg::CommandLine;
use arrrg_derive::CommandLine;

use guacamole::Guacamole;
use guacamole::Zipf;

#[derive(CommandLine, PartialEq)]
struct ZipfOptions {
    #[arrrg(required, "Approximate cardinality of the set.", "N")]
    card: u64,
    #[arrrg(optional, "Skew value for the zipf distribution (0.0, 1.0).")]
    skew: Option<f64>,
    #[arrrg(optional, "Guacamole seed.")]
    seed: Option<u64>,
}

impl Default for ZipfOptions {
    fn default() -> Self {
        Self {
            card: 1000,
            skew: Some(0.99),
            seed: None,
        }
    }
}

impl Eq for ZipfOptions {}

fn main() {
    let (cmdline, free) = ZipfOptions::from_command_line("Usage: zipf [--skew SKEW] [OPTIONS]");
    if !free.is_empty() {
        panic!("free arguments are not accepted");
    }
    let skew = cmdline.skew.unwrap_or(0.99);
    let zipf = Zipf::from_param(cmdline.card, skew);
    let mut guac = Guacamole::new(cmdline.seed.unwrap_or(0));
    loop {
        let x = zipf.next(&mut guac);
        println!("{}", x);
    }
}
