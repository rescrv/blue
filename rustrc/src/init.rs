//! Running rustrc as an init process, split in two.
//!
//! An init process reaps every orphan, which means calling waitpid on children it did not spawn.
//! Doing that inside the supervisor races every other fork-and-wait in the process, and a panic in
//! a multi-threaded supervisor that is PID 1 takes the container down with an immediate SIGKILL to
//! every service.  So [split] forks before any thread exists.  The parent becomes a small,
//! single-threaded init that only reaps orphans and forwards signals.  The child returns from
//! [split] and runs the supervisor as an ordinary process.
//!
//! When the supervisor exits, the init sends SIGTERM to whatever is still running (orphans the
//! supervisor left behind, normally nothing), waits up to a grace period while reaping, sends
//! SIGKILL to what remains, and exits with the supervisor's status (128 + signal if it was killed).
//!
//! As PID 1, the init signals everything in its namespace with `kill(-1)`.  As a Linux child
//! subreaper (`--container-init` outside PID 1), it signals the children the kernel reparented to
//! it, and their process groups.

use std::time::{Duration, Instant};

/// What the init half does once the supervisor is dead and the leftovers are reaped.  Called once,
/// with the supervisor's exit code (128 + signal if one killed it); must not return.
///
/// The init half calls it single-threaded, with every signal still blocked and no rustrc state
/// alive, so plain syscalls are the natural body.  It is a function pointer, not a closure,
/// because nothing may be captured into the init half.
pub type Exit = fn(code: i32) -> !;

/// Split into init and supervisor.  Must be called before any thread is spawned, with every signal
/// blocked.  Returns in the supervisor.  In the init, never returns.
pub fn split(grace: Duration) -> std::io::Result<()> {
    split_exiting(grace, std::process::exit)
}

/// [split], with the init half's ending owned by the embedder:  once the supervisor is dead and
/// whatever it left behind has been TERMed, reaped for up to `grace`, KILLed, and reaped for up to
/// 5s more, the init half calls `exit` with the supervisor's exit code instead of
/// [std::process::exit] doing it.  The default ending is right in a container, where a PID 1 exit
/// ends the machine; an embedder that is PID 1 of a virtual machine, whose exit would panic the
/// kernel instead, passes a hook that powers off there.
pub fn split_exiting(grace: Duration, exit: Exit) -> std::io::Result<()> {
    // SAFETY(rescrv): getpid cannot fail.
    let parent = unsafe { libc::getpid() };
    let is_pid1 = parent == 1;
    if !is_pid1 {
        become_subreaper()?;
    }
    // SAFETY(rescrv): the caller guarantees the process is single-threaded, so the child is a
    // faithful copy and may continue running arbitrary Rust.
    let child = unsafe { libc::fork() };
    if child < 0 {
        return Err(std::io::Error::last_os_error());
    }
    if child == 0 {
        die_with_parent(parent);
        return Ok(());
    }
    let status = supervise(child);
    teardown(is_pid1, grace);
    exit(exit_code(status));
}

/// Forward signals to the supervisor and reap everything until the supervisor exits.
fn supervise(child: libc::pid_t) -> libc::c_int {
    loop {
        if let Some(status) = reap_all(child) {
            return status;
        }
        match minimal_signals::wait(minimal_signals::SignalSet::new().fill()) {
            Some(minimal_signals::SIGCHLD) | None => {}
            Some(signal) => {
                // SAFETY(rescrv): kill observes only integer arguments; child is unreaped.
                unsafe {
                    libc::kill(child, signal.into_i32());
                }
            }
        }
    }
}

/// Reap every exited child.  Returns the supervisor's status if it was among them.
fn reap_all(supervisor: libc::pid_t) -> Option<libc::c_int> {
    let mut result = None;
    loop {
        let mut status = 0;
        // SAFETY(rescrv): waitpid observes only the status pointer.
        let pid = unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) };
        if pid <= 0 {
            if pid < 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            return result;
        }
        if pid == supervisor {
            result = Some(status);
        }
    }
}

/// Stop whatever the supervisor left behind:  SIGTERM, reap for up to `grace`, SIGKILL, reap.
fn teardown(is_pid1: bool, grace: Duration) {
    let mut signaled = Vec::new();
    let deadline = Instant::now() + grace;
    while !reaped_everything() && Instant::now() < deadline {
        signal_leftovers(is_pid1, libc::SIGTERM, &mut signaled);
        std::thread::sleep(Duration::from_millis(20));
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut signaled = Vec::new();
    while !reaped_everything() && Instant::now() < deadline {
        signal_leftovers(is_pid1, libc::SIGKILL, &mut signaled);
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Reap what has exited.  True once there are no children left.
fn reaped_everything() -> bool {
    loop {
        let mut status = 0;
        // SAFETY(rescrv): waitpid observes only the status pointer.
        let pid = unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) };
        if pid > 0 {
            continue;
        }
        let err = std::io::Error::last_os_error();
        if pid < 0 && err.raw_os_error() == Some(libc::EINTR) {
            continue;
        }
        return pid < 0 && err.raw_os_error() == Some(libc::ECHILD);
    }
}

/// Send `signal` to each leftover once.  Leftovers appear over time as the processes they belonged
/// to die, so this is called repeatedly and remembers whom it has signaled.
fn signal_leftovers(is_pid1: bool, signal: libc::c_int, signaled: &mut Vec<libc::pid_t>) {
    if is_pid1 {
        if signaled.is_empty() {
            // SAFETY(rescrv): kill observes only integer arguments.  As PID 1, -1 means every
            // process in the namespace except ourselves.
            unsafe {
                libc::kill(-1, signal);
            }
            signaled.push(-1);
        }
        return;
    }
    for pid in our_children() {
        if signaled.contains(&pid) {
            continue;
        }
        signaled.push(pid);
        // SAFETY(rescrv): kill observes only integer arguments.  pid is our unreaped child, so
        // neither it nor a group it leads can have been recycled.
        unsafe {
            libc::kill(-pid, signal);
            libc::kill(pid, signal);
        }
    }
}

#[cfg(target_os = "linux")]
fn become_subreaper() -> std::io::Result<()> {
    // SAFETY(rescrv): prctl observes only integer arguments.
    if unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn become_subreaper() -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "container init outside PID 1 needs a Linux child subreaper",
    ))
}

/// In the supervisor:  receive SIGTERM (a graceful shutdown) if the init dies.
#[cfg(target_os = "linux")]
fn die_with_parent(parent: libc::pid_t) {
    // SAFETY(rescrv): prctl and getppid observe only integer arguments.
    unsafe {
        libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM, 0, 0, 0);
        if libc::getppid() != parent {
            // The init died before the prctl took effect.
            libc::raise(libc::SIGTERM);
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn die_with_parent(_: libc::pid_t) {}

/// Our live children, from /proc.  Only a subreaper needs this, and only Linux has subreapers.
#[cfg(target_os = "linux")]
fn our_children() -> Vec<libc::pid_t> {
    let me = std::process::id().to_string();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return vec![];
    };
    let mut children = vec![];
    for entry in entries.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<libc::pid_t>() else {
            continue;
        };
        let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        let Some((_, rest)) = stat.rsplit_once(')') else {
            continue;
        };
        let fields = rest.split_whitespace().collect::<Vec<_>>();
        if fields.get(1) == Some(&me.as_str()) && fields.first() != Some(&"Z") {
            children.push(pid);
        }
    }
    children
}

#[cfg(not(target_os = "linux"))]
fn our_children() -> Vec<libc::pid_t> {
    vec![]
}

fn exit_code(status: libc::c_int) -> i32 {
    if libc::WIFEXITED(status) {
        libc::WEXITSTATUS(status)
    } else if libc::WIFSIGNALED(status) {
        128 + libc::WTERMSIG(status)
    } else {
        1
    }
}
