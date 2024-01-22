use prototk_derive::Message;

use one_two_eight::{generate_id, generate_id_prototk};

use rpc_pb::service;

use zerror_core::ErrorCore;

///////////////////////////////////////////// Constants ////////////////////////////////////////////

pub const DEFAULT_ALPHA: u64 = 256;
pub const MIN_ALPHA: u64 = 1;
pub const MAX_ALPHA: u64 = 1 << 20;

//////////////////////////////////////////////// IDs ///////////////////////////////////////////////

generate_id!(PaxosID, "paxos:");
generate_id_prototk!(PaxosID);

generate_id!(ReplicaID, "replica:");
generate_id_prototk!(ReplicaID);

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Message, zerror_derive::Z)]
pub enum Error {
    #[prototk(573440, message)]
    Success {
        #[prototk(1, message)]
        core: ErrorCore,
    },
    #[prototk(573441, message)]
    SerializationError {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, message)]
        what: prototk::Error,
    },
    #[prototk(573441, message)]
    RpcError {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, message)]
        what: rpc_pb::Error,
    },
    #[prototk(573441, message)]
    IoError {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        what: String,
    },
}

impl Default for Error {
    fn default() -> Self {
        Self::Success {
            core: ErrorCore::default(),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(what: std::io::Error) -> Self {
        Self::IoError {
            core: ErrorCore::default(),
            what: what.to_string(),
        }
    }
}

impl From<prototk::Error> for Error {
    fn from(what: prototk::Error) -> Self {
        Self::SerializationError {
            core: ErrorCore::default(),
            what,
        }
    }
}

impl From<rpc_pb::Error> for Error {
    fn from(what: rpc_pb::Error) -> Self {
        Self::RpcError {
            core: ErrorCore::default(),
            what,
        }
    }
}

////////////////////////////////////////////// Ballot //////////////////////////////////////////////

/// Ballots are neither created nor destroyed, they just exist.  The overall protocol must
/// guarantee that no two replicas ever work the synod protocol using the same ballot.  To
/// accomplish this, a ballot is the ordered pair of (number, leader), where only the listed leader
/// is allowed to issue proposals under the ballot.
///
/// Ballots are comparable.  When `ballot1` < `ballot2`, we will say that ballot2 supersedes
/// ballot1.  The comparison is lexicographic by (number, leader), which ensures that a proposer
/// whose ballot is superseded by a competing proposer can select another ballot to supersede
/// either of the first two.
#[derive(Clone, Debug, Eq, Hash, Message, PartialEq, PartialOrd, Ord)]
pub struct Ballot {
    #[prototk(1, uint64)]
    pub number: u64,
    #[prototk(2, message)]
    pub leader: ReplicaID,
}

impl Ballot {
    /// The smallest possible ballot.
    pub const BOTTOM: Ballot = Ballot {
        number: 0,
        leader: ReplicaID::BOTTOM,
    };

    /// The largest possible ballot.
    pub const TOP: Ballot = Ballot {
        number: u64::max_value(),
        leader: ReplicaID::TOP,
    };
}

impl Default for Ballot {
    fn default() -> Self {
        Self::BOTTOM
    }
}

/////////////////////////////////////////// Configuration //////////////////////////////////////////

/// A configuration of the Paxos ensemble.
#[derive(Clone, Default, Eq, Message, PartialEq)]
pub struct Configuration {
    /// The Paxos ensemble this configuration represents.
    #[prototk(1, message)]
    pub group: PaxosID,
    /// A sequence number indicating how many prior configurations this ensemble has had.  The
    /// first valid configuration is epoch=1, allowing a default configuration to take epoch=0.
    #[prototk(2, uint64)]
    pub epoch: u64,
    /// Alpha may be any positive value.  Defaults to 1.
    /// Alpha must be a positive value.  Defaults to DEFAULT_ALPHA.  MAX_ALPHA is maximum value.
    /// MIN_ALPHA is minimum value.
    #[prototk(3, uint64)]
    pub alpha: u64,
    /// Replicas involved in this ensemble.  These will be used for acceptors, learners, and
    /// proposers, and to provide the durability guarantees for the cluster.  All data gets written
    /// to at least a quorum of replicas before it is considered committed.
    #[prototk(4, message)]
    pub replicas: Vec<ReplicaID>,
}

impl Configuration {
    /// Is the provided replica a member of this Paxos ensemble.
    pub fn is_replica(&self, replica: ReplicaID) -> bool {
        self.replicas.iter().any(|r| r == &replica)
    }
}

impl std::fmt::Debug for Configuration {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        let replicas: Vec<String> = self.replicas.iter().map(|x| x.human_readable()).collect();
        f.debug_struct("Configuration")
            .field("group", &self.group.human_readable())
            .field("epoch", &self.epoch)
            .field("alpha", &self.alpha)
            .field("replicas", &replicas)
            .finish()
    }
}

////////////////////////////////////////////// Command /////////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, Message, PartialEq)]
enum Command {
    #[prototk(1, message)]
    #[default]
    Nop,
    #[prototk(2, message)]
    Call {
        #[prototk(1, string)]
        object: String,
        #[prototk(2, string)]
        method: String,
        #[prototk(3, bytes)]
        req: Vec<u8>,
    },
    #[prototk(3, message)]
    Poke {
        #[prototk(1, string)]
        message: String,
    },
    #[prototk(4, message)]
    Tick {
        #[prototk(1, message)]
        replica: ReplicaID,
        #[prototk(2, uint64)]
        clock_ms: u64,
    },
}

////////////////////////////////////////////// PValue //////////////////////////////////////////////

/// A Proposed Value, or PValue, is commonly referred to as a "decree" in the Paxos papers.
///
/// PValues are triples of (slot, ballot, command) and can be interpreted as, "The proposer
/// championing `ballot` proposes putting `command` into `slot`".
#[derive(Clone, Debug, Eq, Message, PartialEq)]
pub struct PValue {
    #[prototk(1, uint64)]
    slot: u64,
    #[prototk(2, message)]
    ballot: Ballot,
    #[prototk(3, message)]
    cmd: Command,
}

impl Default for PValue {
    fn default() -> Self {
        Self {
            slot: 0,
            ballot: Ballot::BOTTOM,
            cmd: Command::default(),
        }
    }
}

////////////////////////////////////////////// Phase1A /////////////////////////////////////////////

/// Phase1A messages rally support for a new ballot.  They are answered with Phase1B messages.
#[derive(Clone, Debug, Default, Message)]
pub struct Phase1A {
    #[prototk(1, message)]
    ballot: Ballot,
    #[prototk(2, uint64)]
    starting_slot: u64,
    #[prototk(3, uint64)]
    ending_slot: u64,
    #[prototk(4, message)]
    group: PaxosID,
}

////////////////////////////////////////////// Phase1B /////////////////////////////////////////////

/// Phase1B messages retrun whether an acceptor supports a ballot.
#[derive(Clone, Debug, Default, Message)]
pub struct Phase1B {
    #[prototk(1, message)]
    ballot: Ballot,
    #[prototk(2, message)]
    pvalues: Vec<PValue>,
    #[prototk(4, message)]
    group: PaxosID,
}

////////////////////////////////////////////// Phase2A /////////////////////////////////////////////

/// Phase2A messages use a previously rallied ballot to assign a [PValue].  The PValue provides the
/// slot and ballot corresponding to a Phase1A message.
#[derive(Clone, Debug, Default, Message)]
pub struct Phase2A {
    #[prototk(1, message)]
    pvalue: PValue,
    #[prototk(4, message)]
    group: PaxosID,
}

////////////////////////////////////////////// Phase2B /////////////////////////////////////////////

/// [Phase2B] messages return whether a [Phase2A] message was accepted.
#[derive(Clone, Debug, Default, Message)]
pub struct Phase2B {
    #[prototk(1, Bool)]
    accepted: bool,
    #[prototk(4, message)]
    group: PaxosID,
}

///////////////////////////////////////////// Acceptor /////////////////////////////////////////////

// [AcceptorService] serves as the mutable memory for the system.  All requests get decided by
// interacting with the acceptors.
service! {
    name = AcceptorService;
    server = AcceptorServer;
    client = AcceptorClient;
    error = Error;

    rpc phase1(Phase1A) -> Phase1B;
    rpc phase2(Phase2A) -> Phase2B;
}

///////////////////////////////////////// GenNoncesRequest /////////////////////////////////////////

/// [GenNoncesRequest] messages embed the number of nonces to fetch.
#[derive(Clone, Debug, Default, Message)]
pub struct GenNoncesRequest {
    #[prototk(1, uint64)]
    count: u64,
    #[prototk(4, message)]
    group: PaxosID,
}

///////////////////////////////////////// GenNoncesResponse ////////////////////////////////////////

/// [GenNoncesResponse] messages embed the first nonce in a sequence of `count` nonces.
#[derive(Clone, Debug, Default, Message)]
pub struct GenNoncesResponse {
    #[prototk(1, uint64)]
    first_nonce: u64,
    #[prototk(4, message)]
    group: PaxosID,
}

//////////////////////////////////////// IssueCommandRequest ///////////////////////////////////////

/// [IssueCommandRequest] issues a command under the given nonce.  It is permissible to make this
/// call indefinitely until it either completes successfully or returns a terminal error.
/// Transient errors can be overcome as the nonce will be checked.
#[derive(Clone, Debug, Default, Message)]
pub struct IssueCommandRequest {
    #[prototk(1, uint64)]
    nonce: u64,
    #[prototk(2, message)]
    command: Command,
    #[prototk(4, message)]
    group: PaxosID,
}

/////////////////////////////////////// IssueCommandResponse ///////////////////////////////////////

/// [IssueCommandResponse] is the result of a command issued under a given nonce.  For the memory
/// of the proposer this will result in the same answer if retried.
#[derive(Clone, Debug, Default, Message)]
pub struct IssueCommandResponse {
    #[prototk(1, uint64)]
    nonce: u64,
    #[prototk(4, message)]
    group: PaxosID,
}

///////////////////////////////////////////// Proposer /////////////////////////////////////////////

// [ProposerService] is responseible for generating nonces and then issuing requests using those
// nonces.  The nonce is guaranteed to serve as a clock timestamp among all other generated nonces.
// Eventually we may make a guarantee to pin the nonce to a wall-clock and real time.
service! {
    name = ProposerService;
    server = ProposerServer;
    client = ProposerClient;
    error = Error;

    rpc gen_nonces(GenNoncesRequest) -> GenNoncesResponse;
    rpc issue_command(IssueCommandRequest) -> IssueCommandResponse;
}
