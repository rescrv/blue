use std::fs::read;
use std::path::PathBuf;
use std::time::Instant;

use buffertk::Unpackable;

use scrunch::{CompressedDocument, Document};

const DEFAULT_INPUT: &str = "/tmp/gutenberg-1g.txt.scrunch";
const SPACE: u32 = b' ' as u32;

#[derive(Clone, Debug)]
struct Options {
    input: PathBuf,
    results: usize,
    iterations: usize,
    warm_up: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            input: PathBuf::from(DEFAULT_INPUT),
            results: 20,
            iterations: 5,
            warm_up: 1,
        }
    }
}

#[derive(Debug)]
struct Word {
    count: usize,
    text: String,
}

#[derive(Debug)]
struct Measurement {
    input_bytes: usize,
    doc_symbols: usize,
    doc_records: usize,
    results: usize,
    iterations: usize,
    elapsed_ns: u128,
    checksum: usize,
    words: Vec<Word>,
}

fn main() {
    let options = parse_args(std::env::args().skip(1)).unwrap_or_else(|err| {
        eprintln!("{err}");
        std::process::exit(2);
    });
    let input = read(&options.input).unwrap_or_else(|err| {
        eprintln!("could not read {}: {err}", options.input.display());
        std::process::exit(1);
    });
    let doc = CompressedDocument::unpack(&input).unwrap_or_else(|err| {
        eprintln!("unpack failed: {err:?}");
        std::process::exit(1);
    });
    let doc = doc.0;
    let measurement = measure_case(&options, input.len(), &doc);
    print_measurement(&measurement);
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut options = Options::default();
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bench" => {}
            "--input" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--input requires a path".to_string())?;
                options.input = PathBuf::from(value);
            }
            "--results" => {
                options.results = parse_next_usize("--results", &mut args)?;
            }
            "--iterations" => {
                options.iterations = parse_next_usize("--iterations", &mut args)?;
            }
            "--warm-up" => {
                options.warm_up = parse_next_usize("--warm-up", &mut args)?;
            }
            "--help" | "-h" => {
                return Err(usage());
            }
            other => {
                return Err(format!("unrecognized argument: {other}\n{}", usage()));
            }
        }
    }
    if options.results == 0 {
        return Err("--results must be positive".to_string());
    }
    if options.iterations == 0 {
        return Err("--iterations must be positive".to_string());
    }
    Ok(options)
}

fn parse_next_usize(
    flag: &str,
    args: &mut std::iter::Peekable<impl Iterator<Item = String>>,
) -> Result<usize, String> {
    let value = args
        .next()
        .ok_or_else(|| format!("{flag} requires a value"))?;
    value
        .parse()
        .map_err(|_| format!("{flag} must be an integer: {value}"))
}

fn usage() -> String {
    format!(
        "USAGE: exemplars [--input PATH] [--results N] [--iterations N] [--warm-up N]\n\
         Default input: {DEFAULT_INPUT}"
    )
}

fn measure_case(
    options: &Options,
    input_bytes: usize,
    doc: &CompressedDocument<'_>,
) -> Measurement {
    let docs = [doc];
    for _ in 0..options.warm_up {
        std::hint::black_box(top_words(&docs, options.results));
    }
    let start = Instant::now();
    let mut checksum = 0usize;
    let mut words = Vec::new();
    for _ in 0..options.iterations {
        words = top_words(&docs, options.results);
        checksum ^= std::hint::black_box(checksum_words(&words));
    }
    Measurement {
        input_bytes,
        doc_symbols: doc.len(),
        doc_records: doc.records(),
        results: options.results,
        iterations: options.iterations,
        elapsed_ns: start.elapsed().as_nanos(),
        checksum,
        words,
    }
}

fn top_words(docs: &[&CompressedDocument<'_>], results: usize) -> Vec<Word> {
    scrunch::exemplars_with_min_length(docs, &[(SPACE, SPACE)], 3)
        .map(|exemplar| Word {
            count: exemplar.count(),
            text: word_text(exemplar.text()),
        })
        .filter(|word| word.text.chars().any(|c| !c.is_whitespace()))
        .take(results)
        .collect()
}

fn word_text(text: &[u32]) -> String {
    let text = if text.len() >= 2 && text[0] == SPACE && text[text.len() - 1] == SPACE {
        &text[1..text.len() - 1]
    } else {
        text
    };
    text.iter()
        .copied()
        .filter_map(char::from_u32)
        .collect::<String>()
}

fn checksum_words(words: &[Word]) -> usize {
    words.iter().fold(0usize, |checksum, word| {
        word.text
            .bytes()
            .fold(checksum ^ word.count, |checksum, byte| {
                checksum.wrapping_mul(16_777_619) ^ byte as usize
            })
    })
}

fn print_measurement(measurement: &Measurement) {
    let elapsed_ms = measurement.elapsed_ns as f64 / 1_000_000.0;
    let per_iter_ms = elapsed_ms / measurement.iterations as f64;
    println!(
        "input_bytes\tdoc_symbols\tdoc_records\tresults\titerations\telapsed_ms\tms_per_iter\tchecksum"
    );
    println!(
        "{}\t{}\t{}\t{}\t{}\t{elapsed_ms:.3}\t{per_iter_ms:.3}\t{}",
        measurement.input_bytes,
        measurement.doc_symbols,
        measurement.doc_records,
        measurement.results,
        measurement.iterations,
        measurement.checksum
    );
    println!("rank\tcount\tword");
    for (rank, word) in measurement.words.iter().enumerate() {
        println!("{}\t{}\t{}", rank + 1, word.count, word.text.escape_debug());
    }
}
