use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::hash::{Hash, Hasher};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Condvar, Mutex, WaitTimeoutResult};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use indicio::{clue, value, ERROR, INFO};
use one_two_eight::generate_id;
use rc_conf::{load_services, RcConf, SwitchPosition};
use utf8path::Path;

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

pub fn register_biometrics(collector: &biometrics::Collector) {
    collector.register_counter(&IO_ERROR);
    collector.register_counter(&SHVAR_ERROR);
    collector.register_counter(&RC_CONF_ERROR);
    collector.register_counter(&NUL_ERROR);
    collector.register_counter(&STATE_NEW);
    collector.register_counter(&INHIBITED_SERVICE);
    collector.register_counter(&WAITPID_ENTER);
    collector.register_counter(&WAITPID_EXIT);
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
}

////////////////////////////////////////////// indicio /////////////////////////////////////////////

pub static COLLECTOR: indicio::Collector = indicio::Collector::new();

//////////////////////////////////////////// ExecutionID ///////////////////////////////////////////

generate_id! {ExecutionID, "execution:"}

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Debug)]
pub enum Error {
    GeneratingExecutionID,
    UnknownService,
    ServiceDisabled,
    ServiceAlreadyStarted,
    ServiceError(String),
    Io(std::io::Error),
    Shvar(shvar::Error),
    RcConf(rc_conf::Error),
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

impl From<Error> for indicio::Value {
    fn from(err: Error) -> Self {
        fn shvar_to_value(err: shvar::Error) -> indicio::Value {
            match err {
                shvar::Error::OpenSingleQuotes => { indicio::value!({open_single_quotes: true}) },
                shvar::Error::OpenDoubleQuotes => { indicio::value!({open_double_quotes: true}) },
                shvar::Error::TrailingRightBrace => { indicio::value!({trailing_right_brace: true}) },
                shvar::Error::InvalidVariable => { indicio::value!({invalid_variable: true}) },
                shvar::Error::InvalidCharacter {
                    expected,
                    returned: Some(returned),
                } => { indicio::value!({invalid_charcater: { expected: expected, returned: returned }}) },
                shvar::Error::InvalidCharacter {
                    expected,
                    returned: None,
                } => { indicio::value!({invalid_charcater: { expected: expected }}) },
                shvar::Error::DepthLimitExceeded => { indicio::value!({depth_limit_exceeded: true}) },
                shvar::Error::Requested(message) => { indicio::value!({requested: message}) },
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
            },
            Error::ServiceDisabled => {
                indicio::value!({
                    service_disabled: true,
                })
            },
            Error::ServiceAlreadyStarted => {
                indicio::value!({
                    service_already_started: true,
                })
            },
            Error::ServiceError(msg) => {
                indicio::value!({
                    service_error: msg,
                })
            },
            Error::Io(err) => {
                indicio::value!({
                    io: format!("{:?}", err),
                })
            },
            Error::Shvar(err) => {
                indicio::value!({
                    shvar: shvar_to_value(err),
                })
            },
            Error::RcConf(err) => {
                let inner = match err {
                    rc_conf::Error::FileTooLarge { path } => {
                        indicio::value!({
                            path: path.as_str(),
                            file_too_large: true,
                        })
                    },
                    rc_conf::Error::TrailingWhack { path } => {
                        indicio::value!({
                            path: path.as_str(),
                            trailing_whack: true,
                        })
                    },
                    rc_conf::Error::ProhibitedCharacter { path, line, string, character } => {
                        indicio::value!({
                            path: path.as_str(),
                            line: line,
                            prohibited_character: {
                                string: string,
                                character: character,
                            },
                        })
                    },
                    rc_conf::Error::InvalidRcConf { path, line, message } => {
                        indicio::value!({
                            path: path.as_str(),
                            line: line,
                            invalid_rc_conf: message,
                        })
                    },
                    rc_conf::Error::InvalidRcScript { path, line, message } => {
                        indicio::value!({
                            path: path.as_str(),
                            line: line,
                            invalid_rc_Script: message,
                        })
                    },
                    rc_conf::Error::InvalidInvocation { message } => {
                        indicio::value!({
                            invalid_invocation: message,
                        })
                    },
                    rc_conf::Error::IoError(err) => {
                        indicio::value!({
                            io: format!("{:?}", err),
                        })
                    },
                    rc_conf::Error::ShvarError(err) => {
                        indicio::value!({
                            shvar: shvar_to_value(err),
                        })
                    },
                    rc_conf::Error::Utf8Error(err) => {
                        indicio::value!({
                            utf8: format!("{:?}", err),
                        })
                    },
                    rc_conf::Error::FromUtf8Error(err) => {
                        indicio::value!({
                            from_utf8: format!("{:?}", err),
                        })
                    },
                };
                indicio::value!({
                    rc_conf: inner,
                })
            },
            Error::NulError => {
                indicio::value!({
                    generating_execution_id: true,
                })
            },
        }
    }
}

////////////////////////////////////////////// Target //////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub enum Target {
    #[default]
    All,
    One(String),
    Pid(i32),
}

impl Target {
    pub fn matches(&mut self, e: &Execution) -> bool {
        match self {
            Target::All => true,
            Target::One(s) => *s == e.service,
            Target::Pid(p) => Some(*p) == e.pid(),
        }
    }

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

#[derive(Clone, Debug, Eq, PartialEq, arrrg_derive::CommandLine)]
pub struct Pid1Options {
    #[arrrg(optional, "A colon-separated PATH-like list of rc.conf files to be loaded in order.  Later files override.")]
    pub rc_conf_path: String,
    #[arrrg(optional, "A colon-separated PATH-like list of rc.d directories to be scanned in order.  Earlier files short-circuit.")]
    pub rc_d_path: String,
}

impl Default for Pid1Options {
    fn default() -> Self {
        Self {
            rc_conf_path: "rc.conf".to_string(),
            rc_d_path: "rc.d".to_string(),
        }
    }
}

impl From<&Pid1Options> for indicio::Value {
    fn from(options: &Pid1Options) -> Self {
        value!({
            rc_conf_path: options.rc_conf_path.as_str(),
            rc_d_path: options.rc_d_path.as_str(),
        })
    }
}

///////////////////////////////////////// Pid1Configuration ////////////////////////////////////////

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pid1Configuration {
    services: HashMap<String, Result<Path<'static>, String>>,
    rc_conf: RcConf,
}

impl Pid1Configuration {
    pub fn from_options(options: &Pid1Options) -> Result<Self, rc_conf::Error> {
        let services = load_services(&options.rc_d_path)?;
        let rc_conf = RcConf::parse(&options.rc_conf_path)?;
        Ok(Self { services, rc_conf })
    }
}

///////////////////////////////////////////// Pid1State ////////////////////////////////////////////

#[derive(Debug)]
struct Pid1State {
    shutdown: bool,
    converge: u64,
    config: Arc<Pid1Configuration>,
    processes: Vec<Arc<Execution>>,
    inhibited: HashSet<String>,
    backedoff: HashMap<String, Instant>,
}

impl Pid1State {
    fn new(config: Arc<Pid1Configuration>) -> Self {
        let shutdown = false;
        let converge = 1;
        let processes = vec![];
        let inhibited = HashSet::new();
        let backedoff = HashMap::new();
        STATE_NEW.click();
        Self {
            shutdown,
            converge,
            config,
            processes,
            inhibited,
            backedoff,
        }
    }

    fn has_processes(&self) -> bool {
        !self.processes.is_empty()
    }

    fn has_process(&self, proc: &Arc<Execution>) -> bool {
        self.processes.iter().any(|p| Arc::ptr_eq(p, proc))
    }

    fn is_running(&self, service: &str) -> bool {
        self.processes.iter().any(|p| p.service == service)
    }

    fn service_switch(&self, service: &str) -> SwitchPosition {
        if self.is_inhibited(service) {
            INHIBITED_SERVICE.click();
            clue!(COLLECTOR, INFO, {
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
        clue!(COLLECTOR, INFO, {
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

    fn spawn(
        &mut self,
        reclaim: SyncSender<Arc<Execution>>,
        service: &str,
        argv: &[&str],
    ) -> Result<ExecutionID, Error> {
        self.clear_backoff(service);
        let execution_id = ExecutionID::generate().ok_or(Error::GeneratingExecutionID)?;
        let config = Arc::clone(&self.config);
        let service = service.to_string();
        let context = ExecutionContext::new(&config, &service, argv)?;
        clue!(COLLECTOR, INFO, {
            spawn: indicio::Value::from(&context),
        });
        let execution = Arc::new(Execution::new(execution_id, config, service, context));
        let exec = Arc::clone(&execution);
        let thread = std::thread::Builder::new()
            .stack_size(65536)
            .spawn(move || Self::wait(exec, reclaim))?;
        execution.set_thread(thread);
        execution.exec()?;
        self.processes.push(execution);
        Ok(execution_id)
    }

    fn wait(exec: Arc<Execution>, reclaim: SyncSender<Arc<Execution>>) {
        let pid = exec.block_until_have_pid();
        if pid > 0 {
            let mut status = 0;
            WAITPID_ENTER.click();
            unsafe {
                if libc::waitpid(pid, &mut status, 0) < 0 {
                    // TODO(rescrv): log that this failed.
                    // TODO(rescrv): backoff and retry in a loop.
                }
            }
            clue!(COLLECTOR, INFO, {
                waitpid: {
                    pid: pid,
                },
                exec: indicio::Value::from(&exec.context),
            });
            WAITPID_EXIT.click();
        } else {
            NON_POSITIVE_PID.click();
        }
        reclaim.send(exec).unwrap();
    }
}

///////////////////////////////////////////// Pid1State ////////////////////////////////////////////

#[derive(Debug, Default)]
struct Pid1Coordination {
    converge: Condvar,
    backoff: Mutex<BackoffTracker>,
}

/////////////////////////////////////////////// Pid1 ///////////////////////////////////////////////

#[derive(Debug)]
pub struct Pid1 {
    options: Mutex<Pid1Options>,
    state: Arc<Mutex<Pid1State>>,
    coord: Arc<Pid1Coordination>,
    // Reclaim threads that waitpid on processes.
    reclaim: SyncSender<Arc<Execution>>,
    reclaimer: JoinHandle<()>,
    // Converge to the configuration regularly, respawning when necessary.
    converger: JoinHandle<()>,
}

impl Pid1 {
    pub fn new(options: Pid1Options) -> Result<Self, Error> {
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
            let backoff = {
                let mut backoff = coord.backoff.lock().unwrap();
                backoff.track(exec.service.to_string(), exec.context.started.elapsed());
                backoff.wipe_debts();
                backoff.backoff(&exec.service)
            };
            let service = exec.service.to_string();
            {
                let mut state = state.lock().unwrap();
                state.processes.retain(|p| !Arc::ptr_eq(p, &exec));
                state.converge = state.converge.saturating_add(1);
                state.set_backoff(service, Instant::now() + backoff);
                coord.converge.notify_all();
                if state.converge == u64::MAX {
                    todo!();
                }
            }
        }
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
                clue!(COLLECTOR, INFO, { wait: format!("{:?}", wait), });
                while !state.shutdown && converge >= state.converge {
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
            match Self::converge(&reclaim, &state) {
                Ok(w) => {
                    wait = w;
                    converge = c;
                },
                Err(err) => {
                    clue!(COLLECTOR, ERROR, {
                        error: indicio::Value::from(err),
                    });
                },
            }
            wait = std::cmp::max(wait, Duration::from_secs(1));
        }
    }

    fn converge(
        reclaim: &SyncSender<Arc<Execution>>,
        state: &Mutex<Pid1State>,
    ) -> Result<Duration, Error> {
        CONVERGE.click();
        let (processes, config) = {
            let state = state.lock().unwrap();
            (state.processes.clone(), Arc::clone(&state.config))
        };
        clue!(COLLECTOR, INFO, {
            converge: true,
            services: indicio::Value::from(config.services.keys().collect::<Vec<_>>()),
        });
        fn has_process(state: &Mutex<Pid1State>, exec: &Arc<Execution>) -> bool {
            let state = state.lock().unwrap();
            state.has_process(exec)
        }
        for exec in processes {
            let current_context = ExecutionContext::new(&config, &exec.service, &[])?;
            if current_context != exec.context {
                let Some(pid) = exec.pid() else {
                    clue!(COLLECTOR, ERROR, {
                        error: {
                            human: format!("failed to converge {}; manually reload and restart", exec.service),
                        }
                    });
                    continue;
                };
                clue!(COLLECTOR, INFO, {
                    converge: {
                        old: indicio::Value::from(&exec.context),
                        new: indicio::Value::from(&current_context),
                        pid: pid,
                    },
                });
                for iter in 1..=3 {
                    for _ in 0..(1 << iter) * 10 {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        if !has_process(state, &exec) {
                            break;
                        }
                    }
                    exec.kill(minimal_signals::SIGTERM)?;
                }
                while has_process(state, &exec) {
                    exec.kill(minimal_signals::SIGKILL)?;
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            }
        }
        let now = Instant::now();
        let mut min = Instant::now() + Duration::from_secs(300);
        for service in config.services.keys() {
            let mut state = state.lock().unwrap();
            if state.is_inhibited(service) {
                clue!(COLLECTOR, INFO, {
                    started: false,
                    service: service,
                    inhibited: true,
                });
            } else if state.service_switch(service) == SwitchPosition::Yes
                && !state.is_running(service)
            {
                RESPAWNING.click();
                let mut delay = false;
                if let Some(backoff_until) = state.get_backoff(service) {
                    if backoff_until > now {
                        clue!(COLLECTOR, INFO, {
                            started: false,
                            service: service,
                            delayed: format!("{:?}", backoff_until - now),
                        });
                        min = std::cmp::min(min, backoff_until);
                        delay = true;
                    }
                }
                if !delay {
                    clue!(COLLECTOR, INFO, {
                        started: true,
                        service: service,
                    });
                    state.spawn(reclaim.clone(), service, &[])?;
                }
            }
        }
        Ok(min.saturating_duration_since(now))
    }

    pub fn shutdown(self) -> Result<(), Error> {
        {
            let mut state = self.state.lock().unwrap();
            state.shutdown = true;
        }
        'outer: for iter in 1..=3 {
            for _ in 0..(1 << iter) * 10 {
                std::thread::sleep(std::time::Duration::from_millis(100));
                if !self.has_processes() {
                    break 'outer;
                }
            }
            let _ = self.kill(Target::All, minimal_signals::SIGTERM);
        }
        while self.has_processes() {
            let _ = self.kill(Target::All, minimal_signals::SIGKILL);
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        let Pid1 {
            options: _,
            state: _,
            coord,
            reclaim,
            reclaimer,
            converger,
        } = self;
        coord.converge.notify_all();
        drop(reclaim);
        reclaimer.join().unwrap();
        converger.join().unwrap();
        Ok(())
    }

    pub fn reconfigure(&self, options: Pid1Options) -> Result<(), Error> {
        RECONFIGURE.click();
        clue!(COLLECTOR, INFO, {
            reconfigure: indicio::Value::from(&options),
        });
        {
            let options2 = options.clone();
            *self.options.lock().unwrap() = options2;
        }
        self.reload()
    }

    pub fn reload(&self) -> Result<(), Error> {
        RELOAD.click();
        clue!(COLLECTOR, INFO, {
            reload: true,
        });
        let options = { self.options.lock().unwrap().clone() };
        let config = Arc::new(Pid1Configuration::from_options(&options)?);
        {
            let mut state = self.state.lock().unwrap();
            state.config = config;
            state.converge = state.converge.saturating_add(1);
            if state.converge == u64::MAX {
                todo!();
            }
        }
        self.coord.converge.notify_all();
        Ok(())
    }

    pub fn kill(&self, mut target: Target, signal: minimal_signals::Signal) -> Result<(), Error> {
        KILL.click();
        clue!(COLLECTOR, INFO, {
            kill: {
                target: indicio::Value::from(&target),
                signal: signal.to_string(),
            },
        });
        let mut err = Ok(());
        let state = self.state.lock().unwrap();
        for process in state.processes.iter() {
            if target.matches(process) {
                let pid: libc::pid_t = *process.pid.lock().unwrap();
                if pid > 0 {
                    unsafe {
                        clue!(COLLECTOR, INFO, {
                            kill: {
                                pid: pid,
                                signal: signal.to_string(),
                            },
                        });
                        if libc::kill(pid, signal.into_i32()) < 0 && err.is_ok() {
                            err = Err(std::io::Error::last_os_error().into());
                        }
                    }
                }
            }
        }
        err
    }

    pub fn list_services(&self) -> Vec<String> {
        LIST_SERVICES.click();
        self.state
            .lock()
            .unwrap()
            .config
            .services
            .keys()
            .cloned()
            .collect()
    }

    pub fn enabled_services(&self) -> Vec<String> {
        ENABLED_SERVICES.click();
        let state = self.state.lock().unwrap();
        state
            .config
            .services
            .keys()
            .filter(|s| state.service_switch(s).is_enabled())
            .cloned()
            .collect()
    }

    pub fn start(&self, service: &str) -> Result<(), Error> {
        START.click();
        let mut state = self.state.lock().unwrap();
        state.clear_inhibit(service);
        match state.service_switch(service) {
            SwitchPosition::Yes => {
                if !state.is_running(service) {
                    state.spawn(self.reclaim.clone(), service, &[])?;
                    Ok(())
                } else {
                    Err(Error::ServiceAlreadyStarted)
                }
            }
            SwitchPosition::Manual => {
                state.spawn(self.reclaim.clone(), service, &[])?;
                Ok(())
            }
            SwitchPosition::No => Err(Error::ServiceDisabled),
        }
    }

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
        let mut state = self.state.lock().unwrap();
        state.clear_inhibit(service);
        if state.service_switch(service) == SwitchPosition::Manual {
            state.spawn(self.reclaim.clone(), service, &[])?;
        }
        self.coord.converge.notify_all();
        Ok(())
    }

    pub fn stop(&self, service: &str) -> Result<(), Error> {
        STOP.click();
        let service_string = service.to_string();
        let mut processes: Vec<Arc<Execution>> = {
            let mut state = self.state.lock().unwrap();
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
                todo!();
            }
            for iter in 1..=3 {
                for _ in 0..(1 << iter) * 10 {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    if !self.has_process(&proc) {
                        break;
                    }
                }
                proc.kill(minimal_signals::SIGTERM)?;
            }
            while self.has_process(&proc) {
                proc.kill(minimal_signals::SIGKILL)?;
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
        Ok(())
    }

    #[cfg(test)]
    fn spawn(&self, service: &str, argv: &[&str]) -> Result<(), Error> {
        self.state
            .lock()
            .unwrap()
            .spawn(self.reclaim.clone(), service, argv)?;
        Ok(())
    }

    fn has_processes(&self) -> bool {
        self.state.lock().unwrap().has_processes()
    }

    fn has_process(&self, proc: &Arc<Execution>) -> bool {
        self.state.lock().unwrap().has_process(proc)
    }
}

///////////////////////////////////////// ExecutionContext /////////////////////////////////////////

#[derive(Clone, Debug, Eq)]
pub struct ExecutionContext {
    pub path: CString,
    pub wrapper: Vec<CString>,
    pub argv: Vec<CString>,
    pub env: Vec<CString>,
    pub started: Instant,
}

impl PartialEq for ExecutionContext {
    fn eq(&self, other: &ExecutionContext) -> bool {
        self.path == other.path
            && self.wrapper == other.wrapper
            && self.argv == other.argv
            && self.env == other.env
    }
}

impl Hash for ExecutionContext {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.path.hash(h);
        self.wrapper.hash(h);
        self.argv.hash(h);
        self.env.hash(h);
    }
}

impl ExecutionContext {
    pub fn new(config: &Pid1Configuration, service: &str, argv: &[&str]) -> Result<Self, Error> {
        // setup path
        let Some(path) = config.services.get(service) else {
            UNKNOWN_SERVICE.click();
            return Err(Error::UnknownService);
        };
        let path = match path {
            Ok(path) => path,
            Err(err) => {
                return Err(Error::ServiceError(err.clone()));
            }
        };
        let bound = config.rc_conf.bind_for_invoke(path)?;
        let path = CString::new(path.as_str())?;
        // setup wrapper
        let wrapper = config
            .rc_conf
            .wrapper(service, "WRAPPER")?
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
        for (key, value) in bound.iter() {
            env.push(CString::new(format!("{}={}", key, value))?);
        }
        for (key, value) in std::env::vars() {
            if matches!(key.as_str(), "PATH" | "TERM" | "TZ" | "LANG") {
                env.push(CString::new(format!("{}={}", key, value))?);
            }
        }
        env.sort();
        let started = Instant::now();
        Ok(Self {
            path,
            wrapper,
            argv,
            env,
            started,
        })
    }
}

impl From<&ExecutionContext> for indicio::Value {
    fn from(exec: &ExecutionContext) -> Self {
        fn c_string_to_string(s: &CString) -> String {
            s.to_string_lossy().into_owned()
        }
        fn to_value(strs: &[CString]) -> indicio::Value {
            strs.iter()
                .map(c_string_to_string)
                .collect::<Vec<_>>()
                .into()
        }
        value!({
            path: c_string_to_string(&exec.path),
            wrapper: to_value(&exec.wrapper),
            argv: to_value(&exec.argv),
            env: to_value(&exec.env),
        })
    }
}

///////////////////////////////////////////// Execution ////////////////////////////////////////////

#[derive(Debug)]
pub struct Execution {
    execution_id: ExecutionID,
    config: Arc<Pid1Configuration>,
    service: String,
    context: ExecutionContext,
    pid: Mutex<libc::pid_t>,
    pid_set: Condvar,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Execution {
    pub fn new(
        execution_id: ExecutionID,
        config: Arc<Pid1Configuration>,
        service: String,
        context: ExecutionContext,
    ) -> Self {
        let pid = Mutex::new(-1);
        let pid_set = Condvar::new();
        let thread = Mutex::new(None);
        Self {
            execution_id,
            config,
            service,
            context,
            pid,
            pid_set,
            thread,
        }
    }

    pub fn id(&self) -> ExecutionID {
        self.execution_id
    }

    pub fn config(&self) -> &Arc<Pid1Configuration> {
        &self.config
    }

    pub fn service(&self) -> &str {
        &self.service
    }

    pub fn context(&self) -> &ExecutionContext {
        &self.context
    }

    pub fn pid(&self) -> Option<i32> {
        let pid = self.pid.lock().unwrap();
        if *pid > 0 {
            Some(*pid)
        } else {
            None
        }
    }

    pub fn kill(&self, signal: minimal_signals::Signal) -> Result<(), Error> {
        EXECUTION_KILL.click();
        clue!(COLLECTOR, INFO, {
            kill: indicio::Value::from(&self.context),
        });
        if let Some(pid) = self.pid() {
            unsafe {
                if libc::kill(pid, signal.into_i32()) < 0
                    && *libc::__errno_location() != libc::ESRCH
                {
                    return Err(std::io::Error::last_os_error().into());
                }
            }
        }
        Ok(())
    }

    fn exec(self: &Arc<Self>) -> Result<(), Error> {
        EXECUTION_EXEC.click();
        clue!(COLLECTOR, INFO, {
            exec: indicio::Value::from(&self.context),
        });
        match self.exec_inner() {
            Ok(pid) => {
                self.set_pid(pid);
                Ok(())
            }
            Err(err) => {
                clue!(COLLECTOR, ERROR, {
                    exec: indicio::Value::from(&self.context),
                });
                self.set_pid(0);
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
        // spawn
        let mut pid: libc::pid_t = -1;
        unsafe {
            if libc::posix_spawnp(
                &mut pid,
                exe.as_ptr() as _,
                std::ptr::null(),
                std::ptr::null(),
                argv,
                envp,
            ) != 0
            {
                return Err(std::io::Error::last_os_error().into());
            }
        }
        Ok(pid)
    }

    fn block_until_have_pid(&self) -> libc::pid_t {
        let mut pid = self.pid.lock().unwrap();
        while *pid < 0 {
            pid = self.pid_set.wait(pid).unwrap();
        }
        *pid
    }

    fn set_pid(&self, pid: libc::pid_t) {
        *self.pid.lock().unwrap() = pid;
        self.pid_set.notify_all();
    }

    fn set_thread(&self, join: JoinHandle<()>) {
        *self.thread.lock().unwrap() = Some(join);
    }

    fn take_thread(&self) -> Option<JoinHandle<()>> {
        std::mem::take(&mut *self.thread.lock().unwrap())
    }
}

////////////////////////////////////////// BackoffTracker //////////////////////////////////////////

#[derive(Debug, Default)]
struct BackoffTracker {
    penalties: HashMap<String, (Instant, Duration)>,
}

impl BackoffTracker {
    fn track(&mut self, service: String, credit: Duration) {
        let (last_tracked, penalty) = self
            .penalties
            .entry(service.clone())
            .or_insert((Instant::now(), Duration::from_secs(1)));
        *last_tracked = Instant::now();
        fn compound(duration: Duration) -> Duration {
            Duration::from_micros(
                (duration.as_micros() as f64 * std::f64::consts::E.powf(0.05 * duration.as_secs_f64() / 60.)) as u64,
            )
        }
        let old_penalty = *penalty;
        *penalty = penalty.saturating_sub(compound(credit));
        *penalty = penalty.saturating_mul(2);
        *penalty = (*penalty).clamp(Duration::ZERO, Duration::from_secs(300));
        // TODO(rescrv): Don't log under lock.  Almost certainly under lock.
        clue!(COLLECTOR, INFO, {
            service: service,
            credit: format!("{:?}", credit),
            adjusted: format!("{:?}", compound(credit)),
            old_penalty: format!("{:?}", old_penalty),
            new_penalty: format!("{:?}", *penalty),
        });
    }

    fn backoff(&mut self, service: &str) -> Duration {
        let (last_tracked, penalty) = self
            .penalties
            .get(service)
            .cloned()
            .unwrap_or((Instant::now(), Duration::ZERO));
        let our_decision = penalty.clamp(Duration::from_secs(10), Duration::from_secs(60));
        let mut hasher = std::hash::DefaultHasher::new();
        last_tracked.hash(&mut hasher);
        let zero_to_one =
            (hasher.finish() & 0x1fffffffffffffu64) as f64 / (1u64 << f64::MANTISSA_DIGITS) as f64;
        Duration::from_micros((our_decision.as_micros() as f64 * (0.0 - zero_to_one.ln())) as u64)
            .clamp(Duration::ZERO, Duration::from_secs(300))
    }

    fn wipe_debts(&mut self) {
        let mut services = vec![];
        for (service, (last_tracked, penalty)) in self.penalties.iter() {
            if last_tracked.elapsed() >= *penalty {
                services.push(service.to_string());
            }
        }
        for service in services {
            self.penalties.remove(&service);
        }
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

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
    fn backoff_tracker() {
        let mut bt = BackoffTracker::default();
        println!(
            "FINDME {:?}",
            bt.track("foo".to_string(), Duration::from_secs(1))
        );
    }
}
