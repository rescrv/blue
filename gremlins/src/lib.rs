use std::process::{Child, Command};
use std::sync::{Arc, Mutex};

use rand::Rng;

use arrrg_derive::CommandLine;

use guacamole::Guacamole;

use one_two_eight::{generate_id, generate_id_prototk};

use rpc_pb::{service, Context};

use rivulet::{RecvChannel, RivuletCommandLine, SendChannel};

use texttale::TextTale;

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
    guac: Guacamole,
    processes: Vec<Process>,
}

/////////////////////////////////////////// ControlCenter //////////////////////////////////////////

pub struct ControlCenter {
    options: ControlCenterOptions,
    state: Arc<Mutex<ControlCenterState>>,
}

impl ControlCenter {
    pub fn new(options: ControlCenterOptions) -> Self {
        let state = Arc::new(Mutex::new(ControlCenterState { 
            guac: Guacamole::new(0),
            processes: Vec::new(),
        }));
        Self {
            options,
            state, 
        }
    }
}

const CONTROL_CENTER_HELP: &str = "Welcome to the gremlins control center.

This is a tool for running distributed systems simulations that feel like text-mode adventures.

Choose from the following options:

help: ....... Print this help menu.
spawn: ...... Spawn a new process.
processes: .. List the open processes under this control center.
kill: ....... Kill a specified process.
";

impl ControlCenter {
    pub fn main_menu<T: TextTale>(&mut self, tale: &mut T) -> Result<(), Error> {
        let mut print_help = true;
        'adventuring:
        loop {
            if print_help {
                writeln!(tale, "{}", CONTROL_CENTER_HELP)?;
                print_help = false;
            }
            if let Some(ref line) = tale.next_command() {
                let cmd: Vec<String> = line.split_whitespace().map(|s| s.to_owned()).collect();
                if cmd.is_empty() {
                    continue 'adventuring;
                }
                match cmd[0].as_str() {
                    "help" => {
                        print_help = true;
                    },
                    "spawn" => {
                        self.spawn(tale)?;
                    },
                    "processes" => {
                        for proc in self.state.lock().unwrap().processes.iter() {
                            writeln!(tale, "{}", proc.pid.human_readable())?;
                        }
                    },
                    "kill" => {
                        if cmd.len() == 2 {
                            let pid = if cmd[1] == "auto" {
                                let mut bytes: [u8; one_two_eight::BYTES] = [0u8; one_two_eight::BYTES];
                                self.state.lock().unwrap().guac.fill(&mut bytes);
                                ProcessID {
                                    id: bytes,
                                }
                            } else if let Some(pid) = ProcessID::from_human_readable(&cmd[1]) {
                                pid
                            } else {
                                writeln!(tale, "pid must be a valid ProcessID or \"auto\".")?;
                                continue 'adventuring;
                            };
                            self.kill(tale, pid)?;
                        } else {
                            writeln!(tale, "kill takes exactly one argument, the pid to kill.")?;
                            continue 'adventuring;
                        }
                    },
                    _ => {
                        writeln!(tale, "unknown command: {}", line.as_str())?;
                    },
                }
            } else {
                break 'adventuring;
            }
        }
        let mut processes = Vec::new();
        std::mem::swap(&mut processes, &mut self.state.lock().unwrap().processes);
        for mut proc in processes.into_iter() {
            proc.child.kill().unwrap_or(());
            proc.child.wait().unwrap();
        }
        Ok(())
    }

    pub fn kill<T: TextTale>(&mut self, tale: &mut T, pid: ProcessID) -> Result<(), Error> {
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
            writeln!(tale, "no such pid")?;
        }
        Ok(())
    }
}

const SPAWN_HELP: &str = "Spawn a gremlin.

This will create a new process in which services register.

Choose from the following options:

pid: ... Set the ProcessID for the process.
proc: .. Set the command for the process.
args: .. Add additional args to the process.
         This will augment existing args.
go: .... Spawn the process.
";

impl ControlCenter {
    pub fn spawn<T: TextTale>(&mut self, tale: &mut T) -> Result<(), Error> {
        let mut pid = ProcessID::default();
        let mut args = Vec::new();
        let mut proc = String::new();
        let mut print_help = true;
        'adventuring:
        loop {
            if print_help {
                writeln!(tale, "{}", SPAWN_HELP)?;
                print_help = false;
            }
            if let Some(ref line) = tale.next_command() {
                let cmd: Vec<String> = line.split_whitespace().map(|s| s.to_owned()).collect();
                if cmd.is_empty() {
                    continue 'adventuring;
                }
                match cmd[0].as_str() {
                    "pid" => {
                        if cmd.len() == 2 {
                            pid = if cmd[1] == "auto" {
                                let mut bytes: [u8; one_two_eight::BYTES] = [0u8; one_two_eight::BYTES];
                                self.state.lock().unwrap().guac.fill(&mut bytes);
                                ProcessID {
                                    id: bytes,
                                }
                            } else if let Some(pid) = ProcessID::from_human_readable(&cmd[1]) {
                                pid
                            } else {
                                writeln!(tale, "pid must be a valid ProcessID or \"auto\".")?;
                                continue 'adventuring;
                            };
                        } else {
                            writeln!(tale, "pid takes exactly one argument, the ProcessID.")?;
                            continue 'adventuring;
                        }
                    },
                    "proc" => {
                        if cmd.len() == 2 {
                            proc = cmd[1].clone();
                        } else {
                            writeln!(tale, "proc takes exactly one argument, the command.")?;
                            continue 'adventuring;
                        }
                    },
                    "args" => {
                        args.extend_from_slice(&cmd[1..]);
                    },
                    "go" => {
                        let child = match Command::new(proc.clone()).args(args.clone()).spawn() {
                            Ok(child) => child,
                            Err(err) => {
                                writeln!(tale, "could not spawn process: {}", err)?;
                                continue 'adventuring;
                            }
                        };
                        let process = Process {
                            pid,
                            child,
                        };
                        self.state.lock().unwrap().processes.push(process);
                        return Ok(());
                    },
                    _ => {
                        writeln!(tale, "unknown command: {}", line.as_str())?;
                        continue 'adventuring;
                    },
                }
            } else {
                break 'adventuring;
            }
        }
        Ok(())
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
