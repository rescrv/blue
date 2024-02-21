use std::path::PathBuf;

use rustyline::error::ReadlineError;
use rustyline::history::FileHistory;
use rustyline::{Config, Editor, Result};

use arrrg::CommandLine;

use analogize::{Analogize, AnalogizeOptions, Error, Query};

fn parse_query(line: &str) -> Option<Query> {
    match Query::parse(line) {
        Ok(query) => {
            Some(query)
        },
        Err(Error::Parsing { core: _, what }) => {
            eprintln!("{}", what);
            None
        }
        Err(err) => {
            eprintln!("error: {}", err);
            None
        }
    }
}

fn main() -> Result<()> {
    // Process the command line.
    let (options, free) =
        AnalogizeOptions::from_command_line("Usage: analogize --logs <dir> --data <dir> [OPTIONS]");
    if !free.is_empty() {
        eprintln!("command takes no positional arguments");
        std::process::exit(1);
    }

    // Create the analogize instance.
    let history = PathBuf::from(options.data()).join(".history");
    let analogize = match Analogize::new(options) {
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
                if let Some(query) = line.strip_prefix("exemplars") {
                    let query = query.trim();
                    if let Ok(query) = Query::parse(query.trim()) {
                        match analogize.correlates(query, 10) {
                            Ok(exemplars) => {
                                for exemplar in exemplars {
                                    println!("{}", exemplar);
                                }
                            },
                            Err(err) => {
                                eprintln!("error: {}", err);
                            }
                        };
                    } else {
                        match analogize.exemplars(10) {
                            Ok(exemplars) => {
                                for exemplar in exemplars {
                                    println!("{}", exemplar);
                                }
                            },
                            Err(err) => {
                                eprintln!("error: {}", err);
                            }
                        };
                    }
                } else {
                    let Some(query) = parse_query(line) else {
                        continue;
                    };
                    match analogize.query(query) {
                        Ok(results) => {
                            for result in results {
                                println!("{}", result);
                            }
                        },
                        Err(err) => {
                            eprintln!("error: {}", err);
                        },
                    };
                }
            }
            Err(ReadlineError::Interrupted) => {
                rl.save_history(&history)?;
            }
            Err(ReadlineError::Eof) => {
                rl.save_history(&history)?;
                return Ok(());
            }
            Err(err) => {
                rl.save_history(&history)?;
                eprintln!("could not read line: {}", err);
            }
        }
    }
}
