use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

use one_two_eight::generate_id;
use rc_conf::{load_services, RcConf, SwitchPosition};
use utf8path::Path;

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
        Self::Io(err)
    }
}

impl From<shvar::Error> for Error {
    fn from(err: shvar::Error) -> Self {
        Self::Shvar(err)
    }
}

impl From<rc_conf::Error> for Error {
    fn from(err: rc_conf::Error) -> Self {
        Self::RcConf(err)
    }
}

impl From<std::ffi::NulError> for Error {
    fn from(_: std::ffi::NulError) -> Self {
        Self::NulError
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
}

impl Pid1State {
    fn new(config: Arc<Pid1Configuration>) -> Self {
        let shutdown = false;
        let converge = 1;
        let processes = vec![];
        let inhibited = HashSet::new();
        Self {
            shutdown,
            converge,
            config,
            processes,
            inhibited,
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
            // TODO(rescrv): biometrics;
            return SwitchPosition::No;
        }
        self.config.rc_conf.service_switch(service)
    }

    fn set_inhibit(&mut self, service: &str) {
        self.inhibited.insert(service.to_string());
    }

    fn clear_inhibit(&mut self, service: &str) {
        self.inhibited.remove(service);
    }

    fn is_inhibited(&self, service: &str) -> bool {
        self.inhibited.contains(service)
    }

    fn spawn(
        &mut self,
        reclaim: SyncSender<Arc<Execution>>,
        service: &str,
        argv: &[&str],
    ) -> Result<ExecutionID, Error> {
        let execution_id = ExecutionID::generate().ok_or(Error::GeneratingExecutionID)?;
        let config = Arc::clone(&self.config);
        let service = service.to_string();
        let context = ExecutionContext::new(&config, &service, argv)?;
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
            unsafe {
                if libc::waitpid(pid, &mut status, 0) < 0 {
                    // TODO(rescrv): log that this failed.
                    // TODO(rescrv): backoff and retry in a loop.
                }
            }
        }
        reclaim.send(exec).unwrap();
    }
}

///////////////////////////////////////////// Pid1State ////////////////////////////////////////////

#[derive(Debug, Default)]
struct Pid1Coordination {
    converge: Condvar,
}

/////////////////////////////////////////////// Pid1 ///////////////////////////////////////////////

#[derive(Debug)]
pub struct Pid1 {
    options: Pid1Options,
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
            if let Some(join) = exec.take_thread() {
                let _ = join.join();
            }
            let mut state = state.lock().unwrap();
            state.processes.retain(|p| !Arc::ptr_eq(p, &exec));
            state.converge = state.converge.saturating_add(1);
            coord.converge.notify_all();
            if state.converge == u64::MAX {
                todo!();
            }
        }
    }

    fn converge_thread(
        reclaim: SyncSender<Arc<Execution>>,
        state: Arc<Mutex<Pid1State>>,
        coord: Arc<Pid1Coordination>,
    ) {
        let mut converge = 0;
        loop {
            let c = {
                let mut state = state.lock().unwrap();
                while !state.shutdown && converge >= state.converge {
                    state = coord.converge.wait(state).unwrap();
                }
                if state.shutdown {
                    break;
                }
                state.converge
            };
            if Self::converge(&reclaim, &state).is_ok() {
                converge = c;
            } else {
                // TODO(rescrv): backoff and retry
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
    }

    fn converge(
        reclaim: &SyncSender<Arc<Execution>>,
        state: &Mutex<Pid1State>,
    ) -> Result<(), Error> {
        let (processes, config) = {
            let state = state.lock().unwrap();
            (state.processes.clone(), Arc::clone(&state.config))
        };
        fn has_process(state: &Mutex<Pid1State>, exec: &Arc<Execution>) -> bool {
            let state = state.lock().unwrap();
            state.has_process(exec)
        }
        for exec in processes {
            let current_context = ExecutionContext::new(&config, &exec.service, &[])?;
            if current_context != exec.context {
                if exec.pid().is_none() {
                    todo!("log this");
                }
                for iter in 1..=3 {
                    for _ in 0..(1 << iter) * 10 {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        if !has_process(state, &exec) {
                            break;
                        }
                    }
                    exec.kill(minimal_signals::SIGTERM)?;
                }
                while !has_process(state, &exec) {
                    exec.kill(minimal_signals::SIGKILL)?;
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            }
        }
        for service in config.services.keys() {
            let mut state = state.lock().unwrap();
            if state.is_inhibited(service) {
                continue;
            }
            if state.service_switch(service) == SwitchPosition::Yes && !state.is_running(service) {
                state.spawn(reclaim.clone(), service, &[])?;
            }
        }
        Ok(())
    }

    pub fn shutdown(self) -> Result<(), Error> {
        {
            let mut state = self.state.lock().unwrap();
            state.shutdown = true;
        }
        for iter in 1..=3 {
            for _ in 0..(1 << iter) * 10 {
                std::thread::sleep(std::time::Duration::from_millis(100));
                if !self.has_processes() {
                    break;
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

    pub fn reload(&self) -> Result<(), Error> {
        let config = Arc::new(Pid1Configuration::from_options(&self.options)?);
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
        let mut err = Ok(());
        let state = self.state.lock().unwrap();
        for process in state.processes.iter() {
            if target.matches(process) {
                let pid: libc::pid_t = *process.pid.lock().unwrap();
                if pid > 0 {
                    unsafe {
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
        Ok(())
    }

    pub fn stop(&self, service: &str) -> Result<(), Error> {
        let mut processes: Vec<Arc<Execution>> = {
            let mut state = self.state.lock().unwrap();
            state.set_inhibit(service);
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

#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub struct ExecutionContext {
    pub path: CString,
    pub wrapper: Vec<CString>,
    pub argv: Vec<CString>,
    pub env: Vec<CString>,
}

impl ExecutionContext {
    pub fn new(config: &Pid1Configuration, service: &str, argv: &[&str]) -> Result<Self, Error> {
        // setup path
        let Some(path) = config.services.get(service) else {
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
        Ok(Self {
            path,
            wrapper,
            argv,
            env,
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
        match self.exec_inner() {
            Ok(pid) => {
                self.set_pid(pid);
                Ok(())
            }
            Err(err) => {
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
            if libc::posix_spawn(
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
}