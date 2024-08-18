use std::sync::Arc;

use arrrg::CommandLine;
use busyrpc::{Server, ServerOptions, ServiceRegistry, SslOptions};
use indicio::{
    clue,
    stdio::StdioEmitter,
    {ALWAYS, INFO},
};
use rpc_pb::IoToZ;

#[derive(Debug, Default, Eq, PartialEq, arrrg_derive::CommandLine)]
struct Options {
    #[arrrg(nested)]
    ssl: SslOptions,
    #[arrrg(required, "The routing configuration to use for service discovery.")]
    routing_conf: String,
    #[arrrg(nested)]
    server: ServerOptions,
}

fn main() {
    minimal_signals::block();
    let (options, free) =
        Options::from_command_line("Usage: busyrpc-service-discovery-server [OPTIONS]");
    if !free.is_empty() {
        eprintln!("command takes no arguments");
        std::process::exit(1);
    }
    // indicio
    let emitter = Arc::new(StdioEmitter);
    busyrpc_service_discovery::COLLECTOR.register(emitter);
    busyrpc_service_discovery::COLLECTOR.set_verbosity(INFO);
    clue!(busyrpc_service_discovery::COLLECTOR, ALWAYS, {
        new_process: std::env::args().map(String::from).collect::<Vec<_>>(),
    });
    // service discovery service
    let routing_conf = std::fs::read_to_string(options.routing_conf).unwrap();
    let routing_conf = routing_conf.parse::<tuple_routing::RoutingConf>().unwrap();
    let sd = busyrpc_service_discovery::MemoryServiceDiscovery::new(routing_conf);
    // services
    let mut services = ServiceRegistry::new();
    services.register(
        "ServiceDiscovery",
        tuple_routing::ServiceDiscoveryServer::bind(sd),
    );
    // server
    let (server, cancel) = Server::new(options.ssl, options.server, services)
        .as_z()
        .pretty_unwrap();
    let _ = std::thread::spawn(move || {
        loop {
            let signal_set = minimal_signals::SignalSet::new().fill();
            let signal = minimal_signals::wait(signal_set);
            if signal != Some(minimal_signals::SIGCHLD) {
                break;
            }
        }
        cancel();
    });
    server.serve().as_z().pretty_unwrap();
    // log goodbye
    clue!(busyrpc_service_discovery::COLLECTOR, ALWAYS, {
        goodbye: std::env::args().map(String::from).collect::<Vec<_>>(),
    });
}
