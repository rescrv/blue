#![doc = include_str!("../README.md")]

use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::hash::{Hash, Hasher};
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Condvar, Mutex, WaitTimeoutResult};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use indicio::{DEBUG, ERROR, INFO, clue, value};
use one_two_eight::generate_id;
use rc_conf::{RcConf, SwitchPosition, load_services};
use utf8path::Path;

mod helper;
pub mod logging;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static IO_ERROR: biometrics::Counter = biometrics::Counter::new("rustrc.error.io");
static SHVAR_ERROR: biometrics::Counter = biometrics::Counter::new("rustrc.error.shvar");
static RC_CONF_ERROR: biometrics::Counter = biometrics::Counter::new("rustrc.error.rc_conf");
static UNKNOWN_SERVICE: biometrics::Counter =
    biometrics::Counter::new("rustrc.error.unknown_service");
static NUL_ERROR: biometrics::Counter = biometrics::Counter::new("rustrc.error.null");
static STATE_NEW: biometrics::Counter = biometrics::Counter::new("rustrc.state.new");
static INHIBITED_SERVICE: biometrics::Counter = biometrics::Counter::new("rustrc.inhibited");
static WAITPID_ENTER: biometrics::Counter = biometrics::Counter::new("rustrc.waitpid.enter");
static WAITPID_EXIT: biometrics::Counter = biometrics::Counter::new("rustrc.waitpid.exit");
static ORPHAN_REAP: biometrics::Counter = biometrics::Counter::new("rustrc.orphan_reap");
static NON_POSITIVE_PID: biometrics::Counter = biometrics::Counter::new("rustrc.non_positive_pid");
static RECLAIM: biometrics::Counter = biometrics::Counter::new("rustrc.reclaim");
static JOINING_THREAD: biometrics::Counter = biometrics::Counter::new("rustrc.join");
static CONVERGE: biometrics::Counter = biometrics::Counter::new("rustrc.converge");
static RESPAWNING: biometrics::Counter = biometrics::Counter::new("rustrc.respawn");
static RECONFIGURE: biometrics::Counter = biometrics::Counter::new("rustrc.api.reconfigure");
static RELOAD: biometrics::Counter = biometrics::Counter::new("rustrc.api.reload");
static KILL: biometrics::Counter = biometrics::Counter::new("rustrc.api.kill");
static LIST_SERVICES: biometrics::Counter = biometrics::Counter::new("rustrc.api.list_services");
static ENABLED_SERVICES: biometrics::Counter =
    biometrics::Counter::new("rustrc.api.enabled_services");
static START: biometrics::Counter = biometrics::Counter::new("rustrc.api.start");
static RESTART: biometrics::Counter = biometrics::Counter::new("rustrc.api.restart");
static STOP: biometrics::Counter = biometrics::Counter::new("rustrc.api.stop");
static EXECUTION_KILL: biometrics::Counter = biometrics::Counter::new("rustrc.execution.kill");
static EXECUTION_EXEC: biometrics::Counter = biometrics::Counter::new("rustrc.execution.exec");

/// Register biometrics with the given collector.
pub fn register_biometrics(collector: &biometrics::Collector) {
    collector.register_counter(&IO_ERROR);
    collector.register_counter(&SHVAR_ERROR);
    collector.register_counter(&RC_CONF_ERROR);
    collector.register_counter(&NUL_ERROR);
    collector.register_counter(&STATE_NEW);
    collector.register_counter(&INHIBITED_SERVICE);
    collector.register_counter(&WAITPID_ENTER);
    collector.register_counter(&WAITPID_EXIT);
    collector.register_counter(&ORPHAN_REAP);
    collector.register_counter(&NON_POSITIVE_PID);
    collector.register_counter(&RECLAIM);
    collector.register_counter(&JOINING_THREAD);
    collector.register_counter(&CONVERGE);
    collector.register_counter(&RESPAWNING);
    collector.register_counter(&RECONFIGURE);
    collector.register_counter(&RELOAD);
    collector.register_counter(&KILL);
    collector.register_counter(&LIST_SERVICES);
    collector.register_counter(&ENABLED_SERVICES);
    collector.register_counter(&START);
    collector.register_counter(&RESTART);
    collector.register_counter(&STOP);
    collector.register_counter(&UNKNOWN_SERVICE);
    collector.register_counter(&EXECUTION_KILL);
    collector.register_counter(&EXECUTION_EXEC);
    helper::register_biometrics(collector);
    logging::register_biometrics(collector);
}

/// How long a stop waits after SIGTERM before SIGKILL when a service sets no STOP_TIMEOUT.
pub const DEFAULT_STOP_TIMEOUT: Duration = Duration::from_secs(10);

//////////////////////////////////////////// init support //////////////////////////////////////////

#[cfg(target_os = "linux")]
fn enable_child_subreaper() -> Result<(), Error> {
    // SAFETY(rescrv): prctl observes only integer arguments here.  PR_SET_CHILD_SUBREAPER asks
    // Linux to reparent orphaned descendants to this process before init(1).
    if unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    clue!(COLLECTOR, INFO, {
        child_subreaper: true,
    });
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn enable_child_subreaper() -> Result<(), Error> {
    clue!(COLLECTOR, INFO, {
        child_subreaper: false,
        unsupported: true,
    });
    Ok(())
}

fn peek_waitable_child() -> Result<Option<libc::pid_t>, Error> {
    loop {
        let mut info = MaybeUninit::<libc::siginfo_t>::zeroed();
        // SAFETY(rescrv): waitid initializes info when a waitable child exists.  WNOWAIT leaves
        // the child for its owner to reap after we classify it.
        let rc = unsafe {
            libc::waitid(
                libc::P_ALL,
                0,
                info.as_mut_ptr(),
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if rc != 0 {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            if err.raw_os_error() == Some(libc::ECHILD) {
                return Ok(None);
            }
            return Err(err.into());
        }
        // SAFETY(rescrv): waitid returned success.  The value was zero-initialized, and si_signo
        // remains zero when WNOHANG found no waitable child.
        let info = unsafe { info.assume_init() };
        if info.si_signo == 0 {
            return Ok(None);
        }
        // SAFETY(rescrv): POSIX requires si_pid to identify the child for SIGCHLD waitid results.
        let pid = unsafe { info.si_pid() };
        if pid > 0 {
            return Ok(Some(pid));
        }
        return Ok(None);
    }
}

////////////////////////////////////////////// indicio /////////////////////////////////////////////

/// An indicio clue-collector hook point.
pub static COLLECTOR: indicio::Collector = indicio::Collector::new();

//////////////////////////////////////////// ExecutionID ///////////////////////////////////////////

generate_id! {ExecutionID, "execution:"}

/////////////////////////////////////////////// Error //////////////////////////////////////////////

/// The Error type.
#[derive(Debug)]
pub enum Error {
    /// There was an error generating enough randomness for an ExecutionID.
    GeneratingExecutionID,
    /// The named service is not known to rustrc.
    UnknownService,
    /// The service is disabled.
    ServiceDisabled,
    /// The service is already started.
    ServiceAlreadyStarted,
    /// There's a persistent error with the service.
    ServiceError(String),
    /// Shutdown has begun; rustrc starts nothing new.
    ShuttingDown,
    /// An error returned by IO.
    Io(std::io::Error),
    /// An error returned by shvar.
    Shvar(shvar::Error),
    /// An error returned by rc_conf.
    RcConf(rc_conf::Error),
    /// A NulError relating to CString.
    NulError,
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        IO_ERROR.click();
        Self::Io(err)
    }
}

impl From<shvar::Error> for Error {
    fn from(err: shvar::Error) -> Self {
        SHVAR_ERROR.click();
        Self::Shvar(err)
    }
}

impl From<rc_conf::Error> for Error {
    fn from(err: rc_conf::Error) -> Self {
        RC_CONF_ERROR.click();
        Self::RcConf(err)
    }
}

impl From<std::ffi::NulError> for Error {
    fn from(_: std::ffi::NulError) -> Self {
        NUL_ERROR.click();
        Self::NulError
    }
}

impl From<&Error> for indicio::Value {
    fn from(err: &Error) -> Self {
        fn shvar_to_value(err: &shvar::Error) -> indicio::Value {
            match err {
                shvar::Error::OpenSingleQuotes => {
                    indicio::value!({open_single_quotes: true})
                }
                shvar::Error::OpenDoubleQuotes => {
                    indicio::value!({open_double_quotes: true})
                }
                shvar::Error::TrailingRightBrace => {
                    indicio::value!({trailing_right_brace: true})
                }
                shvar::Error::InvalidVariable => {
                    indicio::value!({invalid_variable: true})
                }
                shvar::Error::InvalidCharacter {
                    expected,
                    returned: Some(returned),
                } => {
                    indicio::value!({invalid_charcater: { expected: *expected, returned: *returned }})
                }
                shvar::Error::InvalidCharacter {
                    expected,
                    returned: None,
                } => {
                    indicio::value!({invalid_charcater: { expected: *expected }})
                }
                shvar::Error::DepthLimitExceeded => {
                    indicio::value!({depth_limit_exceeded: true})
                }
                shvar::Error::Requested(message) => {
                    indicio::value!({requested: message})
                }
            }
        }
        match err {
            Error::GeneratingExecutionID => {
                indicio::value!({
                    generating_execution_id: true,
                })
            }
            Error::UnknownService => {
                indicio::value!({
                    unknown_service: true,
                })
            }
            Error::ServiceDisabled => {
                indicio::value!({
                    service_disabled: true,
                })
            }
            Error::ServiceAlreadyStarted => {
                indicio::value!({
                    service_already_started: true,
                })
            }
            Error::ShuttingDown => {
                indicio::value!({
                    shutting_down: true,
                })
            }
            Error::ServiceError(msg) => {
                indicio::value!({
                    service_error: msg,
                })
            }
            Error::Io(err) => {
                indicio::value!({
                    io: format!("{:?}", err),
                })
            }
            Error::Shvar(err) => {
                indicio::value!({
                    shvar: shvar_to_value(err),
                })
            }
            Error::RcConf(err) => {
                let inner = match err {
                    rc_conf::Error::FileTooLarge { path } => {
                        indicio::value!({
                            path: path.as_str(),
                            file_too_large: true,
                        })
                    }
                    rc_conf::Error::TrailingWhack { path } => {
                        indicio::value!({
                            path: path.as_str(),
                            trailing_whack: true,
                        })
                    }
                    rc_conf::Error::ProhibitedCharacter {
                        path,
                        line,
                        string,
                        character,
                    } => {
                        indicio::value!({
                            path: path.as_str(),
                            line: *line,
                            prohibited_character: {
                                string: string,
                                character: *character,
                            },
                        })
                    }
                    rc_conf::Error::InvalidRcConf {
                        path,
                        line,
                        message,
                    } => {
                        indicio::value!({
                            path: path.as_str(),
                            line: *line,
                            invalid_rc_conf: message,
                        })
                    }
                    rc_conf::Error::InvalidRcScript {
                        path,
                        line,
                        message,
                    } => {
                        indicio::value!({
                            path: path.as_str(),
                            line: *line,
                            invalid_rc_Script: message,
                        })
                    }
                    rc_conf::Error::InvalidInvocation { message } => {
                        indicio::value!({
                            invalid_invocation: message,
                        })
                    }
                    rc_conf::Error::IoError(err) => {
                        indicio::value!({
                            io: format!("{:?}", err),
                        })
                    }
                    rc_conf::Error::ShvarError(err) => {
                        indicio::value!({
                            shvar: shvar_to_value(err),
                        })
                    }
                    rc_conf::Error::Utf8Error(err) => {
                        indicio::value!({
                            utf8: format!("{:?}", err),
                        })
                    }
                    rc_conf::Error::FromUtf8Error(err) => {
                        indicio::value!({
                            from_utf8: format!("{:?}", err),
                        })
                    }
                    rc_conf::Error::ExecFailed { command, error } => {
                        indicio::value!({
                            command: command,
                            exec_failed: format!("{:?}", error),
                        })
                    }
                };
                indicio::value!({
                    rc_conf: inner,
                })
            }
            Error::NulError => {
                indicio::value!({
                    nul_error: true,
                })
            }
        }
    }
}

////////////////////////////////////////////// Target //////////////////////////////////////////////

/// The target to kill.
#[derive(Clone, Debug, Default)]
pub enum Target {
    #[default]
    All,
    One(String),
    Pid(i32),
}

impl Target {
    fn matches(&mut self, e: &Execution) -> bool {
        match self {
            Target::All => true,
            Target::One(s) => *s == e.service,
            Target::Pid(p) => Some(*p) == e.pid(),
        }
    }

    /// True if the execution matches the provided name.
    pub fn matches_name(&mut self, name: impl AsRef<str>) -> bool {
        match self {
            Target::All => true,
            Target::One(s) => *s == name.as_ref(),
            Target::Pid(_) => false,
        }
    }
}

impl<S: AsRef<str>> From<S> for Target {
    fn from(s: S) -> Self {
        let s = s.as_ref();
        if s == "*" {
            Target::All
        } else {
            Target::One(s.to_string())
        }
    }
}

impl From<&Target> for indicio::Value {
    fn from(target: &Target) -> Self {
        match target {
            Target::All => {
                value!({
                    all: true,
                })
            }
            Target::One(s) => {
                value!({
                    one: s,
                })
            }
            Target::Pid(p) => {
                value!({
                    pid: *p,
                })
            }
        }
    }
}

//////////////////////////////////////////// Pid1Options ///////////////////////////////////////////

/// Pid1Options captures configuration paths and init-style process behavior.
#[derive(Clone, Debug, Eq, PartialEq, arrrg_derive::CommandLine)]
pub struct Pid1Options {
    #[arrrg(
        optional,
        "A colon-separated PATH-like list of rc.conf files to be loaded in order.  Later files override."
    )]
    pub rc_conf_path: String,
    #[arrrg(
        optional,
        "A colon-separated PATH-like list of rc.d directories to be scanned in order.  Earlier files short-circuit."
    )]
    pub rc_d_path: String,
    #[arrrg(
        flag,
        "Reap exited child processes that are not managed rustrc services."
    )]
    pub reap_orphans: bool,
    #[arrrg(
        flag,
        "On Linux, ask the kernel to reparent orphaned descendants to this process."
    )]
    pub child_subreaper: bool,
    #[arrrg(
        optional,
        "Milliseconds a stub gets to answer `rcvar` before its process group is killed."
    )]
    pub stub_timeout_ms: u64,
}

impl Default for Pid1Options {
    fn default() -> Self {
        Self {
            rc_conf_path: "rc.conf".to_string(),
            rc_d_path: "rc.d".to_string(),
            reap_orphans: false,
            child_subreaper: false,
            stub_timeout_ms: 10_000,
        }
    }
}

impl From<&Pid1Options> for indicio::Value {
    fn from(options: &Pid1Options) -> Self {
        value!({
            rc_conf_path: options.rc_conf_path.as_str(),
            rc_d_path: options.rc_d_path.as_str(),
            reap_orphans: options.reap_orphans,
            child_subreaper: options.child_subreaper,
            stub_timeout_ms: options.stub_timeout_ms,
        })
    }
}

///////////////////////////////////////// Pid1Configuration ////////////////////////////////////////

/// Pre-parse the rc_conf and rc.d paths to a data structure that can be accessed without I/O.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pid1Configuration {
    services: HashMap<String, Result<Path<'static>, String>>,
    rc_conf: RcConf,
    stub_timeout: Duration,
}

impl Pid1Configuration {
    /// Create a new Pid1Configuration using the options.
    pub fn from_options(options: &Pid1Options) -> Result<Self, rc_conf::Error> {
        let services = load_services(&options.rc_d_path)?;
        let rc_conf = RcConf::parse(&options.rc_conf_path)?;
        let stub_timeout = Duration::from_millis(options.stub_timeout_ms);
        Ok(Self {
            services,
            rc_conf,
            stub_timeout,
        })
    }

    /// The services and aliases available from the combination of the rc_conf and rc.d.
    pub fn services(&self) -> Vec<String> {
        let mut services = vec![];
        services.extend(self.services.keys().cloned());
        services.extend(self.rc_conf.aliases());
        services
    }

    fn get_service_path<'a>(&'a self, service: &str) -> Option<Result<Path<'a>, String>> {
        let service = self.rc_conf.resolve_alias(service);
        self.services.get(service).cloned()
    }
}

///////////////////////////////////////////// Pid1State ////////////////////////////////////////////

#[derive(Debug)]
struct Pid1State {
    // Set when shutdown begins.  Nothing spawns once it is set.
    shutdown: bool,
    // Set when shutdown has drained the process table.  The orphan reaper runs until then.
    finished: bool,
    converge: u64,
    config: Arc<Pid1Configuration>,
    processes: Vec<Arc<Execution>>,
    inhibited: HashSet<String>,
    backedoff: HashMap<String, Instant>,
    history: HashMap<String, ServiceHistory>,
}

/// Per-service counters kept across executions.
#[derive(Clone, Debug, Default)]
struct ServiceHistory {
    starts: u64,
    last_exit: Option<(libc::c_int, Instant)>,
}

impl Pid1State {
    fn new(config: Arc<Pid1Configuration>) -> Self {
        let shutdown = false;
        let finished = false;
        let converge = 1;
        let processes = vec![];
        let inhibited = HashSet::new();
        let backedoff = HashMap::new();
        let history = HashMap::new();
        STATE_NEW.click();
        Self {
            shutdown,
            finished,
            converge,
            config,
            processes,
            inhibited,
            backedoff,
            history,
        }
    }

    fn is_running(&self, service: &str) -> bool {
        self.processes.iter().any(|p| p.service == service)
    }

    fn service_switch(&self, service: &str) -> SwitchPosition {
        if self.is_inhibited(service) {
            INHIBITED_SERVICE.click();
            clue!(COLLECTOR, DEBUG, {
                inhibited: service,
            });
            return SwitchPosition::No;
        }
        self.config.rc_conf.service_switch(service)
    }

    fn set_inhibit(&mut self, service: String) {
        clue!(COLLECTOR, INFO, {
            set_inhibit: &service,
        });
        self.inhibited.insert(service);
    }

    fn clear_inhibit(&mut self, service: &str) {
        clue!(COLLECTOR, DEBUG, {
            clear_inhibit: service,
        });
        self.inhibited.remove(service);
    }

    fn is_inhibited(&self, service: &str) -> bool {
        self.inhibited.contains(service)
    }

    fn set_backoff(&mut self, service: String, when: Instant) {
        self.backedoff.insert(service, when);
    }

    fn get_backoff(&self, service: &str) -> Option<Instant> {
        self.backedoff.get(service).cloned()
    }

    fn clear_backoff(&mut self, service: &str) {
        self.backedoff.remove(service);
    }

    fn cleanup_backoff(&mut self, now: Instant) {
        self.backedoff.retain(|_, v| *v > now);
    }

    /// Record a failed start so the next attempt waits out a backoff.
    fn spawn_failed(&mut self, coord: &Pid1Coordination, service: &str) {
        let mut backoff = coord.backoff.lock().unwrap();
        backoff.track(service.to_string(), Duration::ZERO);
        self.set_backoff(
            service.to_string(),
            Instant::now() + backoff.backoff(service),
        );
    }

    /// Spawn `service` from a context computed without the state lock held (see
    /// [`Pid1::context_for`]).  A failed context or spawn counts against the service's backoff.
    fn spawn(
        &mut self,
        coord: &Pid1Coordination,
        reclaim: SyncSender<Arc<Execution>>,
        service: &str,
        context: Result<ExecutionContext, Error>,
    ) -> Result<ExecutionID, Error> {
        // Checked under the state lock that shutdown sets the flag under, so no spawn can slip in
        // after shutdown snapshots the process table.
        if self.shutdown {
            return Err(Error::ShuttingDown);
        }
        let result = context.and_then(|context| self.spawn_inner(reclaim, service, context));
        if result.is_err() {
            self.spawn_failed(coord, service);
        }
        result
    }

    fn spawn_inner(
        &mut self,
        reclaim: SyncSender<Arc<Execution>>,
        service: &str,
        context: ExecutionContext,
    ) -> Result<ExecutionID, Error> {
        self.clear_backoff(service);
        let execution_id = ExecutionID::generate().ok_or(Error::GeneratingExecutionID)?;
        let config = Arc::clone(&self.config);
        let service = service.to_string();
        let execution = Arc::new(Execution::new(execution_id, config, service, context));
        let exec = Arc::clone(&execution);
        let thread = std::thread::Builder::new()
            .stack_size(65536)
            .spawn(move || Self::wait(exec, reclaim))?;
        execution.set_thread(thread);
        // posix_spawn and the push happen under the state lock so the orphan reaper, which
        // classifies pids under the same lock, never mistakes a fresh service for an orphan.
        execution.exec()?;
        self.history
            .entry(execution.service.clone())
            .or_default()
            .starts += 1;
        self.processes.push(execution);
        Ok(execution_id)
    }

    fn wait(exec: Arc<Execution>, reclaim: SyncSender<Arc<Execution>>) {
        if let Some(pid) = exec.block_until_spawned() {
            WAITPID_ENTER.click();
            exec.await_exit(pid);
            exec.reap(pid);
            WAITPID_EXIT.click();
        } else {
            NON_POSITIVE_PID.click();
        }
        // The reclaimer outlives every execution; shutdown drops the last sender only after the
        // process table drains.
        let _ = reclaim.send(exec);
    }
}

///////////////////////////////////////////// Pid1State ////////////////////////////////////////////

#[derive(Debug, Default)]
struct Pid1Coordination {
    converge: Condvar,
    backoff: Mutex<BackoffTracker>,
}

/////////////////////////////////////////////// Pid1 ///////////////////////////////////////////////

/// Pid1 provides process supervision over processes specified by rc_conf and rc.d.
#[derive(Debug)]
pub struct Pid1 {
    options: Mutex<Pid1Options>,
    state: Arc<Mutex<Pid1State>>,
    coord: Arc<Pid1Coordination>,
    // Reclaim threads that waitpid on processes.
    reclaim: SyncSender<Arc<Execution>>,
    reclaimer: JoinHandle<()>,
    // Reap orphaned descendants when rustrc is acting as an init process.
    orphan_reaper: Option<JoinHandle<()>>,
    // Converge to the configuration regularly, respawning when necessary.
    converger: JoinHandle<()>,
}

impl Pid1 {
    /// Create a new Pid1 from the provided options.
    pub fn new(options: Pid1Options) -> Result<Self, Error> {
        if options.child_subreaper {
            enable_child_subreaper()?;
        }
        let config = Arc::new(Pid1Configuration::from_options(&options)?);
        let state = Arc::new(Mutex::new(Pid1State::new(config)));
        let coord = Arc::new(Pid1Coordination::default());
        let (reclaim, recv) = sync_channel(1);
        let reclaim_state = Arc::clone(&state);
        let reclaim_coord = Arc::clone(&coord);
        let reclaimer = std::thread::Builder::new()
            .stack_size(65536)
            .spawn(move || Self::reclaim_thread(recv, reclaim_state, reclaim_coord))
            .unwrap();
        let orphan_reaper = if options.reap_orphans {
            let orphan_state = Arc::clone(&state);
            let orphan_coord = Arc::clone(&coord);
            Some(
                std::thread::Builder::new()
                    .stack_size(65536)
                    .spawn(move || Self::orphan_reaper_thread(orphan_state, orphan_coord))
                    .unwrap(),
            )
        } else {
            None
        };
        let converge_reclaim = reclaim.clone();
        let converge_state = Arc::clone(&state);
        let converge_coord = Arc::clone(&coord);
        let converger = std::thread::Builder::new()
            .spawn(move || Self::converge_thread(converge_reclaim, converge_state, converge_coord))
            .unwrap();
        let options = Mutex::new(options);

        Ok(Self {
            options,
            state,
            coord,
            reclaim,
            reclaimer,
            orphan_reaper,
            converger,
        })
    }

    fn reclaim_thread(
        reclaim: Receiver<Arc<Execution>>,
        state: Arc<Mutex<Pid1State>>,
        coord: Arc<Pid1Coordination>,
    ) {
        loop {
            let exec = match reclaim.recv() {
                Ok(exec) => exec,
                Err(_) => {
                    break;
                }
            };
            RECLAIM.click();
            if let Some(join) = exec.take_thread() {
                JOINING_THREAD.click();
                let _ = join.join();
            }
            // An exit rustrc asked for (stop, restart, reconfigure, shutdown) is not a crash and
            // earns no penalty; anything else backs off.
            let backoff = (!exec.stop_requested()).then(|| {
                let mut backoff = coord.backoff.lock().unwrap();
                backoff.track(exec.service.to_string(), exec.context.started.elapsed());
                backoff.wipe_debts();
                backoff.backoff(&exec.service)
            });
            let service = exec.service.to_string();
            {
                let mut state = state.lock().unwrap();
                state.processes.retain(|p| !Arc::ptr_eq(p, &exec));
                if let Some(status) = *exec.exit_status.lock().unwrap() {
                    state.history.entry(service.clone()).or_default().last_exit =
                        Some((status, Instant::now()));
                }
                match backoff {
                    Some(backoff) => state.set_backoff(service, Instant::now() + backoff),
                    None => state.clear_backoff(&service),
                }
                coord.converge.notify_all();
                state.converge = state.converge.wrapping_add(1);
            }
            exec.mark_done();
        }
    }

    fn orphan_reaper_thread(state: Arc<Mutex<Pid1State>>, coord: Arc<Pid1Coordination>) {
        loop {
            if let Err(err) = Self::reap_orphans_once(&state) {
                clue!(COLLECTOR, ERROR, {
                    orphan_reap: false,
                    error: indicio::Value::from(&err),
                });
            }
            let state_guard = state.lock().unwrap();
            if state_guard.finished {
                break;
            }
            // Keep reaping through shutdown:  stopping services is exactly when orphans appear.
            let (state_guard, _) = coord
                .converge
                .wait_timeout(state_guard, Duration::from_millis(250))
                .unwrap();
            if state_guard.finished {
                break;
            }
        }
        // One last pass for whatever the final stops left behind.
        let _ = Self::reap_orphans_once(&state);
    }

    fn reap_orphans_once(state: &Mutex<Pid1State>) -> Result<(), Error> {
        loop {
            let Some(pid) = peek_waitable_child()? else {
                return Ok(());
            };
            // Services and stub helpers have owners that reap them; the zombie stays first in
            // line until they do, so try again next pass.
            if helper::is_helper(pid) || Self::is_managed_pid(state, pid) {
                return Ok(());
            }
            let mut status = 0;
            let reaped = loop {
                WAITPID_ENTER.click();
                // SAFETY(rescrv): waitpid observes only the supplied pid and status pointer.
                let rc = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
                WAITPID_EXIT.click();
                if rc == pid {
                    break true;
                }
                if rc == 0 {
                    break false;
                }
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() == Some(libc::EINTR) {
                    continue;
                }
                if err.raw_os_error() == Some(libc::ECHILD) {
                    break false;
                }
                return Err(err.into());
            };
            if reaped {
                ORPHAN_REAP.click();
                clue!(COLLECTOR, DEBUG, {
                    orphan_reap: {
                        pid: pid,
                        status: status,
                    },
                });
            }
        }
    }

    fn is_managed_pid(state: &Mutex<Pid1State>, pid: libc::pid_t) -> bool {
        state
            .lock()
            .unwrap()
            .processes
            .iter()
            .any(|exec| exec.pid() == Some(pid))
    }

    fn converge_thread(
        reclaim: SyncSender<Arc<Execution>>,
        state: Arc<Mutex<Pid1State>>,
        coord: Arc<Pid1Coordination>,
    ) {
        let mut converge = 0;
        let mut wait = Duration::from_secs(10);
        loop {
            let c = {
                let mut state = state.lock().unwrap();
                clue!(COLLECTOR, DEBUG, { wait: format!("{:?}", wait), });
                while !state.shutdown && converge == state.converge {
                    let timed_out: WaitTimeoutResult;
                    (state, timed_out) = coord.converge.wait_timeout(state, wait).unwrap();
                    if timed_out.timed_out() {
                        break;
                    }
                }
                if state.shutdown {
                    break;
                }
                state.cleanup_backoff(Instant::now());
                state.converge
            };
            wait = Duration::from_secs(300);
            if Self::converge(&coord, &reclaim, &state, &mut wait) {
                converge = c;
            }
            wait = std::cmp::max(wait, Duration::from_secs(1));
        }
    }

    fn converge(
        coord: &Pid1Coordination,
        reclaim: &SyncSender<Arc<Execution>>,
        state: &Mutex<Pid1State>,
        wait: &mut Duration,
    ) -> bool {
        let mut converged = true;
        CONVERGE.click();
        let (processes, config) = {
            let state = state.lock().unwrap();
            if state.shutdown {
                return true;
            }
            (state.processes.clone(), Arc::clone(&state.config))
        };
        clue!(COLLECTOR, DEBUG, {
            converge: true,
            services: indicio::Value::from(config.services()),
        });
        for exec in processes {
            if exec.stop_requested() {
                continue;
            }
            let current_context = match ExecutionContext::new(&config, &exec.service, &[]) {
                Ok(current_context) => current_context,
                Err(err) => {
                    clue!(COLLECTOR, ERROR, {
                        service: exec.service.as_str(),
                        error: indicio::Value::from(&err),
                    });
                    continue;
                }
            };
            if current_context == exec.context || exec.pid().is_none() {
                continue;
            }
            if !exec.request_stop() {
                continue;
            }
            clue!(COLLECTOR, INFO, {
                reconfigure: {
                    service: exec.service.as_str(),
                    changed: indicio::Value::from(context_changes(&exec.context, &current_context)),
                },
            });
            // Stopping can take the whole STOP_TIMEOUT.  Do it on its own thread so this pass keeps
            // starting and respawning everything else; the reclaimer's notification brings the
            // replacement up with the new context.
            let stopping = Arc::clone(&exec);
            let spawned = std::thread::Builder::new()
                .name(format!("rustrc-restart-{}", exec.service))
                .stack_size(65536)
                .spawn(move || {
                    if let Err(err) = terminate(&stopping) {
                        clue!(COLLECTOR, ERROR, {
                            service: stopping.service.as_str(),
                            error: indicio::Value::from(&err),
                        });
                    }
                });
            if let Err(err) = spawned {
                clue!(COLLECTOR, ERROR, {
                    service: exec.service.as_str(),
                    error: format!("could not start restart thread: {err:?}"),
                });
                let _ = terminate(&exec);
            }
        }
        // Decide what to start under the lock, compute contexts (which runs each stub's rcvar)
        // without it, then revalidate under the lock before spawning.
        let now = Instant::now();
        let candidates = {
            let state = state.lock().unwrap();
            let mut candidates = vec![];
            for service in config.services() {
                if state.is_inhibited(&service) {
                    clue!(COLLECTOR, DEBUG, {
                        started: false,
                        service: service.as_str(),
                        inhibited: true,
                    });
                    continue;
                }
                if state.service_switch(&service) != SwitchPosition::Yes
                    || state.is_running(&service)
                {
                    continue;
                }
                RESPAWNING.click();
                if let Some(until) = state.get_backoff(&service).filter(|b| *b > now) {
                    clue!(COLLECTOR, DEBUG, {
                        started: false,
                        service: service.as_str(),
                        delayed: format!("{:?}", until - now),
                    });
                    *wait = std::cmp::min(*wait, until - now);
                    continue;
                }
                candidates.push(service);
            }
            candidates
        };
        for service in candidates {
            let context = ExecutionContext::new(&config, &service, &[]);
            let mut state = state.lock().unwrap();
            if !Arc::ptr_eq(&state.config, &config) {
                // A reload landed while we computed contexts; its converge pass takes over.
                return false;
            }
            let now = Instant::now();
            if state.is_inhibited(&service)
                || state.service_switch(&service) != SwitchPosition::Yes
                || state.is_running(&service)
                || state.get_backoff(&service).is_some_and(|b| b > now)
            {
                continue;
            }
            match state.spawn(coord, reclaim.clone(), &service, context) {
                Ok(_) => {
                    clue!(COLLECTOR, INFO, {
                        started: true,
                        service: service.as_str(),
                    });
                }
                Err(err) => {
                    converged = false;
                    let delay = state
                        .get_backoff(&service)
                        .map(|b| b.saturating_duration_since(now))
                        .unwrap_or_default();
                    clue!(COLLECTOR, ERROR, {
                        started: false,
                        service: service.as_str(),
                        delayed: format!("{delay:?}"),
                        error: indicio::Value::from(&err),
                    });
                    *wait = std::cmp::min(*wait, delay);
                }
            }
        }
        converged
    }

    /// Begin shutting down:  from now on nothing is started, respawned, or restarted.  Idempotent.
    ///
    /// Call this before signaling services on the way out; otherwise the converge loop can respawn
    /// a service that exits before [`Pid1::shutdown`] runs.
    pub fn begin_shutdown(&self) {
        let mut state = self.state.lock().unwrap();
        if !state.shutdown {
            clue!(COLLECTOR, INFO, {
                shutdown: true,
            });
        }
        state.shutdown = true;
        state.converge = state.converge.wrapping_add(1);
        self.coord.converge.notify_all();
    }

    /// Consume the pid1 and shut it down properly.  Every service's process group gets SIGTERM
    /// and, after its STOP_TIMEOUT, SIGKILL.  Returns only after all resources are reclaimed.
    pub fn shutdown(self) -> Result<(), Error> {
        self.begin_shutdown();
        // Spawns are fenced, so this snapshot is every process there will ever be.
        let processes = { self.state.lock().unwrap().processes.clone() };
        std::thread::scope(|scope| {
            for proc in processes.iter() {
                scope.spawn(|| {
                    if let Err(err) = terminate(proc) {
                        clue!(COLLECTOR, ERROR, {
                            service: proc.service.as_str(),
                            error: indicio::Value::from(&err),
                        });
                    }
                });
            }
        });
        let Pid1 {
            options: _,
            state,
            coord,
            reclaim,
            reclaimer,
            orphan_reaper,
            converger,
        } = self;
        {
            let mut state = state.lock().unwrap();
            debug_assert!(state.processes.is_empty());
            state.finished = true;
            coord.converge.notify_all();
        }
        converger.join().unwrap();
        drop(reclaim);
        reclaimer.join().unwrap();
        if let Some(orphan_reaper) = orphan_reaper {
            orphan_reaper.join().unwrap();
        }
        Ok(())
    }

    /// Reconfigure the Pid1 to use the new options.  This will call reload after loading the new
    /// options.
    pub fn reconfigure(&self, options: Pid1Options) -> Result<(), Error> {
        RECONFIGURE.click();
        clue!(COLLECTOR, INFO, {
            reconfigure: indicio::Value::from(&options),
        });
        let mut options_guard = self.options.lock().unwrap();
        let previous_options = options_guard.clone();
        *options_guard = options.clone();
        drop(options_guard);

        if let Err(error) = self.reload() {
            let mut options_guard = self.options.lock().unwrap();
            *options_guard = previous_options;
            Err(error)
        } else {
            Ok(())
        }
    }

    /// Reload the configuration from the rc_conf and rc.d paths provided as of the last
    /// configuration.
    pub fn reload(&self) -> Result<(), Error> {
        RELOAD.click();
        clue!(COLLECTOR, INFO, {
            reload: true,
        });
        let options = { self.options.lock().unwrap().clone() };
        let config = Arc::new(Pid1Configuration::from_options(&options)?);
        self.install(config);
        Ok(())
    }

    /// Reload, returning what converging onto the new configuration will do.
    pub fn reload_with_plan(&self) -> Result<ReloadPlan, Error> {
        RELOAD.click();
        clue!(COLLECTOR, INFO, {
            reload: true,
        });
        let options = { self.options.lock().unwrap().clone() };
        let config = Arc::new(Pid1Configuration::from_options(&options)?);
        let plan = self.plan_for(&config);
        self.install(config);
        Ok(plan)
    }

    /// Compute what a reload would do without applying it.
    pub fn plan_reload(&self) -> Result<ReloadPlan, Error> {
        let options = { self.options.lock().unwrap().clone() };
        let config = Pid1Configuration::from_options(&options)?;
        Ok(self.plan_for(&config))
    }

    fn install(&self, config: Arc<Pid1Configuration>) {
        {
            let mut state = self.state.lock().unwrap();
            state.config = config;
            state.converge = state.converge.wrapping_add(1);
        }
        self.coord.converge.notify_all();
    }

    fn plan_for(&self, config: &Pid1Configuration) -> ReloadPlan {
        let now = Instant::now();
        let (processes, inhibited, backedoff) = {
            let state = self.state.lock().unwrap();
            (
                state.processes.clone(),
                state.inhibited.clone(),
                state.backedoff.clone(),
            )
        };
        let mut plan = ReloadPlan::default();
        let mut running = HashSet::new();
        for exec in processes.iter() {
            if !running.insert(exec.service.clone()) {
                continue;
            }
            if config.get_service_path(&exec.service).is_none()
                || config.rc_conf.service_switch(&exec.service) == SwitchPosition::No
            {
                plan.keep_running.push(exec.service.clone());
                continue;
            }
            match ExecutionContext::new(config, &exec.service, &[]) {
                Ok(context) => {
                    if context != exec.context {
                        plan.restart.push((
                            exec.service.clone(),
                            context_changes(&exec.context, &context),
                        ));
                    }
                }
                Err(err) => plan.errors.push((exec.service.clone(), format!("{err:?}"))),
            }
        }
        for service in config.services() {
            if !running.contains(&service)
                && !inhibited.contains(&service)
                && config.rc_conf.service_switch(&service) == SwitchPosition::Yes
            {
                if let Err(err) = ExecutionContext::new(config, &service, &[]) {
                    plan.errors.push((service.clone(), format!("{err:?}")));
                }
                let backoff = backedoff
                    .get(&service)
                    .filter(|b| **b > now)
                    .map(|b| *b - now);
                plan.start.push((service, backoff));
            }
        }
        if let Ok(enabled) = config.rc_conf.list_services() {
            for service in enabled {
                if config.get_service_path(&service).is_none() {
                    plan.errors
                        .push((service, "enabled but there is no rc.d stub".to_string()));
                }
            }
        }
        plan.start.sort();
        plan.start.dedup();
        plan.restart.sort();
        plan.keep_running.sort();
        plan.errors.sort();
        plan
    }

    /// A point-in-time status of every known service, sorted by name.
    pub fn status(&self) -> Vec<ServiceStatus> {
        let state = self.state.lock().unwrap();
        let now = Instant::now();
        let mut names = state.config.services().into_iter().collect::<HashSet<_>>();
        names.extend(state.processes.iter().map(|p| p.service.clone()));
        names.extend(state.history.keys().cloned());
        let mut names = names.into_iter().collect::<Vec<_>>();
        names.sort();
        names
            .into_iter()
            .map(|service| {
                let known = state.config.get_service_path(&service).is_some();
                let running = state
                    .processes
                    .iter()
                    .filter(|p| p.service == service)
                    .filter_map(|p| p.pid().map(|pid| (pid, p.context.started.elapsed())))
                    .collect();
                let history = state.history.get(&service).cloned().unwrap_or_default();
                let log = state
                    .processes
                    .iter()
                    .find(|p| p.service == service)
                    .and_then(|p| p.context.log.as_ref())
                    .map(|l| l.to_string_lossy().into_owned())
                    .or_else(|| {
                        state
                            .config
                            .rc_conf
                            .argv(&service, "LOG", &())
                            .ok()
                            .and_then(|v| v.into_iter().next())
                    });
                ServiceStatus {
                    switch: known.then(|| state.config.rc_conf.service_switch(&service)),
                    inhibited: state.is_inhibited(&service),
                    running,
                    backoff: state
                        .get_backoff(&service)
                        .filter(|b| *b > now)
                        .map(|b| b - now),
                    starts: history.starts,
                    last_exit: history
                        .last_exit
                        .map(|(st, when)| (describe_wait_status(st), when.elapsed())),
                    log,
                    service,
                }
            })
            .collect()
    }

    /// Send the specified signal to all processes that match the target.
    pub fn kill(&self, target: Target, signal: minimal_signals::Signal) -> Result<(), Error> {
        self.signal(target, signal).map(|_| ())
    }

    /// Send the specified signal to all processes that match the target, returning how many
    /// processes matched.
    pub fn signal(
        &self,
        mut target: Target,
        signal: minimal_signals::Signal,
    ) -> Result<usize, Error> {
        KILL.click();
        clue!(COLLECTOR, DEBUG, {
            kill: {
                target: indicio::Value::from(&target),
                signal: signal.to_string(),
            },
        });
        let mut err = Ok(());
        let mut matched = 0;
        let state = self.state.lock().unwrap();
        for process in state.processes.iter() {
            if process.pid().is_some() && target.matches(process) {
                matched += 1;
                if let Err(local) = process.kill(signal) {
                    clue!(COLLECTOR, ERROR, {
                        error: indicio::Value::from(&local),
                    });
                    if err.is_ok() {
                        err = Err(local);
                    }
                }
            }
        }
        err.map(|()| matched)
    }

    /// List the available services.
    pub fn list_services(&self) -> Vec<String> {
        LIST_SERVICES.click();
        self.state
            .lock()
            .unwrap()
            .config
            .services()
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    /// List the enabled services.
    pub fn enabled_services(&self) -> Vec<String> {
        ENABLED_SERVICES.click();
        let state = self.state.lock().unwrap();
        state
            .config
            .services()
            .iter()
            .filter(|s| state.service_switch(s).can_be_started())
            .map(|s| s.to_string())
            .collect()
    }

    /// Start the named service.
    pub fn start(&self, service: &str) -> Result<(), Error> {
        START.click();
        {
            let state = self.state.lock().unwrap();
            if let Some(err) = Self::start_refusal(&state, service) {
                return Err(err);
            }
        }
        let context = self.context_for(service, &[]);
        let mut state = self.state.lock().unwrap();
        state.clear_inhibit(service);
        if let Some(err) = Self::start_refusal(&state, service) {
            return Err(err);
        }
        state.spawn(&self.coord, self.reclaim.clone(), service, context)?;
        Ok(())
    }

    fn start_refusal(state: &Pid1State, service: &str) -> Option<Error> {
        if state.shutdown {
            return Some(Error::ShuttingDown);
        }
        // Ignore a stop-inhibit:  starting is how one clears it.
        match state.config.rc_conf.service_switch(service) {
            SwitchPosition::Yes if state.is_running(service) => Some(Error::ServiceAlreadyStarted),
            SwitchPosition::Yes | SwitchPosition::Manual => None,
            SwitchPosition::No => Some(Error::ServiceDisabled),
        }
    }

    /// Compute `service`'s execution context against the current configuration.  This runs the
    /// stub's `rcvar`, so it must never be called with the state lock held.
    fn context_for(&self, service: &str, argv: &[&str]) -> Result<ExecutionContext, Error> {
        let config = Arc::clone(&self.state.lock().unwrap().config);
        ExecutionContext::new(&config, service, argv)
    }

    /// Stop then start the named service.
    pub fn restart(&self, service: &str) -> Result<(), Error> {
        RESTART.click();
        let switch = {
            let state = self.state.lock().unwrap();
            state.service_switch(service)
        };
        if switch == SwitchPosition::No {
            return Err(Error::ServiceDisabled);
        }
        self.stop(service)?;
        let context = (switch == SwitchPosition::Manual).then(|| self.context_for(service, &[]));
        let mut state = self.state.lock().unwrap();
        state.clear_inhibit(service);
        state.clear_backoff(service);
        if let Some(context) = context
            && state.service_switch(service) == SwitchPosition::Manual
        {
            state.spawn(&self.coord, self.reclaim.clone(), service, context)?;
        }
        state.converge = state.converge.wrapping_add(1);
        self.coord.converge.notify_all();
        Ok(())
    }

    /// Stop the named service.
    pub fn stop(&self, service: &str) -> Result<(), Error> {
        STOP.click();
        let service_string = service.to_string();
        let mut processes: Vec<Arc<Execution>> = {
            let mut state = self.state.lock().unwrap();
            if !state.processes.iter().any(|p| p.service == service)
                && state.config.get_service_path(service).is_none()
            {
                return Err(Error::UnknownService);
            }
            if !state.processes.iter().any(|p| p.service == service) {
                return Err(Error::ServiceError("service is not running".to_string()));
            }
            state.set_inhibit(service_string);
            state
                .processes
                .iter()
                .filter(|p| p.service == service)
                .cloned()
                .collect()
        };
        while let Some(proc) = processes.pop() {
            if proc.pid().is_none() {
                continue;
            }
            terminate(&proc)?;
        }
        Ok(())
    }

    #[cfg(test)]
    fn spawn(&self, service: &str, argv: &[&str]) -> Result<(), Error> {
        let context = self.context_for(service, argv);
        self.state
            .lock()
            .unwrap()
            .spawn(&self.coord, self.reclaim.clone(), service, context)?;
        Ok(())
    }
}

///////////////////////////////////////// ExecutionContext /////////////////////////////////////////

/// The set of facts about a service that get used for determining when to restart a process.
#[derive(Clone, Debug, Eq)]
pub struct ExecutionContext {
    /// The path to the executable.
    pub path: CString,
    /// The wrapper to execute with.
    pub wrapper: Vec<CString>,
    /// The args to provide to the command.
    pub argv: Vec<CString>,
    /// The environment to set.
    pub env: Vec<CString>,
    /// Where to send stdout and stderr (opened O_APPEND), from the service's LOG variable.  When
    /// unset, the service inherits rustrc's stdout and stderr.
    pub log: Option<CString>,
    /// How long to wait after SIGTERM before SIGKILL, from the service's STOP_TIMEOUT variable
    /// (seconds; fractions allowed).  When unset, rustrc waits [`DEFAULT_STOP_TIMEOUT`].  Not used
    /// for equality or hashing:  changing it applies to the next stop without a restart.
    pub stop_timeout: Option<Duration>,
    /// The instant that it started (not used for equality or hashing).
    pub started: Instant,
}

impl PartialEq for ExecutionContext {
    fn eq(&self, other: &ExecutionContext) -> bool {
        self.path == other.path
            && self.wrapper == other.wrapper
            && self.argv == other.argv
            && self.env == other.env
            && self.log == other.log
    }
}

impl Hash for ExecutionContext {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.path.hash(h);
        self.wrapper.hash(h);
        self.argv.hash(h);
        self.env.hash(h);
        self.log.hash(h);
    }
}

impl ExecutionContext {
    /// Create a new execution context for a service and argument.
    pub fn new(config: &Pid1Configuration, service: &str, argv: &[&str]) -> Result<Self, Error> {
        // setup path
        let Some(path) = config.get_service_path(service) else {
            UNKNOWN_SERVICE.click();
            return Err(Error::UnknownService);
        };
        let path = match path {
            Ok(path) => path,
            Err(err) => {
                return Err(Error::ServiceError(err.clone()));
            }
        };
        // rc_conf's bind_for_invoke runs the stub with no deadline and races the orphan reaper;
        // run it ourselves and let rc_conf do the binding.
        let keys = helper::stub_rcvars(service, &path, config.stub_timeout)?;
        let keys = keys.iter().map(String::as_str).collect::<Vec<_>>();
        let bound = config.rc_conf.generate_rcvars(service, &keys)?;
        let path = CString::new(path.as_str())?;
        // setup wrapper
        let wrapper = config
            .rc_conf
            .argv(service, "WRAPPER", &())?
            .into_iter()
            .map(|a| CString::new(a.as_bytes()))
            .collect::<Result<Vec<_>, std::ffi::NulError>>()?;
        // setup argv
        let argv = argv
            .iter()
            .map(|a| CString::new(a.as_bytes()))
            .collect::<Result<Vec<_>, std::ffi::NulError>>()?;
        // setup env
        let mut env: Vec<CString> = vec![];
        env.push(CString::new(format!(
            "RCVAR_ARGV0={}",
            rc_conf::var_name_from_service(service)
        ))?);
        for (key, value) in bound.iter() {
            env.push(CString::new(format!("{key}={value}"))?);
        }
        for (key, value) in std::env::vars() {
            if matches!(key.as_str(), "PATH" | "TERM" | "TZ" | "LANG") {
                env.push(CString::new(format!("{key}={value}"))?);
            }
        }
        env.sort();
        // setup log
        let log = config.rc_conf.argv(service, "LOG", &())?;
        let log = match log.len() {
            0 => None,
            1 => Some(CString::new(log[0].as_bytes())?),
            _ => {
                return Err(Error::ServiceError(format!(
                    "LOG must expand to exactly one path; got {log:?}"
                )));
            }
        };
        // setup stop timeout
        let stop_timeout = match config.rc_conf.lookup_suffix(service, "STOP_TIMEOUT") {
            None => None,
            Some(raw) => {
                let raw =
                    shvar::expand_recursive(&config.rc_conf.variable_provider_for(service)?, &raw)?;
                let raw = raw.trim();
                if raw.is_empty() {
                    None
                } else {
                    match raw.parse::<f64>() {
                        Ok(secs) if secs.is_finite() && secs >= 0.0 => {
                            Some(Duration::from_secs_f64(secs))
                        }
                        _ => {
                            return Err(Error::ServiceError(format!(
                                "STOP_TIMEOUT must be a non-negative number of seconds; got {raw:?}"
                            )));
                        }
                    }
                }
            }
        };
        let started = Instant::now();
        Ok(Self {
            path,
            wrapper,
            argv,
            env,
            log,
            stop_timeout,
            started,
        })
    }
}

impl From<&ExecutionContext> for indicio::Value {
    fn from(exec: &ExecutionContext) -> Self {
        fn c_string_to_string(s: &CString) -> String {
            s.to_string_lossy().into_owned()
        }
        // The environment carries configuration, which is where secrets live.  Log which keys a
        // service got, never their values.
        fn env_keys(env: &[CString]) -> indicio::Value {
            env.iter()
                .map(|e| {
                    let e = c_string_to_string(e);
                    e.split_once('=').map_or(e.clone(), |(k, _)| k.to_string())
                })
                .collect::<Vec<_>>()
                .into()
        }
        // A WRAPPER like `/usr/bin/env TOKEN=...` puts values on the command line; redact words
        // shaped like an env(1) assignment.
        fn redacted(strs: &[CString]) -> indicio::Value {
            strs.iter()
                .map(|s| {
                    let s = c_string_to_string(s);
                    match s.split_once('=') {
                        Some((k, _)) if is_env_name(k) => format!("{k}=<redacted>"),
                        _ => s,
                    }
                })
                .collect::<Vec<_>>()
                .into()
        }
        fn is_env_name(k: &str) -> bool {
            let mut chars = k.chars();
            chars
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        }
        value!({
            path: c_string_to_string(&exec.path),
            wrapper: redacted(&exec.wrapper),
            argv: redacted(&exec.argv),
            env: env_keys(&exec.env),
            log: exec.log.as_ref().map(c_string_to_string).unwrap_or_default(),
        })
    }
}

///////////////////////////////////////////// terminate ////////////////////////////////////////////

/// Stop `exec`, returning once the reclaimer has retired it.
///
/// SIGTERM goes to the service's process group immediately; SIGKILL follows after the service's
/// STOP_TIMEOUT (default [`DEFAULT_STOP_TIMEOUT`]) and repeats every second until it is gone.
fn terminate(exec: &Execution) -> Result<(), Error> {
    exec.request_stop();
    let grace = exec.context.stop_timeout.unwrap_or(DEFAULT_STOP_TIMEOUT);
    exec.kill_group(minimal_signals::SIGTERM)?;
    if exec.wait_done(grace) {
        return Ok(());
    }
    clue!(COLLECTOR, INFO, {
        stop_timeout: {
            service: exec.service.as_str(),
            after: format!("{grace:?}"),
        },
    });
    while !exec.wait_done(Duration::ZERO) {
        exec.kill_group(minimal_signals::SIGKILL)?;
        exec.wait_done(Duration::from_secs(1));
    }
    Ok(())
}

/// Parse a signal given as a number, a name (`SIGHUP`), or a bare name (`HUP`), case-insensitively.
pub fn parse_signal(s: &str) -> Option<minimal_signals::Signal> {
    if let Ok(n) = s.parse::<i32>() {
        return minimal_signals::Signal::from_i32(n);
    }
    let upper = s.to_ascii_uppercase();
    let want = if upper.starts_with("SIG") {
        upper
    } else {
        format!("SIG{upper}")
    };
    minimal_signals::Signal::known().find(|sig| sig.name() == want)
}

/////////////////////////////////////////// ServiceStatus //////////////////////////////////////////

/// How a process ended, decoded from a waitpid status.
pub fn describe_wait_status(status: libc::c_int) -> String {
    if libc::WIFEXITED(status) {
        format!("exit {}", libc::WEXITSTATUS(status))
    } else if libc::WIFSIGNALED(status) {
        let sig = libc::WTERMSIG(status);
        match minimal_signals::Signal::from_i32(sig) {
            Some(signal) => format!("killed by {signal}"),
            None => format!("killed by signal {sig}"),
        }
    } else {
        format!("status {status:#x}")
    }
}

/// A point-in-time view of one service.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceStatus {
    /// The service name.
    pub service: String,
    /// The switch from rc.conf, ignoring any stop-inhibit.
    pub switch: Option<SwitchPosition>,
    /// True if the service was stopped by request and will not be restarted until started.
    pub inhibited: bool,
    /// Running executions as (pid, uptime).
    pub running: Vec<(i32, Duration)>,
    /// Time remaining before the supervisor will try to start it again, if backing off.
    pub backoff: Option<Duration>,
    /// Successful spawns since rustrc started.
    pub starts: u64,
    /// The most recent exit, decoded, and how long ago it was.
    pub last_exit: Option<(String, Duration)>,
    /// Where the service's output goes, if LOG is set.
    pub log: Option<String>,
}

impl ServiceStatus {
    /// A one-word summary of the state.
    pub fn state(&self) -> &'static str {
        if !self.running.is_empty() {
            "running"
        } else if self.inhibited {
            "stopped"
        } else if self.backoff.is_some() {
            "backoff"
        } else {
            match self.switch {
                Some(SwitchPosition::Yes) => "down",
                Some(SwitchPosition::Manual) => "manual",
                Some(SwitchPosition::No) => "disabled",
                None => "unknown",
            }
        }
    }
}

//////////////////////////////////////////// ReloadPlan ////////////////////////////////////////////

/// What converging onto a new configuration will do.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReloadPlan {
    /// Enabled services that are not running and will be started, with the remaining backoff if
    /// the start will be delayed.
    pub start: Vec<(String, Option<Duration>)>,
    /// Running services whose execution context changed, with what changed.  They will be
    /// stopped and started.
    pub restart: Vec<(String, Vec<String>)>,
    /// Running services that the new configuration disables or no longer knows.  rustrc does not
    /// stop these on reload; stop them explicitly.
    pub keep_running: Vec<String>,
    /// Services whose execution context could not be computed under the new configuration.
    pub errors: Vec<(String, String)>,
}

impl ReloadPlan {
    /// True if converging will do nothing.
    pub fn is_empty(&self) -> bool {
        self.start.is_empty()
            && self.restart.is_empty()
            && self.keep_running.is_empty()
            && self.errors.is_empty()
    }
}

impl std::fmt::Display for ReloadPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_empty() {
            return writeln!(f, "no changes");
        }
        for (s, backoff) in self.start.iter() {
            match backoff {
                Some(d) => writeln!(f, "start {s} (after {:.1}s backoff)", d.as_secs_f64())?,
                None => writeln!(f, "start {s}")?,
            }
        }
        for (s, why) in self.restart.iter() {
            writeln!(f, "restart {s}: {}", why.join(", "))?;
        }
        for s in self.keep_running.iter() {
            writeln!(
                f,
                "keep-running {s}: disabled or removed; stop it explicitly"
            )?;
        }
        for (s, e) in self.errors.iter() {
            writeln!(f, "{s}: error: {e}")?;
        }
        Ok(())
    }
}

/// Describe how two execution contexts differ, naming env keys but never values.
fn context_changes(old: &ExecutionContext, new: &ExecutionContext) -> Vec<String> {
    fn env_map(env: &[CString]) -> HashMap<String, String> {
        env.iter()
            .map(|e| {
                let e = e.to_string_lossy();
                match e.split_once('=') {
                    Some((k, v)) => (k.to_string(), v.to_string()),
                    None => (e.to_string(), String::new()),
                }
            })
            .collect()
    }
    let mut changes = vec![];
    if old.path != new.path {
        changes.push("stub".to_string());
    }
    if old.wrapper != new.wrapper {
        changes.push("WRAPPER".to_string());
    }
    if old.argv != new.argv {
        changes.push("argv".to_string());
    }
    if old.log != new.log {
        changes.push("LOG".to_string());
    }
    let (a, b) = (env_map(&old.env), env_map(&new.env));
    let mut keys = a
        .keys()
        .chain(b.keys())
        .filter(|k| a.get(*k) != b.get(*k))
        .cloned()
        .collect::<Vec<_>>();
    keys.sort();
    keys.dedup();
    changes.extend(keys);
    changes
}

///////////////////////////////////////////// Execution ////////////////////////////////////////////

/// Where an execution's process is in its lifecycle.
///
/// The pid is only meaningful while the process is `Running`:  the process either runs or is a
/// zombie we have not reaped, so the kernel cannot hand its pid to anyone else.  The waiter moves
/// the state to `Reaped` under the same mutex it reaps under, and every signal is sent under that
/// mutex, so a signal can never land on a recycled pid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProcessState {
    /// posix_spawn has not returned.
    Pending,
    /// posix_spawn failed; there is no process.
    Failed,
    /// The process exists and is ours to signal.  It leads a process group of the same id.
    Running(libc::pid_t),
    /// The process has been reaped.  The pid may already belong to an unrelated process.
    Reaped(libc::pid_t),
}

#[derive(Debug)]
struct Execution {
    service: String,
    context: ExecutionContext,
    process: Mutex<ProcessState>,
    process_changed: Condvar,
    thread: Mutex<Option<JoinHandle<()>>>,
    exit_status: Mutex<Option<libc::c_int>>,
    // Set once rustrc decides to stop this execution.  Its exit is then not a crash.
    stop_requested: AtomicBool,
    // Set once the reclaimer has removed this execution from the process table.
    done: Mutex<bool>,
    done_changed: Condvar,
}

impl Execution {
    fn new(
        _execution_id: ExecutionID,
        _config: Arc<Pid1Configuration>,
        service: String,
        context: ExecutionContext,
    ) -> Self {
        Self {
            service,
            context,
            process: Mutex::new(ProcessState::Pending),
            process_changed: Condvar::new(),
            thread: Mutex::new(None),
            exit_status: Mutex::new(None),
            stop_requested: AtomicBool::new(false),
            done: Mutex::new(false),
            done_changed: Condvar::new(),
        }
    }

    /// The pid, if the process exists and has not been reaped.
    fn pid(&self) -> Option<i32> {
        match *self.process.lock().unwrap() {
            ProcessState::Running(pid) => Some(pid),
            _ => None,
        }
    }

    /// Signal the service's main process.  A no-op once the process has been reaped.
    fn kill(&self, signal: minimal_signals::Signal) -> Result<(), Error> {
        EXECUTION_KILL.click();
        let process = self.process.lock().unwrap();
        let ProcessState::Running(pid) = *process else {
            return Ok(());
        };
        clue!(COLLECTOR, DEBUG, {
            kill: {
                service: self.service.as_str(),
                pid: pid,
                signal: signal.to_string(),
            },
        });
        send_signal(pid, signal.into_i32())
    }

    /// Signal every process in the service's process group.  A no-op once the main process has
    /// been reaped:  after that the group id may be recycled.
    fn kill_group(&self, signal: minimal_signals::Signal) -> Result<(), Error> {
        EXECUTION_KILL.click();
        let process = self.process.lock().unwrap();
        let ProcessState::Running(pid) = *process else {
            return Ok(());
        };
        clue!(COLLECTOR, DEBUG, {
            kill_group: {
                service: self.service.as_str(),
                pgid: pid,
                signal: signal.to_string(),
            },
        });
        // The service may have moved its main process out of the group it was born into; fall
        // back to signaling the process itself when the group is gone or unsignalable.
        match send_signal(-pid, signal.into_i32()) {
            Err(Error::Io(err))
                if matches!(err.raw_os_error(), Some(libc::ESRCH) | Some(libc::EPERM)) =>
            {
                send_signal(pid, signal.into_i32())
            }
            result => result,
        }
    }

    fn exec(self: &Arc<Self>) -> Result<(), Error> {
        EXECUTION_EXEC.click();
        match self.exec_inner() {
            Ok(pid) => {
                clue!(COLLECTOR, INFO, {
                    exec: {
                        service: self.service.as_str(),
                        pid: pid,
                        context: indicio::Value::from(&self.context),
                    },
                });
                self.set_process(ProcessState::Running(pid));
                Ok(())
            }
            Err(err) => {
                clue!(COLLECTOR, ERROR, {
                    exec: {
                        service: self.service.as_str(),
                        context: indicio::Value::from(&self.context),
                    },
                    error: indicio::Value::from(&err),
                });
                self.set_process(ProcessState::Failed);
                Err(err)
            }
        }
    }

    fn exec_inner(self: &Arc<Self>) -> Result<libc::pid_t, Error> {
        // setup exe
        let exe = if self.context.wrapper.is_empty() {
            &self.context.path
        } else {
            &self.context.wrapper[0]
        };
        // setup argv
        let mut argv: Vec<*mut libc::c_char> = vec![];
        for w in self.context.wrapper.iter() {
            argv.push(w.as_ptr() as _);
        }
        argv.push(self.context.path.as_ptr() as _);
        argv.push(c"run".as_ptr() as _);
        for a in self.context.argv.iter() {
            argv.push(a.as_ptr() as _);
        }
        argv.push(std::ptr::null_mut());
        let argv: *const *mut libc::c_char = argv.as_mut_ptr() as _;
        // setup envp
        let mut envp: Vec<*mut libc::c_char> = vec![];
        for e in self.context.env.iter() {
            envp.push(e.as_ptr() as _);
        }
        envp.push(std::ptr::null_mut());
        let envp: *const *mut libc::c_char = envp.as_mut_ptr() as _;
        let actions = FileActions::new(self.context.log.as_ref())?;
        let attr = SpawnAttr::new()?;
        let mut pid: libc::pid_t = -1;
        // SAFETY(rescrv): every pointer is valid for the duration of the call:  exe, argv, and
        // envp borrow CStrings owned by self.context, and actions/attr are initialized guards.
        // posix_spawn* return an errno value rather than setting errno.
        let rc = unsafe {
            libc::posix_spawnp(
                &mut pid,
                exe.as_ptr() as _,
                actions.as_ptr(),
                attr.as_ptr(),
                argv,
                envp,
            )
        };
        if rc != 0 {
            return Err(std::io::Error::from_raw_os_error(rc).into());
        }
        Ok(pid)
    }

    /// Block until posix_spawn has returned.  None means there is no process to wait for.
    fn block_until_spawned(&self) -> Option<libc::pid_t> {
        let mut process = self.process.lock().unwrap();
        loop {
            match *process {
                ProcessState::Pending => {
                    process = self.process_changed.wait(process).unwrap();
                }
                ProcessState::Running(pid) => return Some(pid),
                ProcessState::Failed | ProcessState::Reaped(_) => return None,
            }
        }
    }

    /// Block until the process exits, without reaping it.
    fn await_exit(&self, pid: libc::pid_t) {
        loop {
            let mut info = MaybeUninit::<libc::siginfo_t>::zeroed();
            // SAFETY(rescrv): waitid writes only into info.  WNOWAIT leaves the zombie in place so
            // the pid stays ours until reap() takes the process mutex.
            let rc = unsafe {
                libc::waitid(
                    libc::P_PID,
                    pid as libc::id_t,
                    info.as_mut_ptr(),
                    libc::WEXITED | libc::WNOWAIT,
                )
            };
            if rc == 0 {
                return;
            }
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            clue!(COLLECTOR, ERROR, {
                waitid: {
                    service: self.service.as_str(),
                    pid: pid,
                },
                error: format!("{err:?}"),
            });
            return;
        }
    }

    /// Reap the exited process and retire its pid.  Anything the service left in its process group
    /// is killed first, while the group id is still guaranteed to be ours.
    fn reap(&self, pid: libc::pid_t) {
        let mut process = self.process.lock().unwrap();
        // SAFETY(rescrv): kill observes only integer arguments.  The leader is a zombie we have
        // not reaped, so the group id cannot have been recycled.
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
        let mut status = 0;
        let rc = loop {
            // SAFETY(rescrv): waitpid observes only the supplied pid and status pointer.
            let rc = unsafe { libc::waitpid(pid, &mut status, 0) };
            if rc < 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            break rc;
        };
        *process = ProcessState::Reaped(pid);
        drop(process);
        self.process_changed.notify_all();
        if rc == pid {
            *self.exit_status.lock().unwrap() = Some(status);
            clue!(COLLECTOR, INFO, {
                exited: {
                    service: self.service.as_str(),
                    pid: pid,
                    status: describe_wait_status(status),
                    uptime: format!("{:?}", self.context.started.elapsed()),
                },
            });
        } else {
            clue!(COLLECTOR, ERROR, {
                waitpid: {
                    service: self.service.as_str(),
                    pid: pid,
                },
                error: format!("{:?}", std::io::Error::last_os_error()),
            });
        }
    }

    fn set_process(&self, state: ProcessState) {
        *self.process.lock().unwrap() = state;
        self.process_changed.notify_all();
    }

    fn set_thread(&self, join: JoinHandle<()>) {
        *self.thread.lock().unwrap() = Some(join);
    }

    fn take_thread(&self) -> Option<JoinHandle<()>> {
        std::mem::take(&mut *self.thread.lock().unwrap())
    }

    /// Mark this execution as being stopped on purpose.  True if it was not already.
    fn request_stop(&self) -> bool {
        !self.stop_requested.swap(true, Ordering::AcqRel)
    }

    fn stop_requested(&self) -> bool {
        self.stop_requested.load(Ordering::Acquire)
    }

    fn mark_done(&self) {
        *self.done.lock().unwrap() = true;
        self.done_changed.notify_all();
    }

    /// Wait up to `timeout` for the reclaimer to retire this execution.  True if it has.
    fn wait_done(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut done = self.done.lock().unwrap();
        while !*done {
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            done = self
                .done_changed
                .wait_timeout(done, deadline - now)
                .unwrap()
                .0;
        }
        true
    }
}

/// Send `signal` to `pid` (a process group when negative), treating a vanished target as success.
fn send_signal(pid: libc::pid_t, signal: libc::c_int) -> Result<(), Error> {
    // SAFETY(rescrv): kill observes only integer arguments.
    if unsafe { libc::kill(pid, signal) } < 0 {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() != Some(libc::ESRCH) || pid < 0 {
            return Err(err.into());
        }
    }
    Ok(())
}

fn spawn_check(rc: libc::c_int) -> Result<(), Error> {
    if rc != 0 {
        return Err(std::io::Error::from_raw_os_error(rc).into());
    }
    Ok(())
}

/// posix_spawn file actions for a service:  stdin from /dev/null, and stdout/stderr to LOG if set.
struct FileActions(Box<MaybeUninit<libc::posix_spawn_file_actions_t>>);

impl FileActions {
    fn new(log: Option<&CString>) -> Result<Self, Error> {
        let mut actions = Box::new(MaybeUninit::<libc::posix_spawn_file_actions_t>::uninit());
        // SAFETY(rescrv): init writes into the boxed, stable storage.
        let rc = unsafe { libc::posix_spawn_file_actions_init(actions.as_mut_ptr()) };
        if rc != 0 {
            return Err(std::io::Error::from_raw_os_error(rc).into());
        }
        let this = Self(actions);
        // Services run in their own process group, so a service reading a terminal would be
        // stopped by SIGTTIN.  Give them /dev/null instead of rustrc's stdin.
        spawn_check(unsafe {
            // SAFETY(rescrv): actions is initialized; the path is a static C string.
            libc::posix_spawn_file_actions_addopen(
                this.raw(),
                libc::STDIN_FILENO,
                c"/dev/null".as_ptr(),
                libc::O_RDONLY,
                0,
            )
        })?;
        if let Some(log) = log {
            spawn_check(unsafe {
                // SAFETY(rescrv): actions is initialized; log outlives the posix_spawn call,
                // which copies the path.
                libc::posix_spawn_file_actions_addopen(
                    this.raw(),
                    libc::STDOUT_FILENO,
                    log.as_ptr(),
                    libc::O_WRONLY | libc::O_CREAT | libc::O_APPEND,
                    0o644,
                )
            })?;
            spawn_check(unsafe {
                // SAFETY(rescrv): actions is initialized.
                libc::posix_spawn_file_actions_adddup2(
                    this.raw(),
                    libc::STDOUT_FILENO,
                    libc::STDERR_FILENO,
                )
            })?;
        }
        Ok(this)
    }

    fn raw(&self) -> *mut libc::posix_spawn_file_actions_t {
        self.0.as_ptr() as *mut _
    }

    fn as_ptr(&self) -> *const libc::posix_spawn_file_actions_t {
        self.0.as_ptr()
    }
}

impl Drop for FileActions {
    fn drop(&mut self) {
        // SAFETY(rescrv): constructed only after a successful init.
        unsafe {
            libc::posix_spawn_file_actions_destroy(self.0.as_mut_ptr());
        }
    }
}

/// posix_spawn attributes for a service:  a fresh process group, an empty signal mask, and default
/// dispositions.
struct SpawnAttr(Box<MaybeUninit<libc::posix_spawnattr_t>>);

impl SpawnAttr {
    fn new() -> Result<Self, Error> {
        let mut attr = Box::new(MaybeUninit::<libc::posix_spawnattr_t>::uninit());
        // SAFETY(rescrv): init writes into the boxed, stable storage.
        let rc = unsafe { libc::posix_spawnattr_init(attr.as_mut_ptr()) };
        if rc != 0 {
            return Err(std::io::Error::from_raw_os_error(rc).into());
        }
        let mut this = Self(attr);
        // rustrc blocks every signal so a dedicated thread can sigwait for them.  Children inherit
        // the mask (and inherited SIG_IGN dispositions) across posix_spawn, which would leave
        // SIGTERM undeliverable to services, so reset both.
        let mut empty = MaybeUninit::<libc::sigset_t>::uninit();
        let mut all = MaybeUninit::<libc::sigset_t>::uninit();
        // SAFETY(rescrv): the sigset functions write into the provided storage; attr is
        // initialized.
        let rc = unsafe {
            libc::sigemptyset(empty.as_mut_ptr());
            libc::sigfillset(all.as_mut_ptr());
            // SIGKILL and SIGSTOP cannot have their disposition changed.
            libc::sigdelset(all.as_mut_ptr(), libc::SIGKILL);
            libc::sigdelset(all.as_mut_ptr(), libc::SIGSTOP);
            let mut rc = libc::posix_spawnattr_setsigmask(this.0.as_mut_ptr(), empty.as_ptr());
            if rc == 0 {
                rc = libc::posix_spawnattr_setsigdefault(this.0.as_mut_ptr(), all.as_ptr());
            }
            if rc == 0 {
                // Each service leads its own process group so a stop reaches everything it
                // started, and so terminal job control aimed at rustrc does not reach services.
                rc = libc::posix_spawnattr_setpgroup(this.0.as_mut_ptr(), 0);
            }
            if rc == 0 {
                rc = libc::posix_spawnattr_setflags(
                    this.0.as_mut_ptr(),
                    (libc::POSIX_SPAWN_SETSIGMASK
                        | libc::POSIX_SPAWN_SETSIGDEF
                        | libc::POSIX_SPAWN_SETPGROUP) as _,
                );
            }
            rc
        };
        if rc != 0 {
            return Err(std::io::Error::from_raw_os_error(rc).into());
        }
        Ok(this)
    }

    fn as_ptr(&self) -> *const libc::posix_spawnattr_t {
        self.0.as_ptr()
    }
}

impl Drop for SpawnAttr {
    fn drop(&mut self) {
        // SAFETY(rescrv): constructed only after a successful init.
        unsafe {
            libc::posix_spawnattr_destroy(self.0.as_mut_ptr());
        }
    }
}

////////////////////////////////////////// BackoffTracker //////////////////////////////////////////

/// The penalty a service starts with.
const INITIAL_PENALTY: Duration = Duration::from_secs(1);
/// The most penalty a service can accumulate.
const MAX_PENALTY: Duration = Duration::from_secs(300);
/// The restart delay for a service with no penalty, before jitter.
const MIN_BACKOFF: Duration = Duration::from_secs(1);
/// The longest restart delay, before jitter.
const MAX_BACKOFF: Duration = Duration::from_secs(60);
/// Forget a service's penalty after this long without an exit.  A service that has run this long
/// (at most 1.5 * MAX_BACKOFF of it spent waiting to restart) has earned more uptime credit than
/// MAX_PENALTY, so forgetting it changes no decision; this only bounds memory.
const FORGET_AFTER: Duration = Duration::from_secs(420);

/// Crash accounting.
///
/// Each exit doubles a service's penalty after crediting the uptime it had (credit grows slightly
/// faster than linearly, so a long run clears any penalty).  The restart delay is the penalty
/// clamped to [MIN_BACKOFF, MAX_BACKOFF] with uniform jitter of +/-50%:  a service that crashes
/// after a healthy run restarts in 0.5-1.5s; one that crashes on start backs off to 30-90s.
#[derive(Debug, Default)]
struct BackoffTracker {
    penalties: HashMap<String, (Instant, Duration)>,
}

impl BackoffTracker {
    fn track(&mut self, service: String, credit: Duration) {
        let (last_tracked, penalty) = self
            .penalties
            .entry(service.clone())
            .or_insert((Instant::now(), INITIAL_PENALTY));
        *last_tracked = Instant::now();
        fn compound(duration: Duration) -> Duration {
            Duration::from_micros(
                (duration.as_micros() as f64
                    * std::f64::consts::E.powf(0.05 * duration.as_secs_f64() / 60.))
                    as u64,
            )
        }
        let old_penalty = *penalty;
        *penalty = penalty.saturating_sub(compound(credit));
        *penalty = penalty.saturating_mul(2);
        *penalty = (*penalty).clamp(Duration::ZERO, MAX_PENALTY);
        clue!(COLLECTOR, DEBUG, {
            service: service,
            credit: format!("{:?}", credit),
            adjusted: format!("{:?}", compound(credit)),
            old_penalty: format!("{:?}", old_penalty),
            new_penalty: format!("{:?}", *penalty),
        });
    }

    fn backoff(&self, service: &str) -> Duration {
        use std::hash::{BuildHasher, RandomState};
        let (last_tracked, penalty) = self
            .penalties
            .get(service)
            .cloned()
            .unwrap_or((Instant::now(), Duration::ZERO));
        let base = penalty.clamp(MIN_BACKOFF, MAX_BACKOFF);
        // RandomState is seeded per instance, so this is a fresh draw each call.
        let mut hasher = RandomState::new().build_hasher();
        service.hash(&mut hasher);
        last_tracked.hash(&mut hasher);
        let unit = (hasher.finish() >> 11) as f64 / (1u64 << 53) as f64;
        base.mul_f64(0.5 + unit)
    }

    fn wipe_debts(&mut self) {
        self.penalties
            .retain(|_, (last_tracked, _)| last_tracked.elapsed() < FORGET_AFTER);
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    /// A throwaway rc.conf and rc.d.  Stubs are plain sh scripts, so tests need neither rcscript
    /// nor any rustrc binary on PATH.
    struct Fixture {
        dir: PathBuf,
    }

    impl Fixture {
        fn new(name: &str) -> Self {
            static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir =
                std::env::temp_dir().join(format!("rustrc-test-{name}-{}-{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join("rc.d")).unwrap();
            std::fs::write(dir.join("rc.conf"), "").unwrap();
            Self { dir }
        }

        fn path(&self, name: &str) -> String {
            self.dir.join(name).to_string_lossy().into_owned()
        }

        /// Write an rc.d stub.  `rcvar` and `run` are sh fragments for the two verbs rustrc uses;
        /// `@` in either is replaced with the fixture directory.
        fn stub(&self, name: &str, rcvar: &str, run: &str) {
            use std::os::unix::fs::PermissionsExt;
            let dir = self.dir.to_string_lossy();
            let body = format!(
                "#!/bin/sh\ncase \"$1\" in\nrcvar)\n{}\n;;\nrun)\nshift\n{}\n;;\n*) exit 64 ;;\nesac\n",
                rcvar.replace('@', &dir),
                run.replace('@', &dir),
            );
            let path = self.dir.join("rc.d").join(name);
            std::fs::write(&path, body).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        fn rc_conf(&self, contents: &str) {
            std::fs::write(self.dir.join("rc.conf"), contents).unwrap();
        }

        fn options(&self) -> Pid1Options {
            Pid1Options {
                rc_conf_path: self.path("rc.conf"),
                rc_d_path: self.path("rc.d"),
                ..Pid1Options::default()
            }
        }

        /// Wait for a file the stub writes, returning its trimmed contents.
        fn wait_for_file(&self, name: &str, timeout: Duration) -> String {
            let deadline = Instant::now() + timeout;
            loop {
                if let Ok(contents) = std::fs::read_to_string(self.dir.join(name))
                    && contents.ends_with('\n')
                {
                    return contents.trim().to_string();
                }
                assert!(Instant::now() < deadline, "{name} never appeared");
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    /// True once `pid` no longer names a live process.  Zombies count as gone:  orphans are
    /// reparented to whatever init the test runs under, which may be slow to reap them.
    fn is_gone(pid: libc::pid_t) -> bool {
        match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
            Ok(stat) => stat
                .rsplit_once(')')
                .map(|(_, rest)| rest.trim_start().starts_with('Z'))
                .unwrap_or(true),
            // SAFETY(rescrv): kill with signal 0 only checks for existence.
            Err(_) => (unsafe { libc::kill(pid, 0) }) != 0,
        }
    }

    fn wait_until(what: &str, timeout: Duration, mut f: impl FnMut() -> bool) {
        let deadline = Instant::now() + timeout;
        while !f() {
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn running_pid(pid1: &Pid1, service: &str) -> Option<libc::pid_t> {
        pid1.status()
            .into_iter()
            .find(|s| s.service == service)
            .and_then(|s| s.running.first().map(|(pid, _)| *pid))
    }

    #[test]
    fn stop_signals_the_whole_process_group() {
        minimal_signals::block();
        let fx = Fixture::new("stop-group");
        fx.stub(
            "svc",
            "",
            "sleep 1000 &\necho $! > @/child.pid\nexec sleep 1000",
        );
        fx.rc_conf("svc_ENABLED=\"YES\"\nsvc_STOP_TIMEOUT=\"5\"\n");
        let pid1 = Pid1::new(fx.options()).unwrap();
        let child: libc::pid_t = fx
            .wait_for_file("child.pid", Duration::from_secs(10))
            .parse()
            .unwrap();
        wait_until("svc running", Duration::from_secs(5), || {
            running_pid(&pid1, "svc").is_some()
        });
        pid1.stop("svc").unwrap();
        wait_until("background child to die", Duration::from_secs(2), || {
            is_gone(child)
        });
        pid1.shutdown().unwrap();
    }

    #[test]
    fn leader_exit_takes_the_group_with_it() {
        minimal_signals::block();
        let fx = Fixture::new("leader-exit");
        fx.stub(
            "svc",
            "",
            "sleep 1000 &\necho $! > @/child.pid\nsleep 0.2\nexit 0",
        );
        fx.rc_conf("svc_ENABLED=\"MANUAL\"\n");
        let pid1 = Pid1::new(fx.options()).unwrap();
        pid1.start("svc").unwrap();
        let child: libc::pid_t = fx
            .wait_for_file("child.pid", Duration::from_secs(10))
            .parse()
            .unwrap();
        wait_until("straggler to die", Duration::from_secs(5), || {
            is_gone(child)
        });
        pid1.shutdown().unwrap();
    }

    #[test]
    fn reaped_pids_are_not_signalable() {
        minimal_signals::block();
        let fx = Fixture::new("reaped");
        fx.stub("svc", "", "sleep 0.3\nexit 0");
        fx.rc_conf("svc_ENABLED=\"MANUAL\"\n");
        let pid1 = Pid1::new(fx.options()).unwrap();
        pid1.start("svc").unwrap();
        let pid = running_pid(&pid1, "svc").expect("svc should be running");
        wait_until("svc to exit", Duration::from_secs(5), || {
            pid1.status()
                .iter()
                .any(|s| s.service == "svc" && s.last_exit.is_some())
        });
        assert_eq!(
            0,
            pid1.signal(Target::Pid(pid), minimal_signals::SIGTERM)
                .unwrap()
        );
        assert_eq!(
            0,
            pid1.signal(Target::One("svc".to_string()), minimal_signals::SIGTERM)
                .unwrap()
        );
        pid1.shutdown().unwrap();
    }

    #[test]
    fn a_stub_that_ignores_rcvar_does_not_wedge_rustrc() {
        minimal_signals::block();
        let fx = Fixture::new("naive-stub");
        // A stub that runs its daemon whatever verb it is given.
        fx.stub(
            "naive",
            "echo $$ > @/naive.tmp && mv @/naive.tmp @/naive.pid\nexec sleep 1000",
            "exec sleep 1000",
        );
        fx.stub("good", "", "exec sleep 1000");
        fx.rc_conf(
            "naive_ENABLED=\"YES\"\ngood_ENABLED=\"YES\"\nnaive_STOP_TIMEOUT=\"5\"\ngood_STOP_TIMEOUT=\"5\"\n",
        );
        let options = Pid1Options {
            stub_timeout_ms: 300,
            ..fx.options()
        };
        let pid1 = Pid1::new(options).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            let start = Instant::now();
            let _ = pid1.status();
            assert!(
                start.elapsed() < Duration::from_millis(250),
                "status blocked for {:?}",
                start.elapsed()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        wait_until("good to start", Duration::from_secs(5), || {
            running_pid(&pid1, "good").is_some()
        });
        assert!(running_pid(&pid1, "naive").is_none());
        let helper: libc::pid_t = fx
            .wait_for_file("naive.pid", Duration::from_secs(5))
            .parse()
            .unwrap();
        wait_until(
            "the hung rcvar to be killed",
            Duration::from_secs(2),
            || is_gone(helper),
        );
        let start = Instant::now();
        pid1.shutdown().unwrap();
        assert!(start.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn stop_sends_sigterm_immediately() {
        minimal_signals::block();
        let fx = Fixture::new("stop-prompt");
        fx.stub("svc", "", "exec sleep 1000");
        fx.rc_conf("svc_ENABLED=\"YES\"\n");
        let pid1 = Pid1::new(fx.options()).unwrap();
        wait_until("svc running", Duration::from_secs(5), || {
            running_pid(&pid1, "svc").is_some()
        });
        let start = Instant::now();
        pid1.stop("svc").unwrap();
        assert!(
            start.elapsed() < Duration::from_millis(500),
            "stop took {:?}",
            start.elapsed()
        );
        pid1.shutdown().unwrap();
    }

    #[test]
    fn stop_escalates_to_sigkill_after_stop_timeout() {
        minimal_signals::block();
        let fx = Fixture::new("stop-escalate");
        fx.stub(
            "svc",
            "",
            "trap '' TERM\necho $$ > @/svc.pid\nwhile :; do sleep 0.05; done",
        );
        fx.rc_conf("svc_ENABLED=\"YES\"\nsvc_STOP_TIMEOUT=\"0.5\"\n");
        let pid1 = Pid1::new(fx.options()).unwrap();
        let pid: libc::pid_t = fx
            .wait_for_file("svc.pid", Duration::from_secs(5))
            .parse()
            .unwrap();
        let start = Instant::now();
        pid1.stop("svc").unwrap();
        let took = start.elapsed();
        assert!(
            took >= Duration::from_millis(450) && took < Duration::from_secs(2),
            "stop took {took:?}"
        );
        assert!(is_gone(pid));
        pid1.shutdown().unwrap();
    }

    #[test]
    fn a_slow_reconfigure_restart_does_not_block_other_starts() {
        minimal_signals::block();
        let fx = Fixture::new("async-restart");
        // slow ignores SIGTERM, so its restart takes the full STOP_TIMEOUT.
        fx.stub(
            "slow",
            "echo \"${RCVAR_ARGV0}_X\"",
            "trap '' TERM\nwhile :; do sleep 0.05; done",
        );
        fx.stub("fast", "", "exec sleep 1000");
        fx.rc_conf(
            "slow_ENABLED=\"YES\"\nslow_X=\"1\"\nslow_STOP_TIMEOUT=\"3\"\nfast_ENABLED=\"NO\"\n",
        );
        let pid1 = Pid1::new(fx.options()).unwrap();
        wait_until("slow running", Duration::from_secs(5), || {
            running_pid(&pid1, "slow").is_some()
        });
        let old = running_pid(&pid1, "slow").unwrap();
        fx.rc_conf(
            "slow_ENABLED=\"YES\"\nslow_X=\"2\"\nslow_STOP_TIMEOUT=\"3\"\nfast_ENABLED=\"YES\"\nfast_STOP_TIMEOUT=\"5\"\n",
        );
        let reloaded = Instant::now();
        let plan = pid1.reload_with_plan().unwrap();
        assert_eq!(
            vec![("slow".to_string(), vec!["slow_X".to_string()])],
            plan.restart
        );
        wait_until("fast to start", Duration::from_millis(1500), || {
            running_pid(&pid1, "fast").is_some()
        });
        // slow comes back on the new context as soon as its stop completes, with no backoff.
        wait_until("slow to restart", Duration::from_secs(6), || {
            running_pid(&pid1, "slow").is_some_and(|pid| pid != old)
        });
        let took = reloaded.elapsed();
        assert!(took < Duration::from_secs(5), "restart took {took:?}");
        pid1.shutdown().unwrap();
    }

    #[test]
    fn nothing_starts_once_shutdown_begins() {
        minimal_signals::block();
        let fx = Fixture::new("fence");
        fx.stub("svc", "", "echo $$ > @/svc.pid\nexec sleep 1000");
        fx.stub("manual", "", "exec sleep 1000");
        fx.rc_conf("svc_ENABLED=\"YES\"\nmanual_ENABLED=\"MANUAL\"\n");
        let pid1 = Pid1::new(fx.options()).unwrap();
        let pid: libc::pid_t = fx
            .wait_for_file("svc.pid", Duration::from_secs(5))
            .parse()
            .unwrap();
        pid1.begin_shutdown();
        assert!(matches!(pid1.start("manual"), Err(Error::ShuttingDown)));
        // A service that dies after shutdown begins stays down.
        // SAFETY(rescrv): pid is a live child we just read from the stub.
        unsafe {
            libc::kill(pid, libc::SIGKILL);
        }
        wait_until("svc to be reclaimed", Duration::from_secs(5), || {
            running_pid(&pid1, "svc").is_none()
        });
        std::thread::sleep(Duration::from_millis(300));
        assert!(pid1.status().iter().all(|s| s.running.is_empty()));
        assert_eq!(
            1,
            pid1.status()
                .iter()
                .find(|s| s.service == "svc")
                .unwrap()
                .starts
        );
        pid1.shutdown().unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn services_read_dev_null() {
        minimal_signals::block();
        let fx = Fixture::new("stdin");
        fx.stub(
            "svc",
            "",
            "readlink /proc/self/fd/0 > @/stdin.tmp && mv @/stdin.tmp @/stdin\nexec sleep 1000",
        );
        fx.rc_conf("svc_ENABLED=\"YES\"\nsvc_STOP_TIMEOUT=\"5\"\n");
        let pid1 = Pid1::new(fx.options()).unwrap();
        assert_eq!(
            "/dev/null",
            fx.wait_for_file("stdin", Duration::from_secs(10))
        );
        pid1.shutdown().unwrap();
    }

    #[test]
    fn smoke_test() {
        minimal_signals::block();
        let options = Pid1Options::default();
        let pid1 = Pid1::new(options).expect("pid1 new should work");
        pid1.reload().expect("reload should work");
        pid1.spawn("rustrc-smoke-test", &["--argument", "GOODBYE WORLD"])
            .expect("spawn should work");
        pid1.shutdown().expect("shutdown should work");
    }

    #[test]
    fn smoking_test() {
        minimal_signals::block();
        let options = Pid1Options::default();
        let pid1 = Pid1::new(options).expect("pid1 new should work");
        pid1.reload().expect("reload should work");
        pid1.spawn("rustrc_smoking_test", &["--argument", "GOODBYE WORLD"])
            .expect("spawn should work");
        pid1.shutdown().expect("shutdown should work");
    }

    #[test]
    fn smoking_wrapper() {
        minimal_signals::block();
        let options = Pid1Options::default();
        let pid1 = Pid1::new(options).expect("pid1 new should work");
        pid1.reload().expect("reload should work");
        pid1.spawn("rustrc_smoking_wrapper", &["--argument", "FROM THE ARGS"])
            .expect("spawn should work");
        pid1.shutdown().expect("shutdown should work");
    }

    #[test]
    fn logged_contexts_never_carry_values() {
        let cs = |v: &[&str]| {
            v.iter()
                .map(|s| CString::new(*s).unwrap())
                .collect::<Vec<_>>()
        };
        let context = ExecutionContext {
            path: CString::new("/rc.d/svc").unwrap(),
            wrapper: cs(&["/usr/bin/env", "TOKEN=hunter2", "--flag=visible"]),
            argv: cs(&["PASSWORD=swordfish"]),
            env: cs(&["svc_API_KEY=sk-live-123", "PATH=/usr/bin"]),
            log: None,
            stop_timeout: None,
            started: Instant::now(),
        };
        let logged = indicio::Value::from(&context).to_string();
        for secret in ["hunter2", "swordfish", "sk-live-123", "/usr/bin\""] {
            assert!(!logged.contains(secret), "{secret} leaked: {logged}");
        }
        for visible in [
            "svc_API_KEY",
            "TOKEN=<redacted>",
            "--flag=visible",
            "/rc.d/svc",
        ] {
            assert!(logged.contains(visible), "{visible} missing: {logged}");
        }
    }

    #[test]
    fn a_healthy_service_restarts_within_about_a_second() {
        let mut bt = BackoffTracker::default();
        let mut seen = HashSet::new();
        for _ in 0..200 {
            bt.track("svc".to_string(), Duration::from_secs(3600));
            let delay = bt.backoff("svc");
            assert!(
                delay >= Duration::from_millis(500) && delay < Duration::from_millis(1500),
                "{delay:?}"
            );
            seen.insert(delay);
        }
        assert!(seen.len() > 100, "jitter is not jittering");
    }

    #[test]
    fn a_crash_loop_backs_off_to_the_cap_and_uptime_forgives_it() {
        let mut bt = BackoffTracker::default();
        let mut ceilings = vec![];
        for _ in 0..12 {
            bt.track("svc".to_string(), Duration::ZERO);
            let (_, penalty) = bt.penalties["svc"];
            ceilings.push(penalty.clamp(MIN_BACKOFF, MAX_BACKOFF));
            let delay = bt.backoff("svc");
            assert!(delay < MAX_BACKOFF.mul_f64(1.5), "{delay:?}");
        }
        assert!(ceilings.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(MAX_BACKOFF, *ceilings.last().unwrap());
        assert!(bt.backoff("svc") >= MAX_BACKOFF / 2);
        // Ten minutes of uptime clears the maximum penalty.
        bt.track("svc".to_string(), Duration::from_secs(600));
        assert!(bt.backoff("svc") < Duration::from_millis(1500));
    }

    #[test]
    fn penalties_survive_a_backoff_wait_and_are_forgotten_later() {
        let mut bt = BackoffTracker::default();
        for _ in 0..8 {
            bt.track("svc".to_string(), Duration::ZERO);
        }
        bt.wipe_debts();
        assert!(bt.penalties.contains_key("svc"));
        if let Some(long_ago) = Instant::now().checked_sub(FORGET_AFTER + Duration::from_secs(1)) {
            bt.penalties.get_mut("svc").unwrap().0 = long_ago;
            bt.wipe_debts();
            assert!(!bt.penalties.contains_key("svc"));
        }
    }

    #[test]
    fn a_crash_restarts_promptly() {
        minimal_signals::block();
        let fx = Fixture::new("crash");
        fx.stub("svc", "", "sleep 0.3\nexit 1");
        fx.rc_conf("svc_ENABLED=\"YES\"\n");
        let pid1 = Pid1::new(fx.options()).unwrap();
        let starts = || {
            pid1.status()
                .iter()
                .find(|s| s.service == "svc")
                .map_or(0, |s| s.starts)
        };
        wait_until("a second start", Duration::from_secs(4), || starts() >= 2);
        pid1.shutdown().unwrap();
    }
}
