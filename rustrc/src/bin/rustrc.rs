use std::sync::Arc;

use arrrg::CommandLine;
use utf8path::Path;

use rustrc::{Pid1, Pid1Options, Target};

#[derive(Clone, Debug, Eq, PartialEq, arrrg_derive::CommandLine)]
pub struct Options {
    #[arrrg(optional, "Path to the UNIX control socket (must not already exist).")]
    pub control_sock: String,
    #[arrrg(optional, "A colon-separated PATH-like list of rc.conf files to be loaded in order.  Later files override.")]
    pub rc_conf_path: String,
    #[arrrg(optional, "A colon-separated PATH-like list of rc.d directories to be scanned in order.  Earlier files short-circuit.")]
    pub rc_d_path: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            control_sock: "rc.sock".to_string(),
            rc_conf_path: "rc.conf".to_string(),
            rc_d_path: "rc.d".to_string(),
        }
    }
}

#[derive(Debug)]
struct UnixSockAdapter {
    pid1: Arc<Pid1>,
}

impl UnixSockAdapter {
    fn new(pid1: Arc<Pid1>) -> Self {
        Self { pid1 }
    }
}

impl unix_sock::Invokable for UnixSockAdapter {
    fn invoke(&self, command: &str) -> String {
        let argv = match shvar::split(command) {
            Ok(argv) => argv,
            Err(err) => {
                return format!("error: {err:?}");
            }
        };
        if argv.is_empty() {
            return String::new();
        }
        let mut response = String::new();
        match argv[0].as_str() {
            "services" => {
                let mut opts = getopts::Options::new();
                opts.parsing_style(getopts::ParsingStyle::StopAtFirstFree);
                opts.optflag("l", "list", "List all possible services.");
                opts.optflag("e", "enabled", "List enabled services.");
                opts.optflag("r", "reload", "Reload the rc_conf and rc_d directories.");
                opts.optflag("s", "start", "Start one or more services.");
                opts.optflag("S", "stop", "Stop one or more services.");
                opts.optflag("R", "restart", "Restart one or more services.");

                let matches = match opts.parse(&argv[1..]) {
                    Ok(matches) => matches,
                    Err(err) => {
                        return format!("error: {err:?}");
                    }
                };

                let free: Vec<String> = matches.free.to_vec();

                if matches.opt_present("l") {
                    let mut targets: Vec<_> = free.iter().map(Target::from).collect();
                    for service in self.pid1.list_services() {
                        if !targets.is_empty()
                            && !targets.iter_mut().any(|t| t.matches_name(&service))
                        {
                            continue;
                        }
                        response += &service;
                        response.push('\n');
                    }
                }

                if matches.opt_present("e") {
                    let mut targets: Vec<_> = free.iter().map(Target::from).collect();
                    for service in self.pid1.enabled_services() {
                        if !targets.is_empty()
                            && !targets.iter_mut().any(|t| t.matches_name(&service))
                        {
                            continue;
                        }
                        response += &service;
                        response.push('\n');
                    }
                }

                if matches.opt_present("r") {
                    if let Err(err) = self.pid1.reload() {
                        response += &format!("error: {err:?}");
                    }
                }

                // NOTE(rescrv):  I've gone back and forth on these five lines.
                //
                // On the one hand, it's handy to restart everything in one fell swoop.
                //
                // On the other hand, it's SEV-worthy to restart everything in one fell swoop.
                /*
                let free = if free.is_empty() {
                    self.pid1.enabled_services()
                } else {
                    free
                };
                */

                if matches.opt_present("s") {
                    for service in free.iter() {
                        if let Err(err) = self.pid1.start(service) {
                            response += &format!("{service}: error: {err:?}");
                        } else {
                            response += &format!("{service}: success");
                        }
                    }
                }

                if matches.opt_present("R") {
                    for service in free.iter() {
                        if let Err(err) = self.pid1.restart(service) {
                            response += &format!("{service}: error: {err:?}");
                        } else {
                            response += &format!("{service}: success");
                        }
                    }
                }

                if matches.opt_present("S") {
                    for service in free.iter() {
                        if let Err(err) = self.pid1.stop(service) {
                            response += &format!("{service}: error: {err:?}");
                        } else {
                            response += &format!("{service}: success");
                        }
                    }
                }
            }
            _ => {
                return format!("error: unknown command {:?}", &argv[0]);
            }
        }
        response
    }
}

fn main() {
    minimal_signals::block();

    let (options, free) = Options::from_command_line(
        "USAGE: rustrc --control SOCKET --rc-conf-path PATH --rc-d-path PATH",
    );
    if !free.is_empty() {
        eprintln!("rustrc takes no positional arguments");
        std::process::exit(129);
    }

    // Setup Pid1
    let pid1_options = Pid1Options {
        rc_conf_path: options.rc_conf_path.clone(),
        rc_d_path: options.rc_d_path.clone(),
    };
    let mut pid1 = Arc::new(Pid1::new(pid1_options).expect("pid1::new should work"));

    // Setup a context that we can cancel on.
    let context = unix_sock::Context::new().expect("context should create");

    // Create a thread to listen for signals and cancel the context if need be.
    let signal_pid1 = Arc::downgrade(&pid1);
    let signal_context = context.clone();
    let signal = std::thread::spawn(move || loop {
        let signal_set = minimal_signals::SignalSet::new().fill();
        let signal = minimal_signals::wait(signal_set);
        if signal == Some(minimal_signals::SIGCHLD) {
            continue;
        }
        signal_context.cancel();
        let Some(pid1) = signal_pid1.upgrade() else {
            break;
        };
        if let Some(signal) = signal {
            let _ = pid1.kill(Target::All, signal);
        }
    });

    // Create a new unix sock that's listening.
    let adapter = UnixSockAdapter::new(Arc::clone(&pid1));
    let mut server_sock =
        unix_sock::Server::new(Path::from(options.control_sock.as_str()), adapter)
            .expect("server should instantiate");
    let server_context = context.clone();
    let server = std::thread::spawn(move || {
        server_sock
            .serve(&server_context)
            .expect("serve should not error");
    });

    // Cleanup
    server.join().unwrap();
    drop(signal);
    let _ = std::fs::remove_file(options.control_sock);

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
