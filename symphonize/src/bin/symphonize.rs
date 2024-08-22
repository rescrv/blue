use std::fs::create_dir_all;
use std::sync::Arc;

use arrrg::CommandLine;
use indicio::stdio::StdioEmitter;
use indicio::{clue, ALWAYS, INFO};
use rustyline::error::ReadlineError;
use rustyline::history::FileHistory;
use rustyline::{Config, Editor};
use utf8path::Path;

use rustrc::{Pid1, Target, COLLECTOR};

use symphonize::{autoinfer_configuration, rebuild_cargo, rebuild_release, SymphonizeOptions};

fn services(_: &SymphonizeOptions, pid1: &Pid1, argv: &[&str]) {
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

fn shell(
    options: SymphonizeOptions,
    pid1: Arc<Pid1>,
    rebuild: impl Fn(&Path<'static>) -> Result<(), std::io::Error>,
    rebuild_dir: Path<'static>,
) {
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
                    }
                    "reload" => {
                        if let Err(err) = rebuild(&rebuild_dir) {
                            eprintln!("failed to rebuild: {err:?}");
                            continue;
                        }
                        let pid1_options = match autoinfer_configuration(&options) {
                            Ok((pid1_options, _)) => pid1_options,
                            Err(err) => {
                                eprintln!("error: {err:?}");
                                continue;
                            }
                        };
                        if let Err(err) = pid1.reconfigure(pid1_options) {
                            eprintln!("error: {err:?}");
                        }
                    }
                    "exit" => {
                        break;
                    }
                    _ => {
                        eprintln!("error: uknown command: {}", &args[0]);
                    }
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
    rl.save_history(&history)
        .expect("should be able to save history");
}

fn main() {
    minimal_signals::block();

    // Parse options.
    let (options, free) = SymphonizeOptions::from_command_line("USAGE: symphonize");
    if !free.is_empty() {
        eprintln!("symphonize takes no positional arguments");
        std::process::exit(129);
    }
    if options.debug && options.release {
        eprintln!("symphonize cannot run debug and release simultaneously");
        std::process::exit(130);
    }
    let (pid1_options, root) =
        autoinfer_configuration(&options).expect("should be able to infer configuration");

    // Working directory
    let workdir: Path<'static> = if let Some(workdir) = options.workdir.as_ref() {
        Path::from(workdir.as_str()).into_owned()
    } else {
        root.join(".symphonize").into_owned()
    };
    create_dir_all(&workdir).expect("symphonize dir should create");
    create_dir_all(workdir.join("vendor")).expect("vendor dir should create");
    create_dir_all(workdir.join("pkg")).expect("pkg dir should create");

    // PATH
    let mut path = std::env::var_os("PATH").unwrap_or_default();
    if options.release {
        todo!();
    } else {
        path.push(":");
        path.push(workdir.join("pkg/bin").as_str());
        path.push(":");
        let bindir = root.join("target/debug");
        path.push(bindir.as_str());
    }
    std::env::set_var("PATH", path);

    // Indicio.
    let emitter = Arc::new(StdioEmitter);
    COLLECTOR.register(Arc::clone(&emitter));
    COLLECTOR.set_verbosity(INFO);
    clue!(COLLECTOR, ALWAYS, {
        env: std::env::vars().map(|(k, v)| k + "=" + &v).collect::<Vec<_>>(),
        args: std::env::args().map(String::from).collect::<Vec<_>>(),
    });

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
    let server = if options.release {
        std::thread::spawn(move || shell(options, shell_pid1, rebuild_release, workdir))
    } else {
        std::thread::spawn(move || shell(options, shell_pid1, rebuild_cargo, workdir))
    };

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
