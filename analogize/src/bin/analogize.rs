use std::path::PathBuf;

use rustyline::error::ReadlineError;
use rustyline::history::FileHistory;
use rustyline::{Config, Editor, Result};

use arrrg::CommandLine;

use analogize::{Analogize, AnalogizeOptions, Error, Query};

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
                match Query::parse(line) {
                    Ok(query) => {
                        match analogize.query(query) {
                            Ok(results) => {
                                for result in results {
                                    println!("{}", result);
                                }
                            }
                            Err(err) => {
                                eprintln!("error: {}", err);
                            }
                        };
                    }
                    Err(Error::Parsing { core: _, what }) => {
                        eprintln!("{}", what);
                    }
                    Err(err) => {
                        eprintln!("error: {}", err);
                    }
                };
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
