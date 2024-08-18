#![doc = include_str!("../README.md")]

use biometrics::Collector;

mod buffers;
mod channel;
mod client;
mod poll;
mod resolve;
mod server;

pub mod builtins;

pub use client::{new_client, ClientOptions};
pub use resolve::StringResolver;
pub use server::{Server, ServerOptions, ServiceRegistry};

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

/// Register the biometrics for this crate.
pub fn register_biometrics(collector: &mut Collector) {
    client::register_biometrics(collector);
    channel::register_biometrics(collector);
    server::register_biometrics(collector);
    poll::register_biometrics(collector);
}

////////////////////////////////////////////// indicio /////////////////////////////////////////////

pub static COLLECTOR: indicio::Collector = indicio::Collector::new();
