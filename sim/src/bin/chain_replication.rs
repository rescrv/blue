#[macro_use]
extern crate prototk_derive;

use prototk::{Unpacker,stack_pack};
use sim::{MILLIS,Environment,NetworkSwitch,Process,ProcessID,Simulator};
use id::{generate_id,generate_id_prototk};

//////////////////////////////////////////////// IDs ///////////////////////////////////////////////

generate_id!{GroupID, "group:"}
generate_id_prototk!{GroupID}
generate_id!{ReplicaID, "replica:"}
generate_id_prototk!{ReplicaID}
generate_id!{ConfigID, "config:"}
generate_id_prototk!{ConfigID}

/////////////////////////////////////////// Configuration //////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, Message, PartialEq)]
pub struct Configuration {
    #[prototk(1, message)]
    pub group_id: GroupID,
    #[prototk(2, message)]
    pub config_id: ConfigID,
    #[prototk(4, message)]
    pub chain: Vec<ReplicaID>,
}

impl Configuration {
    fn prev(&self, next: ReplicaID) -> Option<ReplicaID> {
        let mut prev = None;
        for p in self.chain.iter() {
            if *p == next {
                return prev;
            }
            prev = Some(*p);
        }
        None
    }

    fn next(&self, prev: ReplicaID) -> Option<ReplicaID> {
        let mut next = None;
        for n in self.chain.iter().rev() {
            if *n == prev {
                return next;
            }
            next = Some(*n);
        }
        None
    }
}

//////////////////////////////////////////// ChainState ////////////////////////////////////////////

enum ChainState {
    Initialized,
    ConfigStarted { initiator: ProcessID, nonce: u64, config: Configuration },
    ForwardStable { initiator: ProcessID, nonce: u64, config: Configuration },
    BackwardStable { initiator: ProcessID, nonce: u64, config: Configuration },
    Stable { config: Configuration },
}

/////////////////////////////////////////// ChainReplica ///////////////////////////////////////////

pub struct ChainReplica {
    id: ReplicaID,
    chain_state: ChainState,
}

impl ChainReplica {
    pub fn new() -> Self {
        Self {
            id: ReplicaID::generate().unwrap(),
            chain_state: ChainState::Initialized,
        }
    }

    fn ping(&mut self, env: &mut Environment, from: ProcessID, ping: PingPong) {
        println!("{} here to say: {} says hello", self.id, from);
        let msg = ChainMessage::Pong(ping);
        let pa = stack_pack(msg);
        env.send(from, pa.to_vec());
    }

    fn pong(&mut self, _env: &mut Environment, from: ProcessID) {
        println!("{} here to ask {} if they're feeling OK: server received pong", self.id, from);
    }

    fn coordinate(&mut self, env: &mut Environment, from: ProcessID, nonce: u64, config: Configuration) {
        let is_new_config = match &self.chain_state {
            ChainState::Initialized => { true },
            ChainState::ConfigStarted { initiator: _, nonce: _, config: c } => { c != &config },
            ChainState::BackwardStable { initiator: _, nonce: _, config: c } => { c != &config },
            ChainState::ForwardStable { initiator: _, nonce: _, config: c } => { c != &config },
            ChainState::Stable { config: c } => { c != &config },
        };
        if is_new_config {
            println!("{} here to say: {} installs {:?}", self.id, from, config);
            self.chain_state = ChainState::ConfigStarted { initiator: from, nonce, config };
            self.make_progress(env);
        }
    }

    fn forward_stabilize(&mut self, env: &mut Environment, from: ProcessID, nonce: u64, config: Configuration) {
    }

    fn backward_stabilize(&mut self, env: &mut Environment, from: ProcessID, nonce: u64, config: Configuration) {
    }

    fn make_progress(&mut self, env: &mut Environment) {
        match &self.chain_state {
            ChainState::Initialized => {
                self.make_progress_initialized(env);
            },
            ChainState::ConfigStarted { initiator, nonce, ref config } => {
                let initiator: ProcessID = initiator.clone();
                let nonce: u64 = nonce.clone();
                let config: Configuration = config.clone();
                self.make_progress_configured(env, initiator, nonce, config);
            },
            ChainState::BackwardStable { initiator, nonce, ref config } => {
                let initiator: ProcessID = initiator.clone();
                let nonce: u64 = nonce.clone();
                let config: Configuration = config.clone();
                self.make_progress_backward_stable(env, initiator, nonce, config);
            },
            ChainState::ForwardStable { initiator, nonce, ref config } => {
                let initiator: ProcessID = initiator.clone();
                let nonce: u64 = nonce.clone();
                let config: Configuration = config.clone();
                self.make_progress_forward_stable(env, initiator, nonce, config);
            },
            ChainState::Stable { config } => {
                let config: Configuration = config.clone();
                self.make_progress_stable(env, config);
            },
        }
    }

    fn make_progress_initialized(&mut self, env: &mut Environment) {
    }

    fn make_progress_configured(&mut self, env: &mut Environment, initiator: ProcessID, nonce: u64, config: Configuration) {
        println!("{} here to say: I'm making progress on a fresh configuration", self.id);
        let coordinate = self.coordinate_msg(nonce, config.clone());
        self.send_backward_stabilize(env, coordinate.clone());
        self.send_forward_stabilize(env, coordinate);
    }

    fn make_progress_backward_stable(&mut self, env: &mut Environment, initiator: ProcessID, nonce: u64, config: Configuration) {
        self.send_forward_stabilize(env, self.coordinate_msg(nonce, config.clone()));
    }

    fn make_progress_forward_stable(&mut self, env: &mut Environment, initiator: ProcessID, nonce: u64, config: Configuration) {
        self.send_backward_stabilize(env, self.coordinate_msg(nonce, config.clone()));
    }

    fn make_progress_stable(&mut self, env: &mut Environment, config: Configuration) {
    }

    fn coordinate_msg(&self, nonce: u64, config: Configuration) -> Coordinate {
        Coordinate {
            nonce,
            config,
        }
    }

    fn send_backward_stabilize(&mut self, env: &mut Environment, coordinate: Coordinate) {
        match coordinate.config.prev(self.id) {
            Some(prev) => {
                let msg = ChainMessage::BackwardStabilize(coordinate);
                let buf = stack_pack(msg).to_vec();
                env.send(ProcessID::new(prev.id), buf);
                println!("{} here to say: I'm making friends with the server behind me: {}", self.id, prev);
            },
            None => {
                match &self.chain_state {
                    ChainState::Initialized => {},
                    ChainState::ConfigStarted { initiator, nonce, ref config } => {
                        self.chain_state = ChainState::BackwardStable {
                            initiator: *initiator,
                            nonce: *nonce,
                            config: config.clone() };
                    },
                    ChainState::BackwardStable { initiator, nonce, ref config } => {},
                    ChainState::ForwardStable { initiator, nonce, ref config } => {
                        self.chain_state = ChainState::Stable {
                            config: config.clone()
                        };
                    },
                    ChainState::Stable { ref config } => {},
                };
            },
        }
    }

    fn send_forward_stabilize(&mut self, env: &mut Environment, coordinate: Coordinate) {
        match coordinate.config.next(self.id) {
            Some(next) => {
                let msg = ChainMessage::ForwardStabilize(coordinate);
                let buf = stack_pack(msg).to_vec();
                env.send(ProcessID::new(next.id), buf);
                println!("{} here to say: I'm making friends with the server in front of me: {}", self.id, next);
            },
            None => {
                match &self.chain_state {
                    ChainState::Initialized => {},
                    ChainState::ConfigStarted { initiator, nonce, config } => {
                        self.chain_state = ChainState::ForwardStable {
                            initiator: *initiator,
                            nonce: *nonce,
                            config: config.clone()
                        };
                    },
                    ChainState::BackwardStable { initiator: _, nonce: _, config } => {
                        self.chain_state = ChainState::Stable { config: config.clone() };
                    },
                    ChainState::ForwardStable { initiator: _, nonce: _, config: _ } => {},
                    ChainState::Stable { config: _ } => {},
                };
            },
        }
    }
}

impl Process for ChainReplica {
    fn pid(&self) -> ProcessID {
        ProcessID::new(self.id.id)
    }

    fn watch_dog(&mut self, env: &mut Environment) {
        env.set_watch_dog(100*MILLIS);
    }

    fn deliver(&mut self, env: &mut Environment, from: ProcessID, what: Vec<u8>) {
        let mut up = Unpacker::new(&what);
        let msg: ChainMessage = up.unpack().unwrap();
        match msg {
            ChainMessage::Ping(ping) => {
                self.ping(env, from, ping);
            },
            ChainMessage::Pong(_ping) => {
                self.pong(env, from);
            },
            ChainMessage::Coordinate(coordinate) => {
                self.coordinate(env, from, coordinate.nonce, coordinate.config);
            },
            ChainMessage::ForwardStabilize(coordinate) => {
                self.forward_stabilize(env, from, coordinate.nonce, coordinate.config);
            },
            ChainMessage::BackwardStabilize(coordinate) => {
                self.backward_stabilize(env, from, coordinate.nonce, coordinate.config);
            },
        }
    }
}

//////////////////////////////////////////// Coordinator ///////////////////////////////////////////

pub struct Coordinator {
    id: ProcessID,
    group_id: GroupID,
    replicas: Vec<ProcessID>,
}

impl Coordinator {
    pub fn new() -> Self {
        Self {
            id: ProcessID::generate().unwrap(),
            group_id: GroupID::generate().unwrap(),
            replicas: Vec::new(),
        }
    }

    pub fn monitor(&mut self, proc: ProcessID) {
        self.replicas.push(proc);
    }

    fn nop(&self, from: ReplicaID) {
        println!("{} here to say NOP from {}", self.id, from);
    }

    fn ack(&self, from: ReplicaID, nonce: u64, config_id: ConfigID) {
        println!("{} here to say {} adopted {}/{}", self.id, from, nonce, config_id);
    }

    fn coordinate_msg(&self, nonce: u64) -> Coordinate {
        let mut config = Configuration {
            group_id: self.group_id,
            config_id: ConfigID::generate().unwrap(),
            chain: Vec::new(),
        };
        for replica in self.replicas.iter() {
            config.chain.push(ReplicaID::new(replica.id))
        }
        Coordinate {
            nonce,
            config,
        }
    }
}

impl Process for Coordinator {
    fn pid(&self) -> ProcessID {
        ProcessID::new(self.id.id)
    }

    fn watch_dog(&mut self, env: &mut Environment) {
        env.set_watch_dog(100*MILLIS);
    }

    fn deliver(&mut self, env: &mut Environment, from: ProcessID, what: Vec<u8>) {
        let mut up = Unpacker::new(&what);
        let msg: CoordinatorMessage = up.unpack().unwrap();
        match msg {
            CoordinatorMessage::NOP(nop) => { self.nop(ReplicaID::new(from.id)); },
            CoordinatorMessage::ConfigAck(ack) => { self.ack(ReplicaID::new(from.id), ack.nonce, ack.config_id); },
        }
    }
}

/////////////////////////////////////////// ChainMessage ///////////////////////////////////////////

#[derive(Clone, Debug, Message)]
enum ChainMessage {
    #[prototk(1, message)]
    Ping(PingPong),
    #[prototk(2, message)]
    Pong(PingPong),
    #[prototk(3, message)]
    Coordinate(Coordinate),
    #[prototk(4, message)]
    ForwardStabilize(Coordinate),
    #[prototk(5, message)]
    BackwardStabilize(Coordinate),
}

impl Default for ChainMessage {
    fn default() -> Self {
        Self::Ping(PingPong { nonce: 0 })
    }
}

///////////////////////////////////////////// PingPong /////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
struct PingPong {
    #[prototk(1, fixed64)]
    nonce: u64,
}

//////////////////////////////////////////// Coordinate ////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
struct Coordinate {
    #[prototk(1, fixed64)]
    nonce: u64,
    #[prototk(2, message)]
    config: Configuration,
}

//////////////////////////////////////// CoordinatorMessage ////////////////////////////////////////

#[derive(Clone, Debug, Message)]
enum CoordinatorMessage {
    #[prototk(1, message)]
    NOP(NOP),
    #[prototk(2, message)]
    ConfigAck(ConfigAck),
}

impl Default for CoordinatorMessage {
    fn default() -> Self {
        Self::NOP(NOP::default())
    }
}

//////////////////////////////////////////////// NOP ///////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
struct NOP {
    #[prototk(1, fixed64)]
    nonce: u64,
}

///////////////////////////////////////////// ConfigAck ////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
struct ConfigAck {
    #[prototk(1, fixed64)]
    nonce: u64,
    #[prototk(2, message)]
    config_id: ConfigID,
}

/////////////////////////////////////////////// main ///////////////////////////////////////////////

fn main() {
    let process_a = ChainReplica::new();
    let process_b = ChainReplica::new();
    let process_c = ChainReplica::new();
    let mut coordinator = Coordinator::new();
    coordinator.monitor(process_a.pid());
    coordinator.monitor(process_b.pid());
    coordinator.monitor(process_c.pid());
    let mut switch = NetworkSwitch::new();
    switch.connect(process_a.pid());
    switch.connect(process_b.pid());
    switch.connect(process_c.pid());
    switch.connect(coordinator.pid());
    let mut sim = Simulator::new();
    sim.add_process(process_a);
    sim.add_process(process_b);
    sim.add_process(process_c);
    sim.add_process(coordinator);
    sim.add_switch(switch);
    sim.run();
}
