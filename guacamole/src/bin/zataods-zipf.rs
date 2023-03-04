use clap::{App, Arg};

use guacamole::Guacamole;
use guacamole::zipf::Zipf;


/// Choose numbers [0, n) from a zipf distribution.
fn main() {
    let app = App::new("zataods-zipf")
        .version("0.1.0")
        .about("Choose numbers [0, n) from a zipf distribution.");
    let app = app.arg(
        Arg::with_name("n")
            .long("n")
            .takes_value(true)
            .help("Approximate number of items."),
    );
    let app = app.arg(
        Arg::with_name("alpha")
            .long("alpha")
            .takes_value(true)
            .help("Alpha value for the zipf distribution.")
    );
    let app = app.arg(
        Arg::with_name("theta")
            .long("theta")
            .takes_value(true)
            .help("Theta value for the zipf distribution.")
    );
    let app = app.arg(
        Arg::with_name("seed")
            .long("seed")
            .takes_value(true)
            .help("Guacamole seed.")
    );
    let args = app.get_matches();
    let n = args.value_of("n").unwrap_or("1000");
    let n = n.parse::<u64>().expect("could not parse n");
    let zipf = match args.value_of("theta") {
        Some(x) => {
            let theta = x.parse::<f64>().expect("could not parse theta");
            Zipf::from_theta(n, theta)
        },
        None => {
            match args.value_of("alpha") {
                Some(x) => {
                    let alpha = x.parse::<f64>().expect("could not parse alpha");
                    Zipf::from_alpha(n, alpha)
                },
                None => {
                    let theta = 0.5;
                    Zipf::from_theta(n, theta)
                }
            }
        },
    };
    let seed = args.value_of("seed").unwrap_or("0");
    let seed = seed.parse::<u64>().expect("could not parse seed");
    let mut guac = Guacamole::new(seed);
    loop {
        let x = zipf.next(&mut guac);
        println!("{}", x);
    }
}
