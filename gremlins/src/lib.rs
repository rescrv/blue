use std::process::{Child, Command};
use std::sync::{Arc, Mutex};

use rand::Rng;

use arrrg_derive::CommandLine;

use guacamole::Guacamole;

use one_two_eight::{generate_id, generate_id_prototk};

use rivulet::{Listener, RecvChannel, RivuletCommandLine, SendChannel};

use texttale::{story, StoryElement, TextTale};

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

/////////////////////////////////////// ControlCenterOptions ///////////////////////////////////////

#[derive(Clone, CommandLine, Debug, Eq, PartialEq)]
pub struct ControlCenterOptions {
    #[arrrg(nested)]
    listener: RivuletCommandLine,
}

impl Default for ControlCenterOptions {
    fn default() -> Self {
        Self {
            listener: RivuletCommandLine::default(),
        }
    }
}

////////////////////////////////////////////// Process /////////////////////////////////////////////

struct Process {
    pid: ProcessID,
    child: Child,
}

//////////////////////////////////////// ControlCenterState ////////////////////////////////////////

struct ControlCenterState {
    listener: Listener,
    guac: Guacamole,
    processes: Vec<Process>,
}

/////////////////////////////////////////// ControlCenter //////////////////////////////////////////

pub struct ControlCenter<T: TextTale> {
    options: ControlCenterOptions,
    state: Arc<Mutex<ControlCenterState>>,
    tale: T,
}

impl<T: TextTale> ControlCenter<T> {
    pub fn new(options: ControlCenterOptions, tale: T) -> Self {
        let listener = options.listener.bind_to().expect("bind-to");
        let state = Arc::new(Mutex::new(ControlCenterState { 
            listener,
            guac: Guacamole::new(0),
            processes: Vec::new(),
        }));
        Self {
            options,
            state, 
            tale,
        }
    }

    pub fn cleanup(&mut self) {
        let mut processes = Vec::new();
        std::mem::swap(&mut processes, &mut self.state.lock().unwrap().processes);
        for mut proc in processes.into_iter() {
            proc.child.kill().unwrap_or(());
            proc.child.wait().unwrap();
        }
    }

    fn interpret_pid(guac: &mut Guacamole, pid: &str) -> Option<ProcessID> {
        if pid == "auto" {
            let mut bytes: [u8; one_two_eight::BYTES] = [0u8; one_two_eight::BYTES];
            guac.fill(&mut bytes);
            Some(ProcessID {
                id: bytes,
            })
        } else if let Some(pid) = ProcessID::from_human_readable(pid) {
            Some(pid)
        } else {
            None
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
}

story! {
    self cmd,
    main_menu by ControlCenter<T>;
    "Welcome to the gremlins control center.

This is a tool for running distributed systems simulations that feel like text-mode adventures.

Choose from the following options:

help: ....... Print this help menu.
spawn: ...... Spawn a new process.
processes: .. List the open processes under this control center.
kill: ....... Kill a specified process.
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
    "kill" => {
        if cmd.len() == 2 {
            let pid = Self::interpret_pid(&mut self.state.lock().unwrap().guac, &cmd[1]);
            if let Some(pid) = pid {
                self.kill(pid);
            } else {
                writeln!(self.tale, "pid must be a valid ProcessID or \"auto\".").unwrap();
            };
        } else {
            writeln!(self.tale, "kill takes exactly one argument, the pid to kill.").unwrap();
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
            if let Some(pid) = ControlCenter::<T>::interpret_pid(&mut self.state.lock().unwrap().guac, &cmd[1]) {
                self.pid = pid;
            } else {
                writeln!(self.tale, "pid must be a valid ProcessID or \"auto\".").unwrap();
            };
        } else {
            writeln!(self.tale, "kill takes exactly one argument, the pid to kill.").unwrap();
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
        match Command::new(self.proc.clone()).args(self.args.clone()).spawn() {
            Ok(child) => {
                let process = Process {
                    pid: self.pid,
                    child,
                };
                self.state.lock().unwrap().processes.push(process);
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
    #[arrrg(nested)]
    control: RivuletCommandLine,
}

impl Default for HarnessOptions {
    fn default() -> Self {
        Self {
            control: RivuletCommandLine::default(),
        }
    }
}

////////////////////////////////////////////// Harness /////////////////////////////////////////////

pub struct Harness {
    options: HarnessOptions,
    control_recv: Mutex<RecvChannel>,
    control_send: Mutex<SendChannel>,
}

impl Harness {
    pub fn new(options: HarnessOptions) -> Result<Arc<Self>, rpc_pb::Error> {
        let (control_recv, control_send) = options.control.connect()?;
        let control_recv = Mutex::new(control_recv);
        let control_send = Mutex::new(control_send);
        Ok(Arc::new(Self {
            options,
            control_recv,
            control_send,
        }))
    }

    pub fn serve(self: Arc<Self>) -> ! {
        // Only one server at a time.  We'll hold the mutex on the recv channel to enforce that.
        let mut control_recv = self.control_recv.lock().unwrap();
        loop {
            let msg_buf = control_recv.recv();
            println!("{:?}", msg_buf);
        }
    }
}
