//! Calculate bloom derivable filter parameters from known parameters.

use bloomcalc::{
    calc_keys_given_probability, calc_m_given_p_n, calc_p_given_n_m, Parameter, K, M, N, P,
};

use arrrg::CommandLine;
use arrrg_derive::CommandLine;

#[derive(CommandLine, Debug, Default, PartialEq)]
struct Parameters {
    #[arrrg(
        optional,
        "Expected number of elements to insert into the bloom filter.",
        "N"
    )]
    card: Option<f64>,
    #[arrrg(optional, "Expected probability of a false positive.", "P")]
    prob: Option<f64>,
    #[arrrg(optional, "Number of bits in use in the bloom filter.", "M")]
    bits: Option<f64>,
    #[arrrg(
        optional,
        "Number of hash functions/keys to set in the bloom filter.",
        "K"
    )]
    keys: Option<f64>,
}

impl Eq for Parameters {}

fn main() {
    let (params, free) = Parameters::from_command_line_relaxed(
        "Usage: bloom-calculator [--n N] [--p P] [--m M] [--k K]",
    );
    if !free.is_empty() {
        panic!("free arguments are not accepted");
    }

    let n = params.card.map(N::new);
    let p = params.prob.map(P::new);
    let m = params.bits.map(M::new);
    let k = params.keys.map(K::new);

    match (n, p, m, k) {
        (None, Some(p), None, None) => {
            println!(
                "a bloom filter with false positive rate of {} will work best with {} keys",
                p.0,
                calc_keys_given_probability(p).0
            );
        }
        (Some(n), Some(p), None, None) => {
            println!(
                "a bloom filter for {} items with false positive rate {} will need {} bits",
                n.0,
                p.0,
                calc_m_given_p_n(p, n).0
            );
        }
        (Some(n), None, Some(m), None) => {
            println!(
                "a bloom filter for {} items with {} bits will have a {} false positive rate",
                n.0,
                m.0,
                calc_p_given_n_m(n, m).0
            );
        }
        (_, _, _, _) => {
            println!("provide --prob to compute --keys");
            println!("provide --card --prob to compute --bits");
            println!("provide --card --bits to compute --prob");
        }
    }
}
