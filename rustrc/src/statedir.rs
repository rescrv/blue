//! What rustrc started, on disk, so a successor can fence what a dead rustrc left running.
//!
//! When rustrc dies without shutting down (SIGKILL, OOM, a panic), its services keep running,
//! reparented elsewhere.  A new rustrc would start a second copy of each.  To prevent that, rustrc
//! writes a record for every process it spawns (pid, service, and the process's start time) into
//! the state directory and removes it after reaping.  On startup, any record whose process still
//! exists with the same start time belongs to a predecessor:  its process group gets SIGTERM, then
//! SIGKILL after the service's stop timeout, before anything new starts.
//!
//! The directory is also a lock:  an exclusive flock on `lock` held for the life of the process,
//! released by the kernel however rustrc exits.  A second rustrc on the same directory refuses to
//! start rather than fencing a live sibling's services.
//!
//! The start time is what makes a recorded pid trustworthy after the pid may have been recycled.
//! It is read from /proc on Linux and proc_pidinfo on macOS; elsewhere records carry none and are
//! never acted on.  Fencing signals a group only while its recorded leader is still alive, so if a
//! leftover leader exits during the grace period, processes it left in its group are not chased:
//! with the leader gone the group id cannot be verified.  Keep the directory on local disk; records
//! are written with the state lock held.

use std::collections::HashMap;
use std::fs::{DirBuilder, File, OpenOptions};
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use indicio::{ERROR, INFO, WARNING, clue};

use crate::{COLLECTOR, Error};

static FENCED: biometrics::Counter = biometrics::Counter::new("rustrc.state.fenced");
static RECORD_ERROR: biometrics::Counter = biometrics::Counter::new("rustrc.state.record_error");

pub(crate) fn register_biometrics(collector: &biometrics::Collector) {
    collector.register_counter(&FENCED);
    collector.register_counter(&RECORD_ERROR);
}

/// A locked state directory.  The lock is released when this is dropped or the process exits.
#[derive(Debug)]
pub struct StateDir {
    path: PathBuf,
    _lock: File,
}

impl StateDir {
    /// Create `path` (mode 0700) if needed and take its lock.  Fails with
    /// [Error::AlreadyRunning] if another process holds it.
    pub fn lock(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref().to_path_buf();
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(path.join("executions"))?;
        // std opens files close-on-exec, so services never inherit the lock.
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .mode(0o600)
            .open(path.join("lock"))?;
        // SAFETY(rescrv): flock observes only the fd and integer flags.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } < 0 {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EWOULDBLOCK) {
                return Err(Error::AlreadyRunning(format!(
                    "another rustrc holds {}",
                    path.display()
                )));
            }
            return Err(err.into());
        }
        Ok(Self { path, _lock: lock })
    }

    /// The directory.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Record a freshly spawned process.  Call before the process can be reaped.  Returns the path
    /// to pass to [StateDir::forget], or None if the record could not be written (logged).
    pub(crate) fn record(
        &self,
        name: &str,
        service: &str,
        pid: libc::pid_t,
        stop_timeout: Duration,
    ) -> Option<PathBuf> {
        let Some(start) = process_start(pid) else {
            // The platform can't identify processes; a record could never be acted on.
            return None;
        };
        let path = self.path.join("executions").join(name);
        let tmp = self.path.join("executions").join(format!(".{name}.tmp"));
        let contents = format!(
            "service={}\npid={pid}\nstart={start}\nstop_timeout_ms={}\n",
            service.replace('\n', " "),
            stop_timeout.as_millis()
        );
        let written = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(&tmp)
            .and_then(|mut f| f.write_all(contents.as_bytes()))
            .and_then(|()| std::fs::rename(&tmp, &path));
        match written {
            Ok(()) => Some(path),
            Err(err) => {
                RECORD_ERROR.click();
                let _ = std::fs::remove_file(&tmp);
                clue!(COLLECTOR, ERROR, {
                    state_record: {
                        service: service,
                        pid: pid,
                    },
                    error: format!("{err:?}"),
                });
                None
            }
        }
    }

    /// Remove a record written by [StateDir::record].
    pub(crate) fn forget(path: &Path) {
        if let Err(err) = std::fs::remove_file(path)
            && err.kind() != std::io::ErrorKind::NotFound
        {
            RECORD_ERROR.click();
            clue!(COLLECTOR, ERROR, {
                state_forget: path.to_string_lossy().into_owned(),
                error: format!("{err:?}"),
            });
        }
    }

    /// Stop every process a predecessor recorded that is still running, then clear all records.
    /// Returns the services that were fenced.
    pub(crate) fn fence(&self) -> Result<Vec<String>, Error> {
        let dir = self.path.join("executions");
        let mut live = vec![];
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            let record = std::fs::read_to_string(&path)
                .ok()
                .and_then(|contents| Record::parse(&contents));
            let _ = std::fs::remove_file(&path);
            let Some(record) = record else {
                continue;
            };
            if process_start(record.pid).as_deref() == Some(record.start.as_str()) {
                live.push(record);
            }
        }
        if live.is_empty() {
            return Ok(vec![]);
        }
        let now = Instant::now();
        let mut deadlines = HashMap::new();
        for record in live.iter() {
            clue!(COLLECTOR, WARNING, {
                fence: {
                    service: record.service.as_str(),
                    pid: record.pid,
                    stop_timeout: format!("{:?}", record.stop_timeout),
                },
            });
            FENCED.click();
            signal_group(record.pid, libc::SIGTERM);
            deadlines.insert(record.pid, now + record.stop_timeout);
        }
        let alive = |r: &Record| process_start(r.pid).as_deref() == Some(r.start.as_str());
        let mut remaining = live.clone();
        let kill_deadline = loop {
            remaining.retain(alive);
            let now = Instant::now();
            for record in remaining.iter() {
                if deadlines.get(&record.pid).is_some_and(|d| *d <= now) {
                    // Signal while the leader is verified; its group id is still its own.
                    signal_group(record.pid, libc::SIGKILL);
                }
            }
            if remaining.is_empty() {
                break None;
            }
            if deadlines.values().all(|d| *d <= now) {
                break Some(now + Duration::from_secs(5));
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        if let Some(kill_deadline) = kill_deadline {
            while !remaining.is_empty() && Instant::now() < kill_deadline {
                remaining.retain(alive);
                std::thread::sleep(Duration::from_millis(20));
            }
            for record in remaining.iter() {
                clue!(COLLECTOR, ERROR, {
                    fence: {
                        service: record.service.as_str(),
                        pid: record.pid,
                        survived_sigkill: true,
                    },
                });
            }
        }
        let fenced = live.into_iter().map(|r| r.service).collect::<Vec<_>>();
        clue!(COLLECTOR, INFO, {
            fenced: indicio::Value::from(fenced.clone()),
        });
        Ok(fenced)
    }
}

#[derive(Clone, Debug)]
struct Record {
    service: String,
    pid: libc::pid_t,
    start: String,
    stop_timeout: Duration,
}

impl Record {
    fn parse(contents: &str) -> Option<Self> {
        let mut service = None;
        let mut pid = None;
        let mut start = None;
        let mut stop_timeout = None;
        for line in contents.lines() {
            match line.split_once('=')? {
                ("service", v) => service = Some(v.to_string()),
                ("pid", v) => pid = v.parse::<libc::pid_t>().ok().filter(|p| *p > 0),
                ("start", v) => start = Some(v.to_string()).filter(|s| !s.is_empty()),
                ("stop_timeout_ms", v) => stop_timeout = v.parse().ok().map(Duration::from_millis),
                _ => {}
            }
        }
        Some(Self {
            service: service?,
            pid: pid?,
            start: start?,
            stop_timeout: stop_timeout?,
        })
    }
}

/// Signal `pid`'s process group, or `pid` itself if it no longer leads one.
fn signal_group(pid: libc::pid_t, signal: libc::c_int) {
    // SAFETY(rescrv): kill observes only integer arguments.
    unsafe {
        if libc::kill(-pid, signal) < 0 {
            libc::kill(pid, signal);
        }
    }
}

/// An identifier for the process `pid` that changes if the pid is recycled:  its start time.  None
/// if there is no such process, it is a zombie, or the platform cannot say.
#[cfg(target_os = "linux")]
pub(crate) fn process_start(pid: libc::pid_t) -> Option<String> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // The command name may contain spaces and parentheses; fields resume after the last ')'.
    let (_, rest) = stat.rsplit_once(')')?;
    let fields = rest.split_whitespace().collect::<Vec<_>>();
    // fields[0] is field 3 (state); starttime is field 22.
    if fields.first() == Some(&"Z") || fields.first() == Some(&"X") {
        return None;
    }
    fields.get(19).map(|s| s.to_string())
}

#[cfg(target_os = "macos")]
pub(crate) fn process_start(pid: libc::pid_t) -> Option<String> {
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    // SAFETY(rescrv): proc_pidinfo writes at most `size` bytes into info.
    let rc = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr() as *mut libc::c_void,
            size,
        )
    };
    if rc != size {
        return None;
    }
    // SAFETY(rescrv): proc_pidinfo filled the whole struct.
    let info = unsafe { info.assume_init() };
    Some(format!(
        "{}.{:06}",
        info.pbi_start_tvsec, info.pbi_start_tvusec
    ))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub(crate) fn process_start(_: libc::pid_t) -> Option<String> {
    None
}
