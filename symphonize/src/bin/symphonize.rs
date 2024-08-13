use std::sync::Arc;

use arrrg::CommandLine;
use rustyline::error::ReadlineError;
use rustyline::history::FileHistory;
use rustyline::{Config, Editor};
use utf8path::Path;

use rustrc::{Pid1, Pid1Options, Target};

#[derive(Clone, Debug, Default, Eq, PartialEq, arrrg_derive::CommandLine)]
pub struct Options {}

fn paths_to_root() -> Result<Vec<Path<'static>>, std::io::Error> {
    let mut cwd = Path::try_from(std::env::current_dir()?).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::Other,
            "current working directory not unicode",
        )
    })?;
    if !cwd.is_abs() && !cwd.has_root() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "current working directory absolute",
        ));
    }
    let mut candidates = vec![];
    while cwd != Path::from("/") {
        candidates.push(cwd.clone().into_owned());
        if cwd.join(".git").exists() {
            candidates.reverse();
            return Ok(candidates);
        }
        cwd = cwd.dirname().into_owned();
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "no git directory found",
    ))
}

fn rc_conf_path(candidates: &[Path]) -> String {
    let mut rc_conf_path = String::new();
    for candidate in candidates {
        if !rc_conf_path.is_empty() {
            rc_conf_path.push(':');
        }
        rc_conf_path += candidate.join("rc.conf").as_str();
    }
    rc_conf_path
}

fn rc_d_path(options: &Options, root: &Path) -> Result<String, std::io::Error> {
    let mut rc_d_paths = vec![];
    rc_d_path_recurse(options, root, &mut rc_d_paths)?;
    Ok(rc_d_paths
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>()
        .join(":"))
}

// TODO(rescrv): address this
#[allow(clippy::only_used_in_recursion)]
fn rc_d_path_recurse(
    options: &Options,
    root: &Path,
    rc_d_paths: &mut Vec<Path<'static>>,
) -> Result<(), std::io::Error> {
    if root.is_dir() {
        let mut entries = vec![];
        for entry in std::fs::read_dir(root.clone().into_std())? {
            let entry = entry?;
            let Ok(path) = Path::try_from(entry.path()) else {
                continue;
            };
            entries.push(path.into_owned());
        }
        entries.sort();
        for entry in entries.into_iter() {
            if entry.as_str().ends_with("/rc.d") {
                rc_d_paths.push(entry.clone());
            }
            rc_d_path_recurse(options, &entry, rc_d_paths)?;
        }
    }
    Ok(())
}

fn autoinfer_configuration(options: &Options) -> Result<Pid1Options, std::io::Error> {
    let paths_to_root = paths_to_root()?;
    assert!(!paths_to_root.is_empty());
    let repo = &paths_to_root[0];
    let rc_conf_path = rc_conf_path(&paths_to_root);
    let rc_d_path = rc_d_path(options, repo)?;
    Ok(Pid1Options {
        rc_conf_path,
        rc_d_path,
    })
}

fn services(_: &Options, pid1: &Pid1, argv: &[&str]) {
    let mut opts = getopts::Options::new();
    opts.parsing_style(getopts::ParsingStyle::StopAtFirstFree);
    opts.optflag("l", "list", "List all possible services.");
    opts.optflag("e", "enabled", "List enabled services.");
    opts.optflag("r", "reload", "Reload the rc_conf and rc_d directories.");
    opts.optflag("s", "start", "Start one or more services.");
    opts.optflag("S", "stop", "Stop one or more services.");
    opts.optflag("R", "restart", "Restart one or more services.");

    let matches = match opts.parse(argv) {
        Ok(matches) => matches,
        Err(err) => {
            eprintln!("error: {err:?}");
            return;
        }
    };

    let free: Vec<String> = matches.free.to_vec();

    if matches.opt_present("l") {
        let mut targets: Vec<_> = free.iter().map(Target::from).collect();
        for service in pid1.list_services() {
            if !targets.is_empty() && !targets.iter_mut().any(|t| t.matches_name(&service)) {
                continue;
            }
            println!("{service}");
        }
    }

    if matches.opt_present("e") {
        let mut targets: Vec<_> = free.iter().map(Target::from).collect();
        for service in pid1.enabled_services() {
            if !targets.is_empty() && !targets.iter_mut().any(|t| t.matches_name(&service)) {
                continue;
            }
            println!("{service}");
        }
    }

    if matches.opt_present("r") {
        if let Err(err) = pid1.reload() {
            eprintln!("error: {err:?}");
        }
    }

    if matches.opt_present("s") {
        for service in free.iter() {
            if let Err(err) = pid1.start(service) {
                eprintln!("{service}: error: {err:?}");
            } else {
                println!("{service}: success");
            }
        }
    }

    if matches.opt_present("R") {
        for service in free.iter() {
            if let Err(err) = pid1.restart(service) {
                eprintln!("{service}: error: {err:?}");
            } else {
                println!("{service}: success");
            }
        }
    }

    if matches.opt_present("S") {
        for service in free.iter() {
            if let Err(err) = pid1.stop(service) {
                eprintln!("{service}: error: {err:?}");
            } else {
                println!("{service}: success");
            }
        }
    }
}

fn shell(options: Options, pid1: Arc<Pid1>) {
    // Create the line editor.
    let config = Config::builder()
        .max_history_size(1_000_000)
        .expect("config builder should allow 1e6 history entries")
        .history_ignore_dups(true)
        .expect("config builder should ignore dupes")
        .history_ignore_space(true)
        .build();
    let hist = FileHistory::with_config(config);
    let mut rl: Editor<(), FileHistory> =
        Editor::with_history(config, hist).expect("editor should construct");
    let history = std::path::PathBuf::from(".symphonize.history");
    if history.exists() {
        rl.load_history(&history)
            .expect("should be able to load history");
    }

    loop {
        let line = rl.readline("symphonize> ");
        match line {
            Ok(line) => {
                rl.add_history_entry(&line)
                    .expect("should be able to save history");
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let args = match shvar::split(line) {
                    Ok(args) => args,
                    Err(err) => {
                        eprintln!("error: {err:?}");
                        continue;
                    }
                };
                if args.is_empty() {
                    continue;
                }
                let args = args.iter().map(|s| s.as_str()).collect::<Vec<_>>();
                match args[0] {
                    "services" | "service" => {
                        services(&options, &pid1, &args[1..]);
                    },
                    "reload" => {
                        let mut child = match std::process::Command::new("cargo")
                            .args(&["build", "--workspace", "--bins"])
                            .spawn() {
                            Ok(child) => child,
                            Err(err) => {
                                eprintln!("error: {err:?}");
                                continue;
                            },
                        };
                        let status = match child.wait() {
                            Ok(status) => status,
                            Err(err) => {
                                eprintln!("error: {err:?}");
                                continue;
                            },
                        };
                        if !status.success() {
                            eprintln!("error: reload incomplete");
                            continue;
                        }
                        let pid1_options = match autoinfer_configuration(&options) {
                            Ok(pid1_options) => pid1_options,
                            Err(err) => {
                                eprintln!("error: {err:?}");
                                continue;
                            }
                        };
                        if let Err(err) = pid1.reconfigure(pid1_options) {
                            eprintln!("error: {err:?}");
                            continue;
                        }
                        if let Err(err) = pid1.reload() {
                            eprintln!("error: {err:?}");
                            continue;
                        }
                    },
                    "exit" => {
                        break;
                    },
                    _ => {
                        eprintln!("error: uknown command: {}", &args[0]);
                    },
                };
            }
            Err(ReadlineError::Interrupted) => {}
            Err(ReadlineError::Eof) => {}
            Err(err) => {
                rl.save_history(&history)
                    .expect("should be able to save history");
                eprintln!("could not read line: {}", err);
            }
        }
    }
}

fn main() {
    minimal_signals::block();

    // Parse options.
    let (options, free) = Options::from_command_line("USAGE: symphonize");
    if !free.is_empty() {
        eprintln!("symphonize takes no positional arguments");
        std::process::exit(129);
    }
    let pid1_options =
        autoinfer_configuration(&options).expect("should be able to infer configuration");

    // Setup Pid1.
    let mut pid1 = Arc::new(Pid1::new(pid1_options).expect("pid1::new should work"));

    // Create a thread to listen for signals and cancel the context if need be.
    let signal_pid1 = Arc::downgrade(&pid1);
    let signal = std::thread::spawn(move || loop {
        let signal_set = minimal_signals::SignalSet::new().fill();
        let signal = minimal_signals::wait(signal_set);
        if signal == Some(minimal_signals::SIGCHLD) {
            continue;
        }
        let Some(pid1) = signal_pid1.upgrade() else {
            break;
        };
        if let Some(signal) = signal {
            let _ = pid1.kill(Target::All, signal);
        }
    });

    // Create a new interactive shell.
    let shell_pid1 = Arc::clone(&pid1);
    let server = std::thread::spawn(move || shell(options, shell_pid1));

    // Cleanup
    server.join().unwrap();
    drop(signal);

    // NOTE(rescrv):  This is a spin loop because there's no good way to synchronize this simply.
    // It shouldn't spin for more than a few times.
    let pid1 = {
        loop {
            break match Arc::try_unwrap(pid1) {
                Ok(pid1) => pid1,
                Err(p) => {
                    pid1 = p;
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }
            };
        }
    };

    pid1.shutdown().expect("shutdown should work");
}
