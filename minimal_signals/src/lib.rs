//! Work with Unix signals using small synchronous and callback-based APIs.
//!
//! The crate keeps the original [`SignalSet`] and [`wait`] building blocks for
//! programs that want to dedicate a thread to `sigwait(3)`.  It also provides a
//! Ctrl-C-style [`set_handler`] API for applications that just need a closure to
//! run when a signal arrives.

#![deny(missing_docs)]

use std::io::{self, Read};
use std::mem::MaybeUninit;
use std::os::fd::FromRawFd;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

////////////////////////////////////////////// Signals /////////////////////////////////////////////

/// Identifies a known Unix signal number.
#[derive(Clone, Copy, Default, Eq, PartialEq, Hash)]
pub struct Signal(i32);

impl Signal {
    /// Convert a raw platform signal number into a known signal.
    pub fn from_i32(signal: i32) -> Option<Self> {
        KNOWN_SIGNALS
            .iter()
            .find_map(|(known, _)| (known.0 == signal).then_some(*known))
    }

    /// Return the raw platform signal number.
    pub fn as_i32(self) -> i32 {
        self.0
    }

    /// Convert this signal into its raw platform signal number.
    pub fn into_i32(self) -> i32 {
        self.0
    }

    /// Return the conventional signal name, such as `SIGINT`.
    pub fn name(self) -> &'static str {
        KNOWN_SIGNALS
            .iter()
            .find_map(|(known, name)| (known.0 == self.0).then_some(*name))
            .unwrap_or("UNKNOWN")
    }

    /// Return true when POSIX allows a handler to be installed for this signal.
    pub fn is_catchable(self) -> bool {
        self != SIGKILL && self != SIGSTOP
    }

    /// Iterate over the signals this crate knows by name.
    pub fn known() -> impl Iterator<Item = Signal> {
        KNOWN_SIGNALS.iter().map(|(signal, _)| *signal)
    }
}

macro_rules! signal_constants {
    ($(#[$doc:meta] $name:ident => $libc_name:ident,)+) => {
        $(
            #[$doc]
            pub const $name: Signal = Signal(libc::$libc_name);
        )+
    };
}

signal_constants! {
    /// Hangup detected on controlling terminal or controlling process death.
    SIGHUP => SIGHUP,
    /// Terminal interrupt, usually Ctrl-C.
    SIGINT => SIGINT,
    /// Terminal quit, usually Ctrl-\.
    SIGQUIT => SIGQUIT,
    /// Illegal instruction.
    SIGILL => SIGILL,
    /// Trace or breakpoint trap.
    SIGTRAP => SIGTRAP,
    /// Process abort.
    SIGABRT => SIGABRT,
    /// Erroneous arithmetic operation.
    SIGFPE => SIGFPE,
    /// Kill signal that cannot be caught or ignored.
    SIGKILL => SIGKILL,
    /// Bus error.
    SIGBUS => SIGBUS,
    /// Invalid memory reference.
    SIGSEGV => SIGSEGV,
    /// Bad system call.
    SIGSYS => SIGSYS,
    /// Write on a pipe with no reader.
    SIGPIPE => SIGPIPE,
    /// Alarm clock.
    SIGALRM => SIGALRM,
    /// Termination request.
    SIGTERM => SIGTERM,
    /// Urgent condition on a socket.
    SIGURG => SIGURG,
    /// Stop signal that cannot be caught or ignored.
    SIGSTOP => SIGSTOP,
    /// Terminal stop, usually Ctrl-Z.
    SIGTSTP => SIGTSTP,
    /// Continue a stopped process.
    SIGCONT => SIGCONT,
    /// Child process stopped or terminated.
    SIGCHLD => SIGCHLD,
    /// Background process attempted to read from a terminal.
    SIGTTIN => SIGTTIN,
    /// Background process attempted to write to a terminal.
    SIGTTOU => SIGTTOU,
    /// I/O is now possible.
    SIGIO => SIGIO,
    /// CPU time limit exceeded.
    SIGXCPU => SIGXCPU,
    /// File size limit exceeded.
    SIGXFSZ => SIGXFSZ,
    /// Virtual alarm clock.
    SIGVTALRM => SIGVTALRM,
    /// Profiling timer expired.
    SIGPROF => SIGPROF,
    /// Window size changed.
    SIGWINCH => SIGWINCH,
    /// User-defined signal 1.
    SIGUSR1 => SIGUSR1,
    /// User-defined signal 2.
    SIGUSR2 => SIGUSR2,
}

const KNOWN_SIGNALS: &[(Signal, &str)] = &[
    (SIGHUP, "SIGHUP"),
    (SIGINT, "SIGINT"),
    (SIGQUIT, "SIGQUIT"),
    (SIGILL, "SIGILL"),
    (SIGTRAP, "SIGTRAP"),
    (SIGABRT, "SIGABRT"),
    (SIGFPE, "SIGFPE"),
    (SIGKILL, "SIGKILL"),
    (SIGBUS, "SIGBUS"),
    (SIGSEGV, "SIGSEGV"),
    (SIGSYS, "SIGSYS"),
    (SIGPIPE, "SIGPIPE"),
    (SIGALRM, "SIGALRM"),
    (SIGTERM, "SIGTERM"),
    (SIGURG, "SIGURG"),
    (SIGSTOP, "SIGSTOP"),
    (SIGTSTP, "SIGTSTP"),
    (SIGCONT, "SIGCONT"),
    (SIGCHLD, "SIGCHLD"),
    (SIGTTIN, "SIGTTIN"),
    (SIGTTOU, "SIGTTOU"),
    (SIGIO, "SIGIO"),
    (SIGXCPU, "SIGXCPU"),
    (SIGXFSZ, "SIGXFSZ"),
    (SIGVTALRM, "SIGVTALRM"),
    (SIGPROF, "SIGPROF"),
    (SIGWINCH, "SIGWINCH"),
    (SIGUSR1, "SIGUSR1"),
    (SIGUSR2, "SIGUSR2"),
];

impl std::fmt::Debug for Signal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::fmt::Display for Signal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

///////////////////////////////////////////// SignalSet ////////////////////////////////////////////

/// Stores a POSIX signal set.
pub struct SignalSet {
    sigset: libc::sigset_t,
}

impl SignalSet {
    /// Construct an empty signal set.
    pub fn new() -> Self {
        let mut none: MaybeUninit<libc::sigset_t> = MaybeUninit::uninit();
        // SAFETY(rescrv): This function cannot fail as we give it a valid place to write and it is
        // assumed to be able to work with uninitialized pointers.
        unsafe {
            libc::sigemptyset(none.as_mut_ptr());
            SignalSet {
                sigset: none.assume_init(),
            }
        }
    }

    /// Construct an empty signal set.
    pub fn empty(mut self) -> Self {
        // SAFETY(rescrv): This function cannot fail as we give it a valid and initialized value.
        unsafe {
            libc::sigemptyset(&mut self.sigset);
        }
        self
    }

    /// Construct a full signal set.
    pub fn all() -> Self {
        Self::new().fill()
    }

    /// Construct a signal set containing `SIGINT`.
    pub fn ctrl_c() -> Self {
        Self::new().add(SIGINT)
    }

    /// Construct a signal set for conventional termination requests.
    pub fn termination() -> Self {
        Self::new().add(SIGHUP).add(SIGINT).add(SIGTERM)
    }

    /// Construct a signal set from an iterator of signals.
    pub fn from_signals(signals: impl IntoIterator<Item = Signal>) -> Self {
        signals.into_iter().collect()
    }

    /// Replace this set with all platform signals.
    pub fn fill(mut self) -> Self {
        // SAFETY(rescrv): This function cannot fail as we give it a valid and initialized value.
        unsafe {
            libc::sigfillset(&mut self.sigset);
        }
        self
    }

    /// Add `signal` to this set.
    #[allow(clippy::should_implement_trait)]
    pub fn add(mut self, signal: Signal) -> Self {
        // SAFETY(rescrv): This function cannot fail as we give it a valid and initialized value.
        unsafe {
            libc::sigaddset(&mut self.sigset, signal.0);
        }
        self
    }

    /// Remove `signal` from this set.
    pub fn remove(self, signal: Signal) -> Self {
        self.del(signal)
    }

    /// Remove `signal` from this set.
    pub fn del(mut self, signal: Signal) -> Self {
        // SAFETY(rescrv): This function cannot fail as we give it a valid and initialized value.
        unsafe {
            libc::sigdelset(&mut self.sigset, signal.0);
        }
        self
    }

    /// Return true when `signal` belongs to this set.
    pub fn contains(&self, signal: Signal) -> bool {
        self.ismember(signal)
    }

    /// Return true when `signal` belongs to this set.
    pub fn ismember(&self, signal: Signal) -> bool {
        // SAFETY(rescrv): This function cannot fail as we give it a valid and initialized value.
        unsafe { libc::sigismember(&self.sigset, signal.0) == 1 }
    }

    /// Iterate over the known signals that belong to this set.
    pub fn iter(&self) -> impl Iterator<Item = Signal> + '_ {
        Signal::known().filter(|signal| self.ismember(*signal))
    }

    /// Replace the calling thread's signal mask with this set.
    ///
    /// This is the historical behavior of the crate's `block` method.  Use
    /// [`SignalSet::block_signals`] to add signals to the current mask.
    pub fn block(&self) -> io::Result<()> {
        self.set_mask()
    }

    /// Replace the calling thread's signal mask with this set.
    pub fn set_mask(&self) -> io::Result<()> {
        // SAFETY(rescrv): We know this is safe because we manipulate the set using libc functions.
        unsafe {
            if libc::sigprocmask(libc::SIG_SETMASK, &self.sigset, std::ptr::null_mut()) != 0 {
                return Err(io::Error::last_os_error());
            }
        };
        Ok(())
    }

    /// Add this set of signals to the calling thread's signal mask.
    pub fn block_signals(&self) -> io::Result<()> {
        self.thread_mask(libc::SIG_BLOCK)
    }

    /// Remove this set of signals from the calling thread's signal mask.
    pub fn unblock_signals(&self) -> io::Result<()> {
        self.thread_mask(libc::SIG_UNBLOCK)
    }

    /// Install a no-op `sigaction(2)` handler for every signal in this set.
    pub fn install(&self) -> io::Result<()> {
        let mut sa: libc::sigaction = unsafe { std::mem::zeroed() };
        sa.sa_sigaction = nop as *const () as usize;
        sa.sa_flags = libc::SA_SIGINFO | libc::SA_RESTART;
        // SAFETY(rescrv): This initializes the mask field before sigaction observes it.
        unsafe {
            libc::sigemptyset(&mut sa.sa_mask);
        }
        for signal in self.iter() {
            // SAFETY(rescrv): We know this is safe because we manipulate the set using libc functions.
            if unsafe { libc::sigaction(signal.0, &sa, std::ptr::null_mut()) } != 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }

    /// Wait synchronously until a signal in this set is pending.
    ///
    /// The relevant signals should be blocked in the threads that should not
    /// receive them asynchronously.
    pub fn wait(&self) -> io::Result<Signal> {
        let mut signal = -1i32;
        // SAFETY(rescrv): We know this is safe because we manipulate the set using libc functions.
        let rc = unsafe { libc::sigwait(&self.sigset, &mut signal) };
        if rc != 0 {
            return Err(io::Error::from_raw_os_error(rc));
        }
        Signal::from_i32(signal).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unknown signal number {signal}"),
            )
        })
    }

    fn thread_mask(&self, how: libc::c_int) -> io::Result<()> {
        // SAFETY(rescrv): We know this is safe because we manipulate the set using libc functions.
        let rc = unsafe { libc::pthread_sigmask(how, &self.sigset, std::ptr::null_mut()) };
        if rc != 0 {
            return Err(io::Error::from_raw_os_error(rc));
        }
        Ok(())
    }
}

impl Clone for SignalSet {
    fn clone(&self) -> Self {
        let mut sigset = MaybeUninit::<libc::sigset_t>::uninit();
        // SAFETY(rescrv): sigset_t is an opaque C value that can be copied byte-for-byte.
        unsafe {
            std::ptr::copy_nonoverlapping(&self.sigset, sigset.as_mut_ptr(), 1);
            Self {
                sigset: sigset.assume_init(),
            }
        }
    }
}

impl Default for SignalSet {
    fn default() -> Self {
        Self::new()
    }
}

impl Extend<Signal> for SignalSet {
    fn extend<T: IntoIterator<Item = Signal>>(&mut self, iter: T) {
        for signal in iter {
            // SAFETY(rescrv): This function cannot fail as we give it a valid and initialized value.
            unsafe {
                libc::sigaddset(&mut self.sigset, signal.0);
            }
        }
    }
}

impl FromIterator<Signal> for SignalSet {
    fn from_iter<T: IntoIterator<Item = Signal>>(iter: T) -> Self {
        let mut set = Self::new();
        set.extend(iter);
        set
    }
}

impl std::fmt::Debug for SignalSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut signals = self.iter();
        let Some(first) = signals.next() else {
            return f.write_str("()");
        };
        write!(f, "{first}")?;
        for signal in signals {
            write!(f, "|{signal}")?;
        }
        Ok(())
    }
}

impl std::fmt::Display for SignalSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl From<Signal> for SignalSet {
    fn from(s: Signal) -> Self {
        Self::new().add(s)
    }
}

/////////////////////////////////////////////// Error //////////////////////////////////////////////

/// Describes why a callback-based signal handler could not be installed.
#[derive(Debug)]
pub enum Error {
    /// A process-global handler has already been installed.
    AlreadyInstalled,
    /// The requested handler did not include any known signal.
    EmptySignalSet,
    /// The requested handler included a signal that cannot be caught.
    UncatchableSignal(Signal),
    /// The operating system rejected an operation.
    System(io::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyInstalled => f.write_str("signal handler already installed"),
            Self::EmptySignalSet => f.write_str("signal handler requires at least one signal"),
            Self::UncatchableSignal(signal) => {
                write!(f, "cannot install a handler for {signal}")
            }
            Self::System(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::System(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::System(err)
    }
}

//////////////////////////////////////// callback handlers /////////////////////////////////////////

static HANDLER_INSTALLED: AtomicBool = AtomicBool::new(false);
static SIGNAL_WRITE_FD: AtomicI32 = AtomicI32::new(-1);

/// Install a process-global handler for `SIGINT`.
///
/// The closure runs on a background thread, not inside the operating-system
/// signal handler.
///
/// # Errors
///
/// Returns an error when a handler has already been installed or when the
/// operating system rejects the signal setup.
pub fn set_handler<F>(mut handler: F) -> std::result::Result<(), Error>
where
    F: FnMut() + Send + 'static,
{
    set_handler_for(SIGINT, move |_| handler())
}

/// Install a process-global handler for each signal in `signals`.
///
/// The closure receives the signal that arrived and runs on a background
/// thread.  Only one callback handler can be installed per process.
///
/// # Errors
///
/// Returns an error when a handler has already been installed, when the set is
/// empty, when the set contains `SIGKILL` or `SIGSTOP`, or when the operating
/// system rejects the signal setup.
pub fn set_handler_for<S, F>(signals: S, handler: F) -> std::result::Result<(), Error>
where
    S: Into<SignalSet>,
    F: FnMut(Signal) + Send + 'static,
{
    let signals: Vec<_> = signals.into().iter().collect();
    if signals.is_empty() {
        return Err(Error::EmptySignalSet);
    }
    for signal in &signals {
        if !signal.is_catchable() {
            return Err(Error::UncatchableSignal(*signal));
        }
    }

    let mut pipe = SignalPipe::new()?;
    if HANDLER_INSTALLED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err(Error::AlreadyInstalled);
    }

    SIGNAL_WRITE_FD.store(pipe.write_fd(), Ordering::Release);
    let read_fd = pipe.take_read();
    if let Err(err) = spawn_signal_thread(read_fd, handler) {
        SIGNAL_WRITE_FD.store(-1, Ordering::Release);
        HANDLER_INSTALLED.store(false, Ordering::Release);
        return Err(Error::System(err));
    }

    if let Err(err) = install_pipe_handlers(&signals) {
        SIGNAL_WRITE_FD.store(-1, Ordering::Release);
        HANDLER_INSTALLED.store(false, Ordering::Release);
        return Err(Error::System(err));
    }

    pipe.keep_write_open();
    Ok(())
}

fn spawn_signal_thread<F>(read_fd: libc::c_int, mut handler: F) -> io::Result<()>
where
    F: FnMut(Signal) + Send + 'static,
{
    std::thread::Builder::new()
        .name("minimal-signals".to_string())
        .spawn(move || {
            // SAFETY(rescrv): read_fd is freshly created by pipe(2), moved to this thread exactly
            // once, and owned by this File until the thread exits.
            let mut reader = unsafe { std::fs::File::from_raw_fd(read_fd) };
            loop {
                let mut bytes = [0u8; std::mem::size_of::<i32>()];
                match reader.read_exact(&mut bytes) {
                    Ok(()) => {
                        let signal = i32::from_ne_bytes(bytes);
                        if let Some(signal) = Signal::from_i32(signal) {
                            handler(signal);
                        }
                    }
                    Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
        })
        .map(|_| ())
}

fn install_pipe_handlers(signals: &[Signal]) -> io::Result<()> {
    let mut installed = Vec::<(Signal, libc::sigaction)>::with_capacity(signals.len());
    for signal in signals {
        let mut sa: libc::sigaction = unsafe { std::mem::zeroed() };
        sa.sa_sigaction = pipe_signal_handler as *const () as usize;
        sa.sa_flags = libc::SA_RESTART;
        // SAFETY(rescrv): This initializes the mask field before sigaction observes it.
        unsafe {
            libc::sigemptyset(&mut sa.sa_mask);
        }
        let mut previous: libc::sigaction = unsafe { std::mem::zeroed() };
        // SAFETY(rescrv): The handler has C ABI, the sigaction values are initialized, and libc
        // owns no Rust references.
        if unsafe { libc::sigaction(signal.0, &sa, &mut previous) } != 0 {
            let err = io::Error::last_os_error();
            restore_signal_handlers(&installed);
            return Err(err);
        }
        installed.push((*signal, previous));
    }
    Ok(())
}

fn restore_signal_handlers(installed: &[(Signal, libc::sigaction)]) {
    for (signal, previous) in installed.iter().rev() {
        // SAFETY(rescrv): previous came from sigaction for this exact signal.
        unsafe {
            libc::sigaction(signal.0, previous, std::ptr::null_mut());
        }
    }
}

extern "C" fn pipe_signal_handler(signal: i32) {
    let fd = SIGNAL_WRITE_FD.load(Ordering::Relaxed);
    if fd < 0 {
        return;
    }
    let bytes = signal.to_ne_bytes();
    // SAFETY(rescrv): write(2) is async-signal-safe. The write end is nonblocking, so a full pipe
    // drops the notification instead of blocking inside the signal handler.
    unsafe {
        let _ = libc::write(fd, bytes.as_ptr().cast(), bytes.len());
    }
}

struct SignalPipe {
    read: Option<libc::c_int>,
    write: Option<libc::c_int>,
}

impl SignalPipe {
    fn new() -> io::Result<Self> {
        let mut fds = [0; 2];
        // SAFETY(rescrv): fds points to two valid file descriptor slots.
        if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let pipe = Self {
            read: Some(fds[0]),
            write: Some(fds[1]),
        };
        if let Err(err) = set_close_on_exec(fds[0])
            .and_then(|_| set_close_on_exec(fds[1]))
            .and_then(|_| set_nonblocking(fds[1]))
        {
            drop(pipe);
            return Err(err);
        }
        Ok(pipe)
    }

    fn write_fd(&self) -> libc::c_int {
        self.write.expect("write fd should exist")
    }

    fn take_read(&mut self) -> libc::c_int {
        self.read.take().expect("read fd should exist")
    }

    fn keep_write_open(&mut self) {
        let _ = self.write.take();
    }
}

impl Drop for SignalPipe {
    fn drop(&mut self) {
        if let Some(fd) = self.read.take() {
            close_fd(fd);
        }
        if let Some(fd) = self.write.take() {
            close_fd(fd);
        }
    }
}

fn set_close_on_exec(fd: libc::c_int) -> io::Result<()> {
    // SAFETY(rescrv): fcntl observes only the provided fd.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY(rescrv): fcntl observes only the provided fd and integer flags.
    if unsafe { libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn set_nonblocking(fd: libc::c_int) -> io::Result<()> {
    // SAFETY(rescrv): fcntl observes only the provided fd.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY(rescrv): fcntl observes only the provided fd and integer flags.
    if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn close_fd(fd: libc::c_int) {
    // SAFETY(rescrv): close observes only the provided fd. Errors are not actionable on drop.
    unsafe {
        libc::close(fd);
    }
}

/////////////////////////////////////////////// block //////////////////////////////////////////////

/// Replace the calling thread's signal mask with a full signal set.
///
/// # Panics
///
/// Panics if the operating system rejects the mask change.
pub fn block() {
    try_block().expect("block should never fail");
}

/// Replace the calling thread's signal mask with a full signal set.
///
/// # Errors
///
/// Returns an error if the operating system rejects the mask change.
pub fn try_block() -> io::Result<()> {
    SignalSet::all().block()
}

////////////////////////////////////////////// unblock /////////////////////////////////////////////

/// Replace the calling thread's signal mask with an empty signal set.
///
/// # Panics
///
/// Panics if the operating system rejects the mask change.
pub fn unblock() {
    try_unblock().expect("unblock should never fail");
}

/// Replace the calling thread's signal mask with an empty signal set.
///
/// # Errors
///
/// Returns an error if the operating system rejects the mask change.
pub fn try_unblock() -> io::Result<()> {
    SignalSet::new().empty().block()
}

////////////////////////////////////////////// install /////////////////////////////////////////////

/// Install a no-op signal handler for all catchable known signals.
///
/// # Panics
///
/// Panics if the operating system rejects the handler setup.
pub fn install() {
    try_install().expect("install should never fail");
}

/// Install a no-op signal handler for all catchable known signals.
///
/// # Errors
///
/// Returns an error if the operating system rejects the handler setup.
pub fn try_install() -> io::Result<()> {
    SignalSet::new()
        .add(SIGHUP)
        .add(SIGINT)
        .add(SIGQUIT)
        .add(SIGALRM)
        .add(SIGTERM)
        .add(SIGCHLD)
        .add(SIGUSR1)
        .add(SIGUSR2)
        .install()
}

/////////////////////////////////////////////// wait ///////////////////////////////////////////////

/// Wait synchronously until a signal in `set` is pending.
pub fn wait(set: SignalSet) -> Option<Signal> {
    let mut signal = -1i32;
    // SAFETY(rescrv): We know this is safe because we manipulate the set using libc functions.
    let rc = unsafe { libc::sigwait(&set.sigset, &mut signal) };
    if rc != 0 {
        return None;
    }
    Signal::from_i32(signal)
}

////////////////////////////////////////////// pending /////////////////////////////////////////////

/// Return the currently pending signals known to this process.
pub fn pending() -> SignalSet {
    let mut set = SignalSet::new();
    // SAFETY(rescrv): We know this is safe because we manipulate the set using libc functions.
    unsafe {
        libc::sigpending(&mut set.sigset);
    };
    set
}

//////////////////////////////////////////////// kill //////////////////////////////////////////////

/// Send `signal` to `pid`.
///
/// # Errors
///
/// Returns an error if `kill(2)` fails.
pub fn kill(pid: libc::pid_t, signal: Signal) -> io::Result<()> {
    // SAFETY(rescrv): kill observes only the pid and signal number.
    if unsafe { libc::kill(pid, signal.0) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

//////////////////////////////////////////////// raise /////////////////////////////////////////////

/// Send `signal` to the current process.
///
/// # Errors
///
/// Returns an error if `raise(3)` fails.
pub fn raise(signal: Signal) -> io::Result<()> {
    // SAFETY(rescrv): raise observes only the signal number.
    if unsafe { libc::raise(signal.0) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

//////////////////////////////////////////////// nop ///////////////////////////////////////////////

extern "C" fn nop(_: i32, _: *mut libc::siginfo_t, _: *mut libc::c_void) {}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_names_round_trip() {
        for signal in Signal::known() {
            assert_eq!(Some(signal), Signal::from_i32(signal.as_i32()));
            assert_eq!(signal.name(), signal.to_string());
        }
    }

    #[test]
    fn signal_set_display() {
        assert_eq!("()", format!("{}", SignalSet::new().empty()));
        assert_eq!(
            "SIGINT",
            format!("{}", SignalSet::new().empty().add(SIGINT))
        );
        assert_eq!(
            "SIGUSR1|SIGUSR2",
            format!("{}", SignalSet::new().empty().add(SIGUSR1).add(SIGUSR2))
        );
    }

    #[test]
    fn signal_set_contains_and_iterates_known_signals() {
        let set = SignalSet::from_signals([SIGTERM, SIGUSR1, SIGUSR2]);
        assert!(set.contains(SIGTERM));
        assert!(!set.contains(SIGINT));
        assert_eq!(
            vec![SIGTERM, SIGUSR1, SIGUSR2],
            set.iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn signal_set_conveniences() {
        assert_eq!(vec![SIGINT], SignalSet::ctrl_c().iter().collect::<Vec<_>>());
        assert_eq!(
            vec![SIGHUP, SIGINT, SIGTERM],
            SignalSet::termination().iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn handler_rejects_bad_signal_sets_then_receives_signal() {
        assert!(matches!(
            set_handler_for(SignalSet::new(), |_| {}),
            Err(Error::EmptySignalSet)
        ));
        assert!(matches!(
            set_handler_for(SIGKILL, |_| {}),
            Err(Error::UncatchableSignal(SIGKILL))
        ));

        let (sender, receiver) = std::sync::mpsc::channel();
        set_handler_for(SIGUSR1, move |signal| {
            let _ = sender.send(signal);
        })
        .expect("handler should install");
        raise(SIGUSR1).expect("raise should succeed");
        assert_eq!(
            SIGUSR1,
            receiver
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("signal should arrive")
        );
    }
}
