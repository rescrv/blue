use prototk_derive::Message;

use one_two_eight::{generate_id, generate_id_prototk};

use rpc_pb::service;

use zerror_core::ErrorCore;

//////////////////////////////////////////////// IDs ///////////////////////////////////////////////

generate_id!(PaxosID, "paxos:");
generate_id_prototk!(PaxosID);

generate_id!(ReplicaID, "replica:");
generate_id_prototk!(ReplicaID);

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Debug, Default, Message)]
pub enum Error {
    #[prototk(1, message)]
    #[default]
    Success,
    #[prototk(2, message)]
    SerializationError {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, message)]
        what: prototk::Error,
    },
    #[prototk(3, message)]
    RpcError {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, message)]
        what: rpc_pb::Error,
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
    /// The smallest slot this configuration is allowed to address.  The first configuration is, by
    /// convention, anchored at slot 1.  Slot 0 is treated as a non-slot so it can be default.
    #[prototk(3, uint64)]
    pub first_slot: u64,
    /// Alpha may be any positive value.  Defaults to 1.
    #[prototk(4, uint64)]
    pub alpha: u64,
    /// Replicas involved in this ensemble.  These will be used for acceptors, learners, and
    /// proposers, and to provide the durability guarantees for the cluster.  All data gets written
    /// to at least a quorum of replicas before it is considered committed.
    #[prototk(5, message)]
    pub replicas: Vec<ReplicaID>,
}

impl std::fmt::Debug for Configuration {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        let replicas: Vec<String> = self.replicas.iter().map(|x| x.human_readable()).collect();
        f.debug_struct("Configuration")
            .field("group", &self.group.human_readable())
            .field("epoch", &self.epoch)
            .field("first_slot", &self.first_slot)
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
    Ping,
    #[prototk(2, message)]
    Reconfigure {
        #[prototk(1, message)]
        config: Configuration,
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
struct Phase1A {
    #[prototk(1, message)]
    ballot: Ballot,
    #[prototk(2, uint64)]
    starting_slot: u64,
    #[prototk(3, uint64)]
    ending_slot: u64,
}

////////////////////////////////////////////// Phase1B /////////////////////////////////////////////

/// Phase1B messages retrun whether an acceptor supports a ballot.
#[derive(Clone, Debug, Default, Message)]
struct Phase1B {
    #[prototk(1, message)]
    ballot: Ballot,
    #[prototk(2, message)]
    pvalues: Vec<PValue>,
}

////////////////////////////////////////////// Phase2A /////////////////////////////////////////////

/// Phase2A messages use a previously rallied ballot to assign a [PValue].  The PValue provides the
/// slot and ballot corresponding to a Phase1A message.
#[derive(Clone, Debug, Default, Message)]
struct Phase2A {
    #[prototk(1, message)]
    pvalue: PValue,
}

////////////////////////////////////////////// Phase2B /////////////////////////////////////////////

/// [Phase2B] messages return whether a [Phase2A] message was accepted.
#[derive(Clone, Debug, Default, Message)]
struct Phase2B {
    #[prototk(1, Bool)]
    accepted: bool,
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

///////////////////////////////////////////// Proposer /////////////////////////////////////////////

/// [GenNonceRequest] messages embed the number of nonces to fetch.
#[derive(Clone, Debug, Default, Message)]
struct GenNoncesRequest {
    #[prototk(1, uint64)]
    count: u64,
}

/// [GenNonceResponse] messages embed the first nonce in a sequence of `count` nonces.
#[derive(Clone, Debug, Default, Message)]
struct GenNoncesResponse {
    #[prototk(1, uint64)]
    nonce: u64,
}

/// [IssueCommandRequest] issues a command under the given nonce.  It is permissible to make this
/// call indefinitely until it either completes successfully or returns a terminal error.
/// Transient errors can be overcome as the nonce will be checked.
#[derive(Clone, Debug, Default, Message)]
struct IssueCommandRequest {
    #[prototk(1, uint64)]
    nonce: u64,
    //#[prototk(2, message)]
    //command: Command,
}

/// [IssueCommandResponse] is the result of a command issued under a given nonce.  For the memory
/// of the proposer this will result in the same answer if retried.
#[derive(Clone, Debug, Default, Message)]
struct IssueCommandResponse {
}

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
