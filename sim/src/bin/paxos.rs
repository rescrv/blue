#[macro_use]
extern crate prototk_derive;

use buffertk::{Unpacker, stack_pack};
use id::{generate_id, generate_id_prototk};

use sim::*;

//////////////////////////////////////////////// IDs ///////////////////////////////////////////////

generate_id!{GroupID, "group:"}
generate_id_prototk!{GroupID}

generate_id!{ReplicaID, "replica:"}
generate_id_prototk!{ReplicaID}

////////////////////////////////////////////// Ballot //////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
struct Ballot {
    #[prototk(1, uint64)]
    seniority: u64,
    #[prototk(2, message)]
    leader: ReplicaID,
}

///////////////////////////////////////////// ReplicaID ////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
struct Replica {
    group: GroupID,
    id: ReplicaID,
}

impl Replica {
    fn make_progress(&mut self, env: &mut Environment) {
    }
}

impl Process for Replica {
    fn pid(&self) -> ProcessID {
        ProcessID::new(self.id.id)
    }

    fn watch_dog(&mut self, env: &mut Environment) {
        env.set_watch_dog(1000);
        self.make_progress(env);
    }

    fn deliver(&mut self, env: &mut Environment, from: ProcessID, what: Vec<u8>) {
        let mut up = Unpacker::new(&what);
        let msg: ReplicaMessage = match up.unpack() {
            Ok(x) => { x },
            Err(e) => {
                // TODO(rescrv): log error
                return;
            }
        };
        // XXX
    }
}

////////////////////////////////////////// ReplicaMessage //////////////////////////////////////////

#[derive(Clone, Debug, Message)]
enum ReplicaMessage {
    #[prototk(1, message)]
    Phase1(Phase1A),
    #[prototk(2, message)]
    Phase2(Phase2A),
}

impl Default for ReplicaMessage {
    fn default() -> Self {
        Self::Phase1(Phase1A::default())
    }
}

////////////////////////////////////////////// Phase1A /////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
struct Phase1A {
    #[prototk(1, message)]
    group: GroupID,
    #[prototk(2, message)]
    ballot: Ballot,
}

////////////////////////////////////////////// Phase2A /////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
struct Phase2A {
}

////////////////////////////////////////// ProposerMessage /////////////////////////////////////////

enum ProposerMessage {
    Phase1P(Phase1P),
    Phase2P(Phase2P),
}

////////////////////////////////////////////// Phase1P /////////////////////////////////////////////

struct Phase1P {
}

////////////////////////////////////////////// Phase2P /////////////////////////////////////////////

struct Phase2P {
}

/////////////////////////////////////////////// clues //////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
struct ClueReplicaDeliverUnpackError<'a> {
    #[prototk(1, message)]
    from: ProcessID,
    #[prototk(2, bytes)]
    what: &'a [u8],
}

/////////////////////////////////////////////// main ///////////////////////////////////////////////

fn main() {
    let mut sim = Simulator::new();
    let replica1 = Replica::default();
    sim.add_process(replica1);
    let replica2 = Replica::default();
    sim.add_process(replica2);
    let replica3 = Replica::default();
    sim.add_process(replica3);
    let switch = NetworkSwitch::default();
    sim.run();
}
