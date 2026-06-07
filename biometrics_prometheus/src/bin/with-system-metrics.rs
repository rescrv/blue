use std::os::unix::process::ExitStatusExt;
use std::process::{self, Command};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use arrrg::CommandLine;
use biometrics_prometheus::{Emitter, Options};
use biometrics_sys::BiometricsSys;
use libc;

const USAGE: &str = "USAGE: with-system-metrics [--poll-in-seconds=<seconds>] <command> [args...]";
const DEFAULT_POLL_SECONDS: u64 = 1;

#[derive(Clone, Debug, Default, Eq, PartialEq, arrrg_derive::CommandLine)]
struct PollOptions {
    #[arrrg(optional, "Polling interval in seconds.")]
    poll_in_seconds: Option<u64>,
}

fn main() {
    let (options, argv) = PollOptions::from_command_line_relaxed(USAGE);
    if argv.is_empty() {
        eprintln!("command is required");
        process::exit(2);
    }
    let mut command = Command::new(&argv[0]);
    command.args(&argv[1..]);

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => {
            eprintln!("failed to start command: {err}");
            process::exit(2);
        }
    };
    let poll_in_seconds = options.poll_in_seconds.unwrap_or(DEFAULT_POLL_SECONDS);
    if poll_in_seconds == 0 {
        eprintln!("--poll-in-seconds must be a positive integer");
        process::exit(2);
    }
    let interval = Duration::from_secs(poll_in_seconds);
    let (stop_tx, stop_rx) = mpsc::channel();
    let monitor = thread::spawn({
        let interval = interval;
        move || monitor_child_rusage(stop_rx, interval)
    });
    let status = match child.wait() {
        Ok(status) => status,
        Err(err) => {
            let _ = stop_tx.send(());
            eprintln!("failed to wait on command: {err}");
            process::exit(2);
        }
    };
    let _ = stop_tx.send(());
    match monitor.join() {
        Ok(Ok(())) => {}
        Ok(Err(err)) => {
            eprintln!("failed to sample child usage: {err}");
            process::exit(2);
        }
        Err(err) => {
            eprintln!("monitor thread failed: {err:?}");
            process::exit(2);
        }
    }

    process::exit(child_exit_code(status));
}

fn monitor_child_rusage(
    stop_rx: mpsc::Receiver<()>,
    interval: Duration,
) -> Result<(), std::io::Error> {
    let baseline = children_rusage()?;
    let mut emitter = Emitter::new(Options::default());
    let mut sys = BiometricsSys::new();

    let emit_sample = |baseline: &libc::rusage,
                       emitter: &mut Emitter,
                       sys: &mut BiometricsSys|
     -> Result<(), std::io::Error> {
        let usage = children_rusage()?;
        let delta = rusage_delta(usage, *baseline);
        let now = now_millis_since_epoch();
        sys.emit_with_rusage(emitter, now, delta);
        emitter.flush()
    };

    loop {
        match stop_rx.recv_timeout(interval) {
            Ok(()) => {
                emit_sample(&baseline, &mut emitter, &mut sys)?;
                break;
            }
            Err(RecvTimeoutError::Timeout) => {
                emit_sample(&baseline, &mut emitter, &mut sys)?;
            }
            Err(RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    Ok(())
}

fn children_rusage() -> Result<libc::rusage, std::io::Error> {
    let mut usage = zero_rusage();
    if unsafe { libc::getrusage(libc::RUSAGE_CHILDREN, &mut usage) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(usage)
}

fn rusage_delta(current: libc::rusage, baseline: libc::rusage) -> libc::rusage {
    let ru_utime = timeval_delta(current.ru_utime, baseline.ru_utime);
    let ru_stime = timeval_delta(current.ru_stime, baseline.ru_stime);
    libc::rusage {
        ru_utime,
        ru_stime,
        ru_maxrss: current.ru_maxrss - baseline.ru_maxrss,
        ru_ixrss: current.ru_ixrss - baseline.ru_ixrss,
        ru_idrss: current.ru_idrss - baseline.ru_idrss,
        ru_isrss: current.ru_isrss - baseline.ru_isrss,
        ru_minflt: current.ru_minflt - baseline.ru_minflt,
        ru_majflt: current.ru_majflt - baseline.ru_majflt,
        ru_nswap: current.ru_nswap - baseline.ru_nswap,
        ru_inblock: current.ru_inblock - baseline.ru_inblock,
        ru_oublock: current.ru_oublock - baseline.ru_oublock,
        ru_msgsnd: current.ru_msgsnd - baseline.ru_msgsnd,
        ru_msgrcv: current.ru_msgrcv - baseline.ru_msgrcv,
        ru_nsignals: current.ru_nsignals - baseline.ru_nsignals,
        ru_nvcsw: current.ru_nvcsw - baseline.ru_nvcsw,
        ru_nivcsw: current.ru_nivcsw - baseline.ru_nivcsw,
    }
}

fn timeval_delta(current: libc::timeval, baseline: libc::timeval) -> libc::timeval {
    let mut sec = current.tv_sec - baseline.tv_sec;
    let mut usec = current.tv_usec - baseline.tv_usec;
    if usec < 0 {
        sec -= 1;
        usec += 1_000_000;
    }
    libc::timeval {
        tv_sec: sec,
        tv_usec: usec,
    }
}

fn child_exit_code(status: process::ExitStatus) -> i32 {
    status
        .code()
        .or_else(|| status.signal().map(|signal| 128 + signal))
        .unwrap_or(129)
}

fn now_millis_since_epoch() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(now) => now.as_millis().try_into().unwrap_or(u64::MAX),
        Err(_) => 0,
    }
}

fn zero_rusage() -> libc::rusage {
    libc::rusage {
        ru_utime: libc::timeval {
            tv_sec: 0,
            tv_usec: 0,
        },
        ru_stime: libc::timeval {
            tv_sec: 0,
            tv_usec: 0,
        },
        ru_maxrss: 0,
        ru_ixrss: 0,
        ru_idrss: 0,
        ru_isrss: 0,
        ru_minflt: 0,
        ru_majflt: 0,
        ru_nswap: 0,
        ru_inblock: 0,
        ru_oublock: 0,
        ru_msgsnd: 0,
        ru_msgrcv: 0,
        ru_nsignals: 0,
        ru_nvcsw: 0,
        ru_nivcsw: 0,
    }
}
