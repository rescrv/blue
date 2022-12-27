use clap::{App, Arg};

use std::io::Write;

use guacamole::Guacamole;

fn main() {
    let app = App::new("guacamole")
        .version("0.1.0")
        .about("generate pseudo-random, predictable bytes");
    let app = app.arg(
        Arg::with_name("n")
            .long("n")
            .takes_value(true)
            .help("Number of bytes to generate."),
    );
    let app = app.arg(
        Arg::with_name("seed")
            .long("seed")
            .takes_value(true)
            .help("Guacamole seed.")
    );
    let args = app.get_matches();
    let n = args.value_of("n").unwrap_or("1000");
    let mut n = n.parse::<u64>().expect("could not parse n") as usize;
    let seed = args.value_of("seed").unwrap_or("1000");
    let seed = seed.parse::<u64>().expect("could not parse seed");
    let mut guac = Guacamole::new(seed);
    let mut buf = [0u8; 1<<20];
    let buf: &mut [u8] = &mut buf;
    loop {
        let remain = if n > buf.len() {
            buf.len()
        } else {
            n
        };
        guac.generate(buf);
        std::io::stdout().write_all(buf).expect("failed to write");
        n -= remain;
        if remain < buf.len() {
            break;
        }
    }
}
