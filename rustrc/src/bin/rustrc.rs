use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use arrrg::CommandLine;
use utf8path::Path;

use rustrc::{Pid1, Pid1Options, ServiceStatus, StateDir, Target};

#[derive(Clone, Debug, Eq, PartialEq, arrrg_derive::CommandLine)]
pub struct Options {
    #[arrrg(
        optional,
        "Path to the UNIX control socket.  A stale socket is replaced; a live one is an error."
    )]
    pub control_sock: String,
    #[arrrg(
        optional,
        "A colon-separated PATH-like list of rc.conf files to be loaded in order.  Later files override."
    )]
    pub rc_conf_path: String,
    #[arrrg(
        optional,
        "A colon-separated PATH-like list of rc.d directories.  A service defined in more than one is an error."
    )]
    pub rc_d_path: String,
    #[arrrg(
        optional,
        "Directory for the single-instance lock and records used to fence leftovers after a crash."
    )]
    pub state_dir: String,
    #[arrrg(flag, "Run without a state directory:  no instance lock, no fencing.")]
    pub no_state_dir: bool,
    #[arrrg(
        flag,
        "Split into a minimal init and the supervisor even when rustrc is not process 1."
    )]
    pub container_init: bool,
    #[arrrg(
        flag,
        "Do not create a UNIX control socket; run until a shutdown signal arrives."
    )]
    pub no_control_sock: bool,
    #[arrrg(
        optional,
        "Log verbosity on stderr:  3 errors, 6 warnings, 9 lifecycle events (default), 12 debug."
    )]
    pub verbosity: u64,
    #[arrrg(
        optional,
        "Milliseconds a stub gets to answer `rcvar` before its process group is killed."
    )]
    pub stub_timeout_ms: u64,
    #[arrrg(
        optional,
        "Ceiling in milliseconds on every service's STOP_TIMEOUT, the default included (unset: none)."
    )]
    pub max_stop_timeout_ms: Option<u64>,
    #[arrrg(
        optional,
        "With the container init, milliseconds between SIGTERM and SIGKILL for what the supervisor left behind."
    )]
    pub init_grace_ms: u64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            control_sock: "rc.sock".to_string(),
            rc_conf_path: "rc.conf".to_string(),
            rc_d_path: "rc.d".to_string(),
            state_dir: "rc.state".to_string(),
            no_state_dir: false,
            container_init: false,
            no_control_sock: false,
            verbosity: indicio::INFO,
            stub_timeout_ms: Pid1Options::default().stub_timeout_ms,
            max_stop_timeout_ms: None,
            init_grace_ms: rustrc::DEFAULT_STOP_TIMEOUT.as_millis() as u64,
        }
    }
}

/// The control socket is bound before Pid1 exists (so a live sibling is detected before anything
/// starts); requests are served only once it is set.
struct UnixSockAdapter {
    pid1: Arc<OnceLock<Arc<Pid1>>>,
    metrics: Arc<biometrics::Collector>,
}

impl unix_sock::Invokable for UnixSockAdapter {
    fn invoke(&self, command: &str) -> String {
        let Some(pid1) = self.pid1.get() else {
            return "error: rustrc is starting".to_string();
        };
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
                opts.optflag(
                    "n",
                    "dry-run",
                    "With --reload, report what reloading would do without applying it.",
                );
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
                    for service in pid1.list_services() {
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
                    for service in pid1.enabled_services() {
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
                    let plan = if matches.opt_present("n") {
                        pid1.plan_reload()
                    } else {
                        pid1.reload_with_plan()
                    };
                    match plan {
                        Ok(plan) => response += &plan.to_string(),
                        Err(err) => response += &format!("error: {err:?}\n"),
                    }
                } else if matches.opt_present("n") {
                    return "error: --dry-run only applies to --reload".to_string();
                }

                // NOTE(rescrv):  I've gone back and forth on these five lines.
                //
                // On the one hand, it's handy to restart everything in one fell swoop.
                //
                // On the other hand, it's SEV-worthy to restart everything in one fell swoop.
                /*
                let free = if free.is_empty() {
                    pid1.enabled_services()
                } else {
                    free
                };
                */

                if matches.opt_present("s") {
                    for service in free.iter() {
                        if let Err(err) = pid1.start(service) {
                            response += &format!("{service}: error: {err:?}\n");
                        } else {
                            response += &format!("{service}: success\n");
                        }
                    }
                }

                if matches.opt_present("R") {
                    for service in free.iter() {
                        if let Err(err) = pid1.restart(service) {
                            response += &format!("{service}: error: {err:?}\n");
                        } else {
                            response += &format!("{service}: success\n");
                        }
                    }
                }

                if matches.opt_present("S") {
                    for service in free.iter() {
                        if let Err(err) = pid1.stop(service) {
                            response += &format!("{service}: error: {err:?}\n");
                        } else {
                            response += &format!("{service}: success\n");
                        }
                    }
                }
            }
            "metrics" => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                let mut emitter = biometrics_prometheus::SlashMetrics::new();
                if let Err(err) = self.metrics.emit(&mut emitter, now) {
                    return format!("error: {err:?}");
                }
                response += &emitter.take();
            }
            "status" => {
                let mut targets: Vec<_> = argv[1..].iter().map(Target::from).collect();
                let statuses = pid1
                    .status()
                    .into_iter()
                    .filter(|s| {
                        targets.is_empty() || targets.iter_mut().any(|t| t.matches_name(&s.service))
                    })
                    .collect::<Vec<_>>();
                response += &render_status(&statuses);
            }
            "kill" => {
                let mut opts = getopts::Options::new();
                opts.parsing_style(getopts::ParsingStyle::StopAtFirstFree);
                opts.optopt("s", "signal", "Signal to send (name or number).", "SIGNAL");
                let matches = match opts.parse(&argv[1..]) {
                    Ok(matches) => matches,
                    Err(err) => {
                        return format!("error: {err:?}");
                    }
                };
                let name = matches.opt_str("s").unwrap_or_else(|| "TERM".to_string());
                let Some(signal) = rustrc::parse_signal(&name) else {
                    return format!("error: unknown signal {name:?}");
                };
                if matches.free.is_empty() {
                    return "error: name one or more services or pids".to_string();
                }
                for target in matches.free.iter() {
                    // NOTE:  Refuse "*"; signaling every service is not a one-token operation.
                    let parsed = if target == "*" {
                        response += "*: error: refusing to signal every service; name them\n";
                        continue;
                    } else if let Ok(pid) = target.parse::<i32>() {
                        Target::Pid(pid)
                    } else {
                        Target::One(target.clone())
                    };
                    match pid1.signal(parsed, signal) {
                        Ok(0) => response += &format!("{target}: error: no running process\n"),
                        Ok(n) => {
                            response += &format!("{target}: sent {signal} to {n} process(es)\n")
                        }
                        Err(err) => response += &format!("{target}: error: {err:?}\n"),
                    }
                }
            }
            _ => {
                return format!("error: unknown command {:?}", argv[0]);
            }
        }
        response
    }
}

fn human(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    if secs >= 86400 {
        format!("{}d{}h", secs / 86400, (secs % 86400) / 3600)
    } else if secs >= 3600 {
        format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{}m{}s", secs / 60, secs % 60)
    } else {
        format!("{:.1}s", d.as_secs_f64())
    }
}

fn render_status(statuses: &[ServiceStatus]) -> String {
    let mut rows = vec![[
        "SERVICE".to_string(),
        "STATE".to_string(),
        "PID".to_string(),
        "UPTIME".to_string(),
        "STARTS".to_string(),
        "LAST EXIT".to_string(),
        "BACKOFF".to_string(),
        "LOG".to_string(),
    ]];
    for s in statuses {
        let (pid, uptime) = match s.running.first() {
            Some((_, up)) => {
                let pids = s
                    .running
                    .iter()
                    .map(|(p, _)| p.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                (pids, human(*up))
            }
            None => ("-".to_string(), "-".to_string()),
        };
        rows.push([
            s.service.clone(),
            s.state().to_string(),
            pid,
            uptime,
            s.starts.to_string(),
            s.last_exit
                .as_ref()
                .map(|(what, ago)| format!("{what} ({} ago)", human(*ago)))
                .unwrap_or_else(|| "-".to_string()),
            s.backoff.map(human).unwrap_or_else(|| "-".to_string()),
            s.log.clone().unwrap_or_else(|| "-".to_string()),
        ]);
    }
    let mut widths = [0usize; 8];
    for row in rows.iter() {
        for (w, cell) in widths.iter_mut().zip(row.iter()) {
            *w = (*w).max(cell.len());
        }
    }
    let mut out = String::new();
    for row in rows.iter() {
        let line = row
            .iter()
            .zip(widths.iter())
            .map(|(cell, w)| format!("{cell:w$}"))
            .collect::<Vec<_>>()
            .join("  ");
        out += line.trim_end();
        out.push('\n');
    }
    out
}

fn main() {
    minimal_signals::block();

    let (options, free) = Options::from_command_line(
        "USAGE: rustrc [--control-sock SOCKET] [--rc-conf-path PATH] [--rc-d-path PATH] [--state-dir DIR]",
    );
    if !free.is_empty() {
        eprintln!("rustrc takes no positional arguments");
        std::process::exit(129);
    }
    // SAFETY(rescrv): getpid cannot fail.
    let running_as_pid1 = unsafe { libc::getpid() } == 1;
    if options.container_init || running_as_pid1 {
        // Before any thread exists.  The init half never returns.
        if let Err(err) = rustrc::init::split(Duration::from_millis(options.init_grace_ms)) {
            fatal(format!("could not start container init: {err}"));
        }
    }

    // Logging:  queued so a stalled stderr can never block rustrc.
    let (emitter, log_writer) = match rustrc::logging::QueuedEmitter::new(std::io::stderr(), 4096) {
        Ok(logging) => logging,
        Err(err) => fatal(format!("could not start logging: {err}")),
    };
    rustrc::COLLECTOR.register(emitter);
    rustrc::COLLECTOR.set_verbosity(options.verbosity);
    let code = run(options);
    rustrc::COLLECTOR.deregister();
    log_writer.finish(Duration::from_secs(2));
    std::process::exit(code);
}

fn fatal(message: String) -> ! {
    let _ = writeln!(std::io::stderr(), "rustrc: {message}");
    std::process::exit(1);
}

fn run(options: Options) -> i32 {
    let metrics = Arc::new(biometrics::Collector::new());
    rustrc::register_biometrics(&metrics);

    // 1. The state directory lock excludes other rustrcs before we touch the socket or any process.
    let state_dir = if options.no_state_dir {
        None
    } else {
        match StateDir::lock(&options.state_dir) {
            Ok(state_dir) => Some(state_dir),
            Err(err) => {
                let _ = writeln!(std::io::stderr(), "rustrc: {}: {err:?}", options.state_dir);
                return 1;
            }
        }
    };

    // 2. The control socket:  a stale one is replaced, a live one means another rustrc owns it.
    let slot = Arc::new(OnceLock::new());
    let server = if options.no_control_sock {
        None
    } else {
        let adapter = UnixSockAdapter {
            pid1: Arc::clone(&slot),
            metrics: Arc::clone(&metrics),
        };
        match unix_sock::Server::new(Path::from(options.control_sock.as_str()), adapter) {
            Ok(server) => Some(server),
            Err(err) => {
                let _ = writeln!(std::io::stderr(), "rustrc: {}: {err}", options.control_sock);
                return 1;
            }
        }
    };
    let remove_socket = || {
        if !options.no_control_sock {
            let _ = std::fs::remove_file(&options.control_sock);
        }
    };

    // 3. Fence anything a predecessor left running, then start services.
    let pid1_options = Pid1Options {
        rc_conf_path: options.rc_conf_path.clone(),
        rc_d_path: options.rc_d_path.clone(),
        stub_timeout_ms: options.stub_timeout_ms,
        max_stop_timeout_ms: options.max_stop_timeout_ms,
        // Orphans are the init half's job (see rustrc::init); the supervisor never reaps them.
        reap_orphans: false,
        child_subreaper: false,
        ..Pid1Options::default()
    };
    let mut pid1 = match Pid1::with_state_dir(pid1_options, state_dir) {
        Ok(pid1) => Arc::new(pid1),
        Err(err) => {
            let _ = writeln!(std::io::stderr(), "rustrc: {err:?}");
            remove_socket();
            return 1;
        }
    };
    let _ = slot.set(Arc::clone(&pid1));
    drop(slot);

    // 4. Signals:  TERM/INT/QUIT shut down, HUP reloads, everything else is ignored.
    let context = unix_sock::Context::new().expect("context should create");
    let signal_pid1 = Arc::downgrade(&pid1);
    let signal_context = context.clone();
    let signal_running = Arc::new(AtomicBool::new(true));
    let signal_running_thread = Arc::clone(&signal_running);
    let signal = std::thread::spawn(move || {
        loop {
            let signal = minimal_signals::wait(minimal_signals::SignalSet::new().fill());
            if !signal_running_thread.load(Ordering::Acquire) {
                break;
            }
            let Some(pid1) = signal_pid1.upgrade() else {
                break;
            };
            match signal {
                Some(minimal_signals::SIGTERM)
                | Some(minimal_signals::SIGINT)
                | Some(minimal_signals::SIGQUIT) => {
                    indicio::clue!(rustrc::COLLECTOR, indicio::INFO, {
                        signal: signal.map(|s| s.to_string()).unwrap_or_default(),
                        action: "shutdown",
                    });
                    pid1.begin_shutdown();
                    signal_context.cancel();
                    break;
                }
                Some(minimal_signals::SIGHUP) => match pid1.reload_with_plan() {
                    Ok(plan) => {
                        indicio::clue!(rustrc::COLLECTOR, indicio::INFO, {
                            signal: "SIGHUP",
                            reload: plan.to_string(),
                        });
                    }
                    Err(err) => {
                        indicio::clue!(rustrc::COLLECTOR, indicio::ERROR, {
                            signal: "SIGHUP",
                            reload: false,
                            error: indicio::Value::from(&err),
                        });
                    }
                },
                Some(minimal_signals::SIGCHLD) | None => {}
                Some(other) => {
                    indicio::clue!(rustrc::COLLECTOR, indicio::DEBUG, {
                        signal: other.to_string(),
                        ignored: true,
                    });
                }
            }
        }
    });

    // 5. Serve until a shutdown signal (or a server failure) cancels the context.
    let mut code = 0;
    if let Some(mut server) = server {
        if let Err(err) = server.serve(&context) {
            indicio::clue!(rustrc::COLLECTOR, indicio::ERROR, {
                control_sock: options.control_sock.as_str(),
                error: format!("{err:?}"),
            });
            pid1.begin_shutdown();
            code = 1;
        }
    } else {
        while !context.canceled() {
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    // 6. Cleanup.
    signal_running.store(false, Ordering::Release);
    // SAFETY(rescrv): kill observes only integer arguments.  Wakes the signal thread if it is
    // still waiting; SIGCHLD is otherwise ignored.
    let _ = unsafe { libc::kill(libc::getpid(), libc::SIGCHLD) };
    signal.join().unwrap();
    remove_socket();

    // NOTE(rescrv):  This is a spin loop because there's no good way to synchronize this simply.
    // It shouldn't spin for more than a few times.
    let pid1 = {
        loop {
            break match Arc::try_unwrap(pid1) {
                Ok(pid1) => pid1,
                Err(p) => {
                    pid1 = p;
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                }
            };
        }
    };
    if let Err(err) = pid1.shutdown() {
        indicio::clue!(rustrc::COLLECTOR, indicio::ERROR, {
            shutdown: false,
            error: indicio::Value::from(&err),
        });
        code = 1;
    }
    code
}
