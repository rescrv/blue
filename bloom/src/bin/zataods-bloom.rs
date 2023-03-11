use clap::{App, Arg, ArgMatches};

use bloom::{
    calc_keys_given_probability, calc_m_given_p_n, calc_p_given_n_m, Parameter, K, M, N, P,
};

fn parse<X: Parameter>(args: &ArgMatches, name: &str) -> Option<X> {
    let x = match args.value_of(name) {
        Some(x) => x,
        None => {
            return None;
        }
    };
    Some(X::new(x.parse::<f64>().expect("could not parse value")))
}

fn main() {
    let app = App::new("zataods-bloom")
        .version("0.1.0")
        .about("compute bloom filter parameters");
    let app = app.arg(
        Arg::with_name("n")
            .long("n")
            .takes_value(true)
            .help("Expected number of elements to insert into the bloom filter."),
    );
    let app = app.arg(
        Arg::with_name("p")
            .long("p")
            .takes_value(true)
            .help("Probability of a false positive."),
    );
    let app = app.arg(
        Arg::with_name("m")
            .long("m")
            .takes_value(true)
            .help("Number of bits to provide the bloom filter."),
    );
    let app = app.arg(
        Arg::with_name("k")
            .long("k")
            .takes_value(true)
            .help("Number of hash functions/keys to set in the bloom filter."),
    );
    let args = app.get_matches();
    let n = parse::<N>(&args, "n");
    let p = parse::<P>(&args, "p");
    let m = parse::<M>(&args, "m");
    let k = parse::<K>(&args, "k");
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
            println!("provide --p to compute --k");
            println!("provide --p --n to compute --m");
            println!("provide --n --m to compute --p");
        }
    }
}
