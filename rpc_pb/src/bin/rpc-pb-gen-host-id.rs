use rpc_pb::sd::HostID;

fn main() {
    println!("{}", HostID::generate().unwrap())
}
