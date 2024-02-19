use std::path::PathBuf;
use std::str::FromStr;

use rustyline::error::ReadlineError;
use rustyline::history::FileHistory;
use rustyline::{Config, Editor, Result};

use arrrg::CommandLine;

use analogize::{Analogize, AnalogizeOptions, Error, Query};

const CORRELATE_DEFAULT_N: usize = 5;

fn main() -> Result<()> {
    // Process the command line.
    let (options, free) =
        AnalogizeOptions::from_command_line("Usage: analogize --json <dir> --data <dir> [OPTIONS]");
    if !free.is_empty() {
        eprintln!("command takes no positional arguments");
        std::process::exit(1);
    }

    // Create the analogize instance.
    let history = PathBuf::from(options.data()).join(".history");
    let mut analogize = match Analogize::new(options) {
        Ok(analogize) => analogize,
        Err(err) => {
            eprintln!("could not instantiate analogize: {}", err);
            std::process::exit(1);
        }
    };

    // Create the line editor.
    let config = Config::builder()
        .max_history_size(1_000_000)?
        .history_ignore_dups(true)?
        .history_ignore_space(true)
        .build();
    let hist = FileHistory::with_config(config);
    let mut rl: Editor<(), FileHistory> = Editor::with_history(config, hist)?;
    if history.exists() {
        rl.load_history(&history)?;
    }

    loop {
        let line = rl.readline("analogize> ");
        match line {
            Ok(line) => {
                rl.add_history_entry(&line)?;
                let line = line.trim();
                let position = line.find(' ').unwrap_or(line.len());
                let command = &line[..position];
                let mut remainder = line[position..].trim();
                match command {
                    "correlate" => {
                        let number = if let Some(space) = remainder.find(' ') {
                            let number = &remainder[..space].trim();
                            if let Some(number) = usize::from_str(number).ok() {
                                remainder = &remainder[space..].trim();
                                number
                            } else {
                                CORRELATE_DEFAULT_N
                            }
                        } else {
                            CORRELATE_DEFAULT_N
                        };
                        let query = match Query::parse(remainder) {
                            Ok(query) => query,
                            Err(Error::Parsing { core: _, what }) => {
                                eprintln!("{}", what);
                                continue;
                            }
                            Err(err) => {
                                eprintln!("unexpected error: {}", err);
                                continue;
                            }
                        };
                        match analogize.correlate(query, number) {
                            Ok(exemplars) => {
                                for exemplar in exemplars {
                                    println!("{}", exemplar);
                                }
                            }
                            Err(err) => {
                                eprintln!("error: {}", err);
                            }
                        };
                    }
                    "exemplars" => {
                        let number = if let Some(number) = usize::from_str(remainder).ok() {
                            number
                        } else {
                            CORRELATE_DEFAULT_N
                        };
                        match analogize.exemplars(number) {
                            Ok(exemplars) => {
                                for exemplar in exemplars {
                                    println!("{}", exemplar);
                                }
                            }
                            Err(err) => {
                                eprintln!("error: {}", err);
                            }
                        };
                    }
                    command => {
                        eprintln!("unknown command: {}", command);
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                rl.save_history(&history)?;
                return Ok(());
            }
            Err(ReadlineError::Eof) => {
                rl.save_history(&history)?;
                return Ok(());
            }
            Err(err) => {
                rl.save_history(&history)?;
                panic!("could not read line: {}", err);
            }
        }
    }
}
