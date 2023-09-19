use arrrg::CommandLine;
use arrrg_derive::CommandLine;

use guacamole::Zipf;
use guacamole::Guacamole;

#[derive(CommandLine, PartialEq)]
struct ZipfOptions {
    #[arrrg(required, "Approximate cardinality of the set.", "N")]
    card: u64,
    #[arrrg(optional, "Alpha value for the zipf distribution.")]
    alpha: Option<f64>,
    #[arrrg(optional, "Theta value for the zipf distribution.")]
    theta: Option<f64>,
    #[arrrg(optional, "Guacamole seed.")]
    seed: Option<u64>,
}

impl Default for ZipfOptions {
    fn default() -> Self {
        Self {
            card: 1000,
            alpha: None,
            theta: Some(0.99),
            seed: None,
        }
    }
}

impl Eq for ZipfOptions {}

/// Choose numbers [0, n) from a zipf distribution.
fn main() {
    let (cmdline, free) =
        ZipfOptions::from_command_line("Usage: zipf [--alpha ALPHA|--theta THETA] [OPTIONS]");
    if !free.is_empty() {
        panic!("free arguments are not accepted");
    }
    if cmdline.alpha.is_none() && cmdline.theta.is_none() {
        panic!("provide at least one of --alpha or --theta");
    }
    if cmdline.alpha.is_some() && cmdline.theta.is_some() {
        panic!("provide at most one of --alpha or --theta");
    }
    let zipf = if let Some(alpha) = cmdline.alpha {
        Zipf::from_alpha(cmdline.card, alpha)
    } else if let Some(theta) = cmdline.theta {
        Zipf::from_theta(cmdline.card, theta)
    } else {
        unreachable!();
    };
    let mut guac = Guacamole::new(cmdline.seed.unwrap_or(0));
    loop {
        let x = zipf.next(&mut guac);
        println!("{}", x);
    }
}
