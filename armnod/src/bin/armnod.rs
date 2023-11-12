use std::io::{BufWriter, Write};

use arrrg::CommandLine;

use guacamole::Guacamole;

use armnod::{Armnod, ArmnodOptions, LengthChooser, SeedChooser};

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
    let (mut cmdline, free) = ArmnodOptions::from_command_line("Usage: armnod [--options]");
    if !free.is_empty() {
        panic!("free arguments are not accepted");
    }
    let string_chooser = if cmdline.chooser_mode == "random" {
        random_chooser()
    } else if cmdline.chooser_mode == "set" {
        set_chooser(cmdline.cardinality.unwrap_or(1_000))
    } else if cmdline.chooser_mode == "set-once" {
        let cardinality = cmdline.cardinality.unwrap_or(1_000);
        let set_once_begin = cmdline.set_once_begin.unwrap_or(0);
        let set_once_end = cmdline.set_once_end.unwrap_or(cardinality);
        if set_once_begin > set_once_end {
            panic!(
                "--set-once-begin must be <= --set-once-end: {} > {}",
                set_once_begin, set_once_end
            );
        }
        if cmdline.number > set_once_end - set_once_begin {
            cmdline.number = set_once_end - set_once_begin;
        }
        set_chooser_once(set_once_begin, set_once_end)
    } else if cmdline.chooser_mode == "set-zipf" {
        let cardinality = cmdline.cardinality.unwrap_or(1_000);
        let zipf_theta = cmdline.zipf_theta.unwrap_or(0.99);
        set_chooser_zipf(cardinality, zipf_theta)
    } else {
        eprintln!("unknown chooser mode {}", cmdline.chooser_mode);
        std::process::exit(1);
    };
    // length chooser
    let length = cmdline.length_mode.unwrap_or("constant".to_string());
    let length_chooser = if length == "constant" {
        constant_length_chooser(cmdline.string_length.unwrap_or(8))
    } else if length == "uniform" {
        let string_min_length: u32 = cmdline.string_min_length.unwrap_or(8);
        let string_max_length: u32 = cmdline.string_max_length.unwrap_or(string_min_length + 8);
        if string_min_length > string_max_length {
            panic!(
                "--string-min-length must be <= --string-max-length: {} > {}",
                string_min_length, string_max_length
            );
        }
        uniform_length_chooser(string_min_length, string_max_length)
    } else {
        eprintln!("unknown length mode {}", length);
        std::process::exit(1);
    };
    // alphabet to use
    let charset = cmdline.charset.unwrap_or("default".to_string());
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
    let mut armnod = Armnod {
        string: string_chooser,
        length: length_chooser,
        characters,
        buffer: Vec::new(),
    };
    let mut guac = Guacamole::default();
    // Buffer stdout
    let mut fout = BufWriter::new(std::io::stdout());
    for _ in 0..cmdline.number {
        match armnod.choose(&mut guac) {
            Some(x) => {
                writeln!(fout, "{}", x).unwrap();
            }
            None => break,
        }
    }
}
