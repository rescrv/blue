use arrrg::CommandLine;
use busyrpc::{new_client, ClientOptions, SslOptions, StringResolver};
use prototk::FieldNumber;
use rpc_pb::{Host, IoToZ};
use tuple_key::{Direction, TupleKey};
use tuple_routing::{Binding, ServiceDiscovery, UnregisterRequest};

#[derive(Clone, Debug, Default, Eq, PartialEq, arrrg_derive::CommandLine)]
struct Options {
    #[arrrg(nested)]
    ssl: SslOptions,
    #[arrrg(nested)]
    service_discovery_client: ClientOptions,
    #[arrrg(
        required,
        "Host connection string in for ServiceDiscovery in host:ID=hostname:port,host:ID=hostname:port format"
    )]
    service_discovery_connect: StringResolver,
    #[arrrg(required, "The routing configuration to use for service discovery.")]
    routing_conf: String,
    #[arrrg(required, "The route to register in service discovery.")]
    routing: String,
    #[arrrg(required, "The host to register in service discovery.")]
    host: Host,
}

fn main() {
    let (options, free) =
        Options::from_command_line("Usage: busyrpc-service-discovery-register [OPTIONS]");
    if !free.is_empty() {
        eprintln!("command takes no positional arguments");
        std::process::exit(1);
    }
    // Get the routing key.
    let routing_conf = std::fs::read_to_string(options.routing_conf).unwrap();
    let routing_conf = routing_conf.parse::<tuple_routing::RoutingConf>().unwrap();
    let routing_keys = routing_conf.keys(options.routing).unwrap();
    // Use the HostID as a service key.
    let mut service_key = TupleKey::default();
    service_key.extend_with_key(
        FieldNumber::must(1),
        options.host.host_id().prefix_free_readable(),
        Direction::Forward,
    );
    // Request unregistration.
    let client = new_client(
        options.ssl,
        options.service_discovery_client,
        options.service_discovery_connect.clone(),
    );
    let sd = tuple_routing::ServiceDiscoveryClient::new(client);
    let ctx = rpc_pb::Context::default();
    let mut bindings = vec![];
    for routing_key in routing_keys.iter() {
        bindings.push(Binding {
            routing_key,
            service_key: &service_key,
            host: options.host.clone(),
        });
    }
    let req = UnregisterRequest { bindings };
    sd.unregister(&ctx, req).as_z().pretty_unwrap();
}
