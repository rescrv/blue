//! Generate a probably-unique HostID.

use rpc_pb::HostID;

fn main() {
    println!("{}", HostID::generate().unwrap())
}
