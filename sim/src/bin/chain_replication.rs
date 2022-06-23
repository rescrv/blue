use util::generate_id;
use sim::{MILLIS,Environment,NetworkSwitch,Process,ProcessID,Simulator};

//////////////////////////////////////////////// IDs ///////////////////////////////////////////////

generate_id!{ReplicaID, "replica:"}
generate_id!{ConfigID, "config:"}

/////////////////////////////////////////// Configuration //////////////////////////////////////////

pub struct Configuration {
    pub chain: Vec<ReplicaID>,
}

/////////////////////////////////////////// ChainReplica ///////////////////////////////////////////

pub struct ChainReplica {
    id: ReplicaID,
}

impl ChainReplica {
    pub fn new() -> Self {
        Self {
            id: ReplicaID::generate().unwrap(),
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
}

/////////////////////////////////////////////// main ///////////////////////////////////////////////

fn main() {
    let process_a = ChainReplica::new();
    let process_b = ChainReplica::new();
    let process_c = ChainReplica::new();
    let mut switch = NetworkSwitch::new();
    switch.connect(process_a.pid());
    switch.connect(process_b.pid());
    switch.connect(process_c.pid());
    let mut sim = Simulator::new();
    sim.add_process(process_a);
    sim.add_process(process_b);
    sim.add_process(process_c);
    sim.add_switch(switch);
    sim.run();
}
