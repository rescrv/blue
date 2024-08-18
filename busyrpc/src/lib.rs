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

//////////////////////////////////////////// SslOptions ////////////////////////////////////////////

/// SSL options for client or server.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[cfg_attr(feature = "binaries", derive(arrrg_derive::CommandLine))]
pub struct SslOptions {
    /// SSL/TLS ca_file.
    #[cfg_attr(feature = "binaries", arrrg(required, "Path to the CA certificate."))]
    pub ca_file: String,
    /// SSL/TLS private key.
    #[cfg_attr(feature = "binaries", arrrg(required, "Path to the private key file."))]
    pub private_key_file: String,
    /// SSL/TLS certificate.
    #[cfg_attr(feature = "binaries", arrrg(required, "Path to the certificate file."))]
    pub certificate_file: String,
}

////////////////////////////////////////////// indicio /////////////////////////////////////////////

pub static COLLECTOR: indicio::Collector = indicio::Collector::new();
