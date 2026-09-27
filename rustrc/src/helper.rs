//! Running an rc.d stub's `rcvar` verb.
//!
//! rc_conf's `stub_rcvars` runs the stub with `Command::output`, which has no deadline, and waits
//! for it with `waitpid(pid)`.  rustrc cannot use it for two reasons.  A stub that ignores `rcvar`
//! (say, a shell script that execs its daemon whatever its arguments) never returns.  And when
//! rustrc reaps orphans as an init process, the reaper and `waitpid(pid)` race for the same zombie.
//!
//! Here the stub runs in its own process group with a deadline, and its pid is registered with
//! [`is_helper`] from before posix_spawn returns until after it is reaped, so the orphan reaper
//! leaves it alone.

use std::collections::HashSet;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use utf8path::Path;

use crate::Error;

static HELPERS: Mutex<Option<HashSet<libc::pid_t>>> = Mutex::new(None);

/// The most output a stub's `rcvar` may produce.
const MAX_OUTPUT: usize = 1 << 20;

static STUB_TIMEOUT: biometrics::Counter = biometrics::Counter::new("rustrc.stub.timeout");

pub(crate) fn register_biometrics(collector: &biometrics::Collector) {
    collector.register_counter(&STUB_TIMEOUT);
}

/// True if `pid` is a helper process rustrc spawned and will reap itself.
pub(crate) fn is_helper(pid: libc::pid_t) -> bool {
    HELPERS
        .lock()
        .unwrap()
        .as_ref()
        .is_some_and(|h| h.contains(&pid))
}

/// Run `path rcvar` as `service` and return the fully-qualified rc variables it reads.
///
/// The environment matches rc_conf's `stub_rcvars`.  If the stub has not exited and closed its
/// output within `timeout`, its process group is killed and this returns an error.
pub(crate) fn stub_rcvars(
    service: &str,
    path: &Path,
    timeout: Duration,
) -> Result<Vec<String>, Error> {
    let mut cmd = Command::new(path.as_str());
    cmd.arg("rcvar")
        .env_clear()
        .env("RCVAR_ARGV0", rc_conf::var_name_from_service(service))
        .envs(
            std::env::vars()
                .filter(|(key, _)| matches!(key.as_str(), "PATH" | "TERM" | "TZ" | "LANG")),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    // rustrc blocks every signal for its sigwait thread; give the stub the empty mask services get.
    // SAFETY(rescrv): the hook runs between fork and exec and calls only async-signal-safe
    // functions on stack storage.
    unsafe {
        cmd.pre_exec(|| {
            let mut empty = std::mem::MaybeUninit::<libc::sigset_t>::uninit();
            libc::sigemptyset(empty.as_mut_ptr());
            libc::pthread_sigmask(libc::SIG_SETMASK, empty.as_ptr(), std::ptr::null_mut());
            Ok(())
        });
    }
    let mut child = {
        // Hold the registry across spawn so the reaper cannot observe the zombie of a helper that
        // exits before we record it.
        let mut helpers = HELPERS.lock().unwrap();
        let child = cmd.spawn()?;
        helpers
            .get_or_insert_with(HashSet::new)
            .insert(child.id() as libc::pid_t);
        child
    };
    let pid = child.id() as libc::pid_t;
    let collected = collect(&mut child, Instant::now() + timeout);
    if collected.is_err() {
        // SAFETY(rescrv): kill observes only integer arguments.  The helper has not been reaped,
        // so its group id is still ours.
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
    }
    // Reap.  After a successful collect this returns the status try_wait already cached.
    let _ = child.wait();
    if let Some(helpers) = HELPERS.lock().unwrap().as_mut() {
        helpers.remove(&pid);
    }
    let (stdout, stderr, status) = match collected {
        Ok(collected) => collected,
        Err(Collect::TimedOut) => {
            STUB_TIMEOUT.click();
            return Err(Error::ServiceError(format!(
                "{path} rcvar did not finish within {timeout:?}; the stub must answer `rcvar` \
                 without running the service"
            )));
        }
        Err(Collect::TooLarge) => {
            return Err(Error::ServiceError(format!(
                "{path} rcvar wrote more than {MAX_OUTPUT} bytes"
            )));
        }
        Err(Collect::Io(err)) => return Err(err.into()),
    };
    if !status.success() {
        let stderr = String::from_utf8_lossy(&stderr);
        let stderr = stderr.trim();
        return Err(Error::RcConf(rc_conf::Error::InvalidInvocation {
            message: if stderr.is_empty() {
                "rcvar command failed".to_string()
            } else {
                format!("rcvar command failed: {stderr}")
            },
        }));
    }
    let keys = String::from_utf8(stdout)
        .map_err(|err| Error::RcConf(rc_conf::Error::FromUtf8Error(err)))?;
    Ok(keys.split_whitespace().map(String::from).collect())
}

enum Collect {
    TimedOut,
    TooLarge,
    Io(std::io::Error),
}

impl From<std::io::Error> for Collect {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

/// Read stdout and stderr to EOF and wait for exit, all before `deadline`.
fn collect(
    child: &mut Child,
    deadline: Instant,
) -> Result<(Vec<u8>, Vec<u8>, ExitStatus), Collect> {
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let mut out = vec![];
    let mut err = vec![];
    while stdout.is_some() || stderr.is_some() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(Collect::TimedOut);
        }
        let mut pfd = [
            libc::pollfd {
                fd: stdout.as_ref().map_or(-1, |s| s.as_raw_fd()),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: stderr.as_ref().map_or(-1, |s| s.as_raw_fd()),
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        let millis = remaining.as_millis().clamp(1, libc::c_int::MAX as u128) as libc::c_int;
        // SAFETY(rescrv): pfd is a valid array of two pollfds; negative fds are ignored.
        if unsafe { libc::poll(pfd.as_mut_ptr(), 2, millis) } < 0 {
            let e = std::io::Error::last_os_error();
            if e.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            return Err(e.into());
        }
        if pfd[0].revents != 0 && drain(&mut stdout, &mut out)? {
            stdout = None;
        }
        if pfd[1].revents != 0 && drain(&mut stderr, &mut err)? {
            stderr = None;
        }
        if out.len() + err.len() > MAX_OUTPUT {
            return Err(Collect::TooLarge);
        }
    }
    // Output is closed; the stub is exiting or has handed its pipes to nobody.
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok((out, err, status));
        }
        if Instant::now() >= deadline {
            return Err(Collect::TimedOut);
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// Read what is available.  True at EOF.
fn drain<R: Read>(reader: &mut Option<R>, buf: &mut Vec<u8>) -> Result<bool, Collect> {
    let Some(r) = reader.as_mut() else {
        return Ok(true);
    };
    let mut chunk = [0u8; 4096];
    loop {
        match r.read(&mut chunk) {
            Ok(0) => return Ok(true),
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                return Ok(false);
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        }
    }
}
