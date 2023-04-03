use prototk_derive::Message;

use rpc::service;

mod common;

use common::Error;

//////////////////////////////////////////// The Service ///////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
struct PingRequest {}

#[derive(Clone, Debug, Default, Message)]
struct PingResponse {}

service! {
    name = Ping; // No magic.  The name of the trait for this service.
    server = PingServer; // No magic.  The name of the type for the server.
    client = PingClient; // No magic.  The name of the type for the client.
    rpc ping1(PingRequest) -> PingResponse;
    rpc ping2(PingRequest) -> PingResponse;
}

fn main() {}
