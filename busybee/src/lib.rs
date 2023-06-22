use std::net::TcpStream;

use boring::ssl::{SslConnector, SslMethod, SslStream};

use biometrics::Collector;

use rpc_pb::{Context, Error};

use zerror_core::ErrorCore;

mod client;
mod server;

pub use client::{Client, ClientOptions};
pub use server::{Server, ServerOptions};

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

pub fn register_biometrics(collector: &mut Collector) {
    if !collector.ingest_swizzle(module_path!(), file!(), line!()) {
        return;
    }
    client::register_biometrics(collector);
    server::register_biometrics(collector);
}

///////////////////////////////////////////// Resolver /////////////////////////////////////////////

pub trait Resolver {
    fn lookup(&self, ctx: &Context) -> Result<SslStream<TcpStream>, Error>;
}

//////////////////////////////////////////// DnsResolver ///////////////////////////////////////////

pub struct DnsResolver {
    pub host: String,
    pub port: u16,
}

impl Resolver for DnsResolver {
    fn lookup(&self, _: &Context) -> Result<SslStream<TcpStream>, Error> {
        let mut builder =
            SslConnector::builder(SslMethod::tls()).map_err(|err| Error::TransportFailure {
                core: ErrorCore::default(),
                what: format!("could not build connector builder: {}", err),
            })?;
        // TODO(rescrv): Production blocker.  Need to sort out certs, etc.
        builder.set_verify(boring::ssl::SslVerifyMode::NONE);
        let connector = builder.build();
        let stream = TcpStream::connect(format!("{}:{}", self.host, self.port))?;
        connector
            .connect(&self.host, stream)
            .map_err(|err| Error::TransportFailure {
                core: ErrorCore::default(),
                what: format!("{}", err),
            })
    }
}
