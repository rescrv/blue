use std::collections::HashMap;
use std::process::{Child, Command};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use rand::{Rng, RngCore};

use arrrg_derive::CommandLine;

use biometrics::{Collector, Counter, Gauge, Moments, Sensor, TDigest};

use buffertk::{stack_pack, Unpacker};

use prototk_derive::Message;

use guacamole::Guacamole;

use one_two_eight::{generate_id, generate_id_prototk};

use rivulet::{Listener, RecvChannel, RivuletCommandLine, SendChannel};

use texttale::{story, StoryElement, TextTale};

use util::fnmatch::Pattern;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static TICK: Counter = Counter::new("gremlins.tick");

pub fn register_biometrics(collector: &mut Collector) {
    collector.register_counter(&TICK);
}

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Debug)]
pub enum Error {
    IoError {
        what: String,
    },
    UsageError {
        what: String,
    },
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::IoError {
            what: err.to_string(),
        }
    }
}

///////////////////////////////////////////// ProcessID ////////////////////////////////////////////

generate_id!{ProcessID, "pid:"}
generate_id_prototk!{ProcessID}

/////////////////////////////////////////////// Event //////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
enum Event {
    #[prototk(1, message)]
    #[default]
    Nop,
    #[prototk(2, message)]
    Tick {
        #[prototk(1, message)]
        pid: ProcessID,
    },
    #[prototk(3, message)]
    Biometrics {
        #[prototk(1, string)]
        glob: String,
    },
}

/////////////////////////////////////////// DisplayAnswer //////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
enum DisplayAnswer {
    #[prototk(1, message)]
    #[default]
    Nop,
    #[prototk(2, message)]
    SingleString {
        #[prototk(1, message)]
        pid: ProcessID,
        #[prototk(2, string)]
        display: String,
    },
}

impl DisplayAnswer {
    fn pid(&self) -> ProcessID {
        match self {
            DisplayAnswer::Nop => {
                ProcessID::BOTTOM
            },
            DisplayAnswer::SingleString { pid, display: _ } => {
                *pid
            },
        }
    }
}

/////////////////////////////////////// ControlCenterOptions ///////////////////////////////////////

#[derive(Clone, CommandLine, Debug, Default, Eq, PartialEq)]
pub struct ControlCenterOptions {
    #[arrrg(nested)]
    listener: RivuletCommandLine,
}

////////////////////////////////////////////// Process /////////////////////////////////////////////

struct Process {
    pid: ProcessID,
    child: Child,
}

//////////////////////////////////////// ControlCenterState ////////////////////////////////////////

struct ControlCenterState {
    closed: bool,
    guac: Guacamole,
    processes: Vec<Process>,
    recv_channels: Vec<RecvChannel>,
    send_channels: Vec<SendChannel>,
}

impl ControlCenterState {
    fn listen(ptr: Arc<Mutex<ControlCenterState>>, listener: Listener) {
        for stream in listener {
            let (recv_chan, send_chan) = stream.unwrap();
            let mut state = ptr.lock().unwrap();
            if state.closed {
                return;
            }
            state.recv_channels.push(recv_chan);
            state.send_channels.push(send_chan);
        }
    }
}

/////////////////////////////////////////// ControlCenter //////////////////////////////////////////

pub struct ControlCenter<T: TextTale> {
    options: ControlCenterOptions,
    state: Arc<Mutex<ControlCenterState>>,
    tale: T,
    listener: JoinHandle<()>,
}

impl<T: TextTale> ControlCenter<T> {
    pub fn new(options: ControlCenterOptions, tale: T) -> Self {
        let listener = options.listener.bind_to().expect("bind-to");
        let state = Arc::new(Mutex::new(ControlCenterState {
            closed: false,
            guac: Guacamole::new(0),
            processes: Vec::new(),
            recv_channels: Vec::new(),
            send_channels: Vec::new(),
        }));
        let statep = Arc::clone(&state);
        let listener = std::thread::spawn(move || {
            ControlCenterState::listen(statep, listener);
        });
        Self {
            options,
            state,
            tale,
            listener,
        }
    }

    pub fn cleanup(self) {
        let mut processes = Vec::new();
        std::mem::swap(&mut processes, &mut self.state.lock().unwrap().processes);
        for mut proc in processes.into_iter() {
            proc.child.kill().unwrap_or(());
            proc.child.wait().unwrap();
        }
        self.state.lock().unwrap().closed = true;
        _ = self.options.listener.connect();
        self.listener.join().unwrap();
    }

    fn interpret_pid_random(guac: &mut Guacamole, pid: &str) -> Option<ProcessID> {
        if pid == "auto" {
            let mut bytes: [u8; one_two_eight::BYTES] = [0u8; one_two_eight::BYTES];
            guac.fill(&mut bytes);
            Some(ProcessID {
                id: bytes,
            })
        } else {
            ProcessID::from_human_readable(pid)
        }
    }

    fn interpret_pid_select(&self, pid: &str) -> Option<ProcessID> {
        if pid == "auto" {
            let mut state = self.state.lock().unwrap();
            if !state.processes.is_empty() {
                let idx = state.guac.next_u64() as usize % state.processes.len();
                Some(state.processes[idx].pid)
            } else {
                None
            }
        } else {
            ProcessID::from_human_readable(pid)
        }
    }

    pub fn kill(&mut self, pid: ProcessID) {
        let mut state = self.state.lock().unwrap();
        let mut idx = 0;
        let mut removed = false;
        while idx < state.processes.len() {
            if state.processes[idx].pid == pid {
                let mut proc = state.processes.remove(idx);
                proc.child.kill().unwrap_or(());
                proc.child.wait().unwrap();
                removed = true;
            } else {
                idx += 1;
            }
        }
        if !removed {
            writeln!(self.tale, "no such pid").unwrap();
        }
    }

    pub fn tick(&mut self, pid: ProcessID) {
        let mut state = self.state.lock().unwrap();
        for send_channel in state.send_channels.iter_mut() {
            let buf = stack_pack(Event::Tick {
                pid,
            }).to_buffer();
            send_channel.send(buf.as_bytes()).expect("could not send");
        }
    }
}

story! {
    self cmd,
    main_menu by ControlCenter<T>;
    "Welcome to the gremlins control center.

This is a tool for running distributed systems simulations that feel like text-mode adventures.

Choose from the following options:

help: ........ Print this help menu.
spawn: ....... Spawn a new process.
processes: ... List the open processes under this control center.
biometrics: .. Inspect the biometrics of a process.
kill: ........ Kill a specified process.
tick: ........ Deliver a tick to the process.
";
    "help" => {
        StoryElement::PrintHelp
    }
    "spawn" => {
        let mut spawner = Spawner {
            state: Arc::clone(&self.state),
            tale: &mut self.tale,
            pid: Default::default(),
            args: Default::default(),
            proc: Default::default(),
        };
        spawner.spawn();
        StoryElement::Continue
    }
    "processes" => {
        for proc in self.state.lock().unwrap().processes.iter() {
            writeln!(self.tale, "{}", proc.pid.human_readable()).expect("print pid");
        }
        StoryElement::Continue
    }
    "biometrics" => {
        if cmd.len() == 2 {
            let mut state = self.state.lock().unwrap();
            for send_channel in state.send_channels.iter_mut() {
                let buf = stack_pack(Event::Biometrics {
                    glob: cmd[1].to_owned(),
                }).to_buffer();
                send_channel.send(buf.as_bytes()).expect("could not send");
            }
            let mut answers = Vec::new();
            for recv_channel in state.recv_channels.iter_mut() {
                let buf = recv_channel.recv().expect("could not recv");
                let mut up = Unpacker::new(buf.as_bytes());
                let ans: DisplayAnswer = up.unpack().unwrap();
                answers.push(ans);
            }
            answers.sort_by_key(|x| x.pid());
            for answer in answers {
                if let DisplayAnswer::SingleString { pid, display } = &answer {
                    writeln!(self.tale, "{}\n{}\n", pid, display).unwrap();
                }
            }
        } else {
            writeln!(self.tale, "kill takes exactly one argument, the pid to kill: {:?}", cmd).unwrap();
        }
        StoryElement::Continue
    }
    "kill" => {
        if cmd.len() == 2 {
            let pid = self.interpret_pid_select(cmd[1]);
            if let Some(pid) = pid {
                self.kill(pid);
            } else {
                writeln!(self.tale, "pid must be a valid ProcessID or \"auto\".").unwrap();
            };
        } else {
            writeln!(self.tale, "kill takes exactly one argument, the pid to kill: {:?}", cmd).unwrap();
        }
        StoryElement::Continue
    }
    "tick" => {
        if cmd.len() == 2 {
            let pid = self.interpret_pid_select(cmd[1]);
            if let Some(pid) = pid {
                self.tick(pid);
                std::thread::sleep(std::time::Duration::from_millis(100));
            } else {
                writeln!(self.tale, "pid must be a valid ProcessID or \"auto\".").unwrap();
            };
        } else {
            writeln!(self.tale, "fire takes one argument, the pid.").unwrap();
        }
        StoryElement::Continue
    }
}

////////////////////////////////////////////// Spawner /////////////////////////////////////////////

pub struct Spawner<'a, T: TextTale> {
    state: Arc<Mutex<ControlCenterState>>,
    tale: &'a mut T,
    pid: ProcessID,
    args: Vec<String>,
    proc: String,
}

story! {
    self cmd,
    spawn by Spawner<'_, T>;
"Spawn a gremlin.

This will create a new process in which services register.

Choose from the following options:

pid: ... Set the ProcessID for the process.
proc: .. Set the command for the process.
args: .. Add additional args to the process.
         This will augment existing args.
go: .... Spawn the process.
";
    "help" => {
        StoryElement::PrintHelp
    }
    "pid" => {
        if cmd.len() == 2 {
            if let Some(pid) = ControlCenter::<T>::interpret_pid_random(&mut self.state.lock().unwrap().guac, cmd[1]) {
                self.pid = pid;
            } else {
                writeln!(self.tale, "pid must be a valid ProcessID or \"auto\".").unwrap();
            };
        } else {
            writeln!(self.tale, "pid takes exactly one argument, the pid.").unwrap();
        }
        StoryElement::Continue
    }
    "proc" => {
        if cmd.len() == 2 {
            self.proc = cmd[1].to_owned();
        } else {
            writeln!(self.tale, "proc takes exactly one argument, the command.").unwrap();
        }
        StoryElement::Continue
    }
    "args" => {
        for arg in &cmd[1..] {
            self.args.push(arg.to_string());
        }
        StoryElement::Continue
    }
    "go" => {
        let mut args = vec!["--harness-process-id".to_owned(), self.pid.human_readable()];
        args.append(&mut self.args.clone());
        match Command::new(self.proc.clone()).args(args).spawn() {
            Ok(child) => {
                let process = Process {
                    pid: self.pid,
                    child,
                };
                self.state.lock().unwrap().processes.push(process);
                std::thread::sleep(std::time::Duration::from_millis(100));
                StoryElement::Return
            }
            Err(err) => {
                writeln!(self.tale, "could not spawn process: {}", err).unwrap();
                StoryElement::Return
            }
        }
    }
}

////////////////////////////////////////// HarnessOptions //////////////////////////////////////////

#[derive(CommandLine, Eq, PartialEq)]
pub struct HarnessOptions {
    #[arrrg(required, "ProcessID of the process in human-readable pid:0ced594f-b619-4be6-1c8d-8ff1d5963525 form", "PID")]
    process_id: ProcessID,
    #[arrrg(nested)]
    control: RivuletCommandLine,
}

impl Default for HarnessOptions {
    fn default() -> Self {
        Self {
            process_id: ProcessID::BOTTOM,
            control: RivuletCommandLine::default(),
        }
    }
}

/////////////////////////////////////////// MemoryEmitter //////////////////////////////////////////

#[derive(Default)]
struct LastValueEmitter {
    counters: HashMap<String, <Counter as Sensor>::Reading>,
    gauges: HashMap<String, <Gauge as Sensor>::Reading>,
    moments: HashMap<String, <Moments as Sensor>::Reading>,
    t_digests: HashMap<String, <TDigest as Sensor>::Reading>,
}

impl LastValueEmitter {
    fn fnmatch(&self, fnmatch: Pattern) -> Vec<String> {
        let mut strings = Vec::new();
        for (label, reading) in self.counters.iter() {
            if fnmatch.fnmatch(label) {
                strings.push(format!("{} = {}", label, reading));
            }
        }
        for (label, reading) in self.gauges.iter() {
            if fnmatch.fnmatch(label) {
                strings.push(format!("{} = {}", label, reading));
            }
        }
        for (label, reading) in self.moments.iter() {
            if fnmatch.fnmatch(label) {
                strings.push(format!("{} = {{ n: {}, mean: {}, variance: {}, skewness: {}, kurtosis: {} }}",
                                     label, reading.n(), reading.mean(), reading.variance(),
                                     reading.skewness(), reading.kurtosis()));
            }
        }
        for (label, _) in self.t_digests.iter() {
            if fnmatch.fnmatch(label) {
                strings.push(format!("{} = <t-digest is unsupported>", label));
            }
        }
        strings.sort();
        strings
    }
}

impl biometrics::Emitter for LastValueEmitter {
    type Error = ();

    fn emit_counter(&mut self, counter: &'static Counter, _: f64) -> Result<(), ()> {
        self.counters.insert(counter.label().to_owned(), counter.read());
        Ok(())
    }

    fn emit_gauge(&mut self, gauge: &'static Gauge, _: f64) -> Result<(), ()> {
        self.gauges.insert(gauge.label().to_owned(), gauge.read());
        Ok(())
    }

    fn emit_moments(&mut self, moments: &'static Moments, _: f64) -> Result<(), ()> {
        self.moments.insert(moments.label().to_owned(), moments.read());
        Ok(())
    }

    fn emit_t_digest(&mut self, t_digest: &'static TDigest, _: f64) -> Result<(), ()> {
        self.t_digests.insert(t_digest.label().to_owned(), t_digest.read());
        Ok(())
    }
}

////////////////////////////////////////////// Harness /////////////////////////////////////////////

pub struct Harness {
    options: HarnessOptions,
    biometrics: Collector,
    control_recv: Mutex<RecvChannel>,
    control_send: Mutex<SendChannel>,
}

impl Harness {
    pub fn new(options: HarnessOptions) -> Result<Arc<Self>, rpc_pb::Error> {
        let (control_recv, control_send) = options.control.connect()?;
        let mut biometrics = Collector::new();
        register_biometrics(&mut biometrics);
        indicio::register_biometrics(&mut biometrics);
        busybee::register_biometrics(&mut biometrics);
        rivulet::register_biometrics(&mut biometrics);
        let control_recv = Mutex::new(control_recv);
        let control_send = Mutex::new(control_send);
        Ok(Arc::new(Self {
            options,
            biometrics,
            control_recv,
            control_send,
        }))
    }

    pub fn serve(self: Arc<Self>) -> ! {
        // Only one server at a time.  We'll hold the mutex on the recv channel to enforce that.
        let mut control_recv = self.control_recv.lock().unwrap();
        loop {
            let msg_buf = control_recv.recv().unwrap();
            let mut up = Unpacker::new(msg_buf.as_bytes());
            let event: Event = up.unpack().unwrap();
            match event {
                Event::Nop => {},
                Event::Tick { pid } => {
                    if pid == self.options.process_id {
                        TICK.click();
                    }
                },
                Event::Biometrics { glob } => {
                    let pattern = Pattern::must(glob);
                    let mut emitter = LastValueEmitter::default();
                    self.biometrics.emit(&mut emitter).unwrap();
                    let sensors = emitter.fnmatch(pattern);
                    let display = sensors.join("\n");
                    let buf = stack_pack(DisplayAnswer::SingleString {
                        pid: self.options.process_id,
                        display,
                    }).to_buffer();
                    self.control_send.lock().unwrap().send(buf.as_bytes()).unwrap();
                },
            }
        }
    }
}
