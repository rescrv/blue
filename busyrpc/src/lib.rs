use biometrics::Collector;

use rpc_pb::sd::Host;

mod buffers;
mod channel;
mod client;
mod poll;
mod resolve;
mod server;

pub mod builtins;

pub use client::{Client, ClientOptions};
pub use resolve::StringResolver;
pub use server::{Server, ServerOptions, ServiceRegistry};

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

pub fn register_biometrics(collector: &mut Collector) {
    client::register_biometrics(collector);
    channel::register_biometrics(collector);
    server::register_biometrics(collector);
    poll::register_biometrics(collector);
}

///////////////////////////////////////////// Resolver /////////////////////////////////////////////

pub trait Resolver {
    fn resolve(&mut self) -> Result<Host, rpc_pb::Error>;
}
