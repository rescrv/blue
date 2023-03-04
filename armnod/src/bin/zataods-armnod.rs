use std::io::{BufWriter, Write};

use clap::{App, Arg};

use guacamole::Guacamole;

use armnod::{ARMNOD, LengthChooser, SeedChooser};

fn random_chooser() -> Box<dyn SeedChooser> {
    Box::<armnod::RandomStringChooser>::default()
}

fn set_chooser(cardinality: u64) -> Box<dyn SeedChooser> {
    Box::new(armnod::SetStringChooser::new(cardinality))
}

fn set_chooser_once(begin: u64, end: u64) -> Box<dyn SeedChooser> {
    Box::new(armnod::SetStringChooserOnce::new(begin, end))
}

fn set_chooser_zipf(cardinality: u64, theta: f64) -> Box<dyn SeedChooser> {
    Box::new(armnod::SetStringChooserZipf::from_theta(cardinality, theta))
}

fn constant_length_chooser(length: u32) -> Box<dyn LengthChooser> {
    Box::new(armnod::ConstantLengthChooser::new(length))
}

fn uniform_length_chooser(min_length: u32, max_length: u32) -> Box<dyn LengthChooser> {
    Box::new(armnod::UniformLengthChooser::new(min_length, max_length))
}

fn main() {
    let app = App::new("armnod")
        .version("0.1.0")
        .about("Generate pseudo-random, predictable strings.");
    let app = app.arg(
        Arg::with_name("n")
            .long("n")
            .takes_value(true)
            .help("Number of strings to generate."),
    );
    // seed mode
    let app = app.arg(
        Arg::with_name("chooser-mode")
            .long("chooser-mode")
            .takes_value(true)
            .help("Armnod string chooser.")
    );
    let app = app.arg(
        Arg::with_name("cardinality")
            .long("cardinality")
            .takes_value(true)
            .help("Number of set elements for set-based modes.")
    );
    let app = app.arg(
        Arg::with_name("set-once-begin")
            .long("set-once-begin")
            .takes_value(true)
            .help("First set element to load in set-once mode.")
    );
    let app = app.arg(
        Arg::with_name("set-once-end")
            .long("set-once-end")
            .takes_value(true)
            .help("One past the last element to load in set-once mode.")
    );
    let app = app.arg(
        Arg::with_name("theta")
            .long("theta")
            .takes_value(true)
            .help("Theta parameter for set-once-zipf.")
    );
    // length mode
    let app = app.arg(
        Arg::with_name("length-mode")
            .long("length-mode")
            .takes_value(true)
            .help("Length chooser mode.")
    );
    let app = app.arg(
        Arg::with_name("string-length")
            .long("string-length")
            .takes_value(true)
            .help("Constant string length.")
    );
    let app = app.arg(
        Arg::with_name("string-min-length")
            .long("string-min-length")
            .takes_value(true)
            .help("Uniform minimum string length.")
    );
    let app = app.arg(
        Arg::with_name("string-max-length")
            .long("string-max-length")
            .takes_value(true)
            .help("Uniform maximum string length.")
    );
    // charset
    let app = app.arg(
        Arg::with_name("charset")
            .long("charset")
            .takes_value(true)
            .help("Charset to use {lower, upper, alpha, digit, alnum, punct, hex, default}.")
    );

    // parse
    let args = app.get_matches();
    // parse n first so that it can be altered by cardinality
    let n = args.value_of("n").unwrap_or("1000");
    let mut n = n.parse::<u64>().expect("could not parse n");
    // seeding
    let chooser = args.value_of("chooser-mode").unwrap_or("random");
    let string_chooser = if chooser == "random" {
        random_chooser()
    } else if chooser == "set" {
        let cardinality = args.value_of("cardinality").unwrap_or("1000");
        let cardinality = cardinality.parse::<u64>().expect("could not parse cardinality");
        set_chooser(cardinality)
    } else if chooser == "set-once" {
        let cardinality_default = format!("{}", n);
        let cardinality = args.value_of("cardinality").unwrap_or(&cardinality_default);
        let cardinality = cardinality.parse::<u64>().expect("could not parse cardinality");
        let cardinality_default = format!("{}", cardinality);
        let set_once_begin = args.value_of("set-once-begin").unwrap_or("0");
        let set_once_begin = set_once_begin.parse::<u64>().expect("could not parse set-once-begin");
        let set_once_end = args.value_of("set-once-end").unwrap_or(&cardinality_default);
        let set_once_end = set_once_end.parse::<u64>().expect("could not parse set-once-end");
        n = set_once_end - set_once_begin;
        assert!(set_once_begin <= set_once_end, "begin must be <= than end");
        set_chooser_once(set_once_begin, set_once_end)
    } else if chooser == "set-zipf" {
        let cardinality = args.value_of("cardinality").unwrap_or("1000");
        let cardinality = cardinality.parse::<u64>().expect("could not parse cardinality");
        let theta = args.value_of("theta").unwrap_or("0.99");
        let theta = theta.parse::<f64>().expect("could not parse theta");
        set_chooser_zipf(cardinality, theta)
    } else {
        eprintln!("unknown chooser mode {}", chooser);
        std::process::exit(1);
    };
    // length chooser
    let length = args.value_of("length-mode").unwrap_or("constant");
    let length_chooser = if length == "constant" {
        let length = args.value_of("string-length").unwrap_or("8");
        let length = length.parse::<u32>().expect("could not parse string string-length");
        constant_length_chooser(length)
    } else if length == "uniform" {
        let string_min_length = args.value_of("string-min-length").unwrap_or("8");
        let string_min_length = string_min_length.parse::<u32>().expect("could not parse string string-min-length");
        let string_max_length = args.value_of("string-max-length").unwrap_or("16");
        let string_max_length = string_max_length.parse::<u32>().expect("could not parse string string-max-length");
        uniform_length_chooser(string_min_length, string_max_length)
    } else {
        eprintln!("unknown length mode {}", length);
        std::process::exit(1);
    };
    // alphabet to use
    let charset = args.value_of("charset").unwrap_or("default");
    let characters = if charset == "default" {
        armnod::CharSetChooser::new(armnod::CHAR_SET_DEFAULT)
    } else if charset == "lower" {
        armnod::CharSetChooser::new(armnod::CHAR_SET_LOWER)
    } else if charset == "upper" {
        armnod::CharSetChooser::new(armnod::CHAR_SET_UPPER)
    } else if charset == "alpha" {
        armnod::CharSetChooser::new(armnod::CHAR_SET_ALPHA)
    } else if charset == "digit" {
        armnod::CharSetChooser::new(armnod::CHAR_SET_DIGIT)
    } else if charset == "alnum" {
        armnod::CharSetChooser::new(armnod::CHAR_SET_ALNUM)
    } else if charset == "punct" {
        armnod::CharSetChooser::new(armnod::CHAR_SET_PUNCT)
    } else if charset == "hex" {
        armnod::CharSetChooser::new(armnod::CHAR_SET_HEX)
    } else {
        eprintln!("unknown character set {}", charset);
        std::process::exit(1);
    };
    let characters: Box<dyn armnod::CharacterChooser> = Box::new(characters);
    // generate strings
    let mut armnod = ARMNOD {
        string: string_chooser,
        length: length_chooser,
        characters,
        buffer: Vec::new(),
    };
    let mut guac = Guacamole::default();
    // Buffer stdout
    let mut fout = BufWriter::new(std::io::stdout());
    for _ in 0..n {
        match armnod.choose(&mut guac) {
            Some(x) => { writeln!(fout, "{}", x).unwrap(); }
            None => break,
        }
    }
}
