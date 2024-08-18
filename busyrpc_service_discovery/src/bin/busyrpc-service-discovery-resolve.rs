use arrrg::CommandLine;
use busyrpc::{new_client, ClientOptions, StringResolver};
use rpc_pb::IoToZ;
use tuple_routing::{ResolveRequest, ServiceDiscovery};

#[derive(Clone, Debug, Default, Eq, PartialEq, arrrg_derive::CommandLine)]
struct Options {
    #[arrrg(nested)]
    client: ClientOptions,
    #[arrrg(
        required,
        "Host connection string in for ServiceDiscovery in host:ID=hostname:port,host:ID=hostname:port format"
    )]
    service_discovery_connect: StringResolver,
    #[arrrg(required, "The routing configuration to use for service discovery.")]
    routing_conf: String,
    #[arrrg(required, "The route to discover in service discovery.")]
    routing: String,
}

fn main() {
    let (options, free) =
        Options::from_command_line("Usage: busyrpc-service-discovery-resolve [OPTIONS]");
    if !free.is_empty() {
        eprintln!("command takes no positional arguments");
        std::process::exit(1);
    }
    // Get the routing key.
    let routing_conf = std::fs::read_to_string(options.routing_conf).unwrap();
    let routing_conf = routing_conf.parse::<tuple_routing::RoutingConf>().unwrap();
    let routing_keys = routing_conf.keys(options.routing).unwrap();
    // Request registration.
    let client = new_client(options.client, options.service_discovery_connect.clone());
    let sd = tuple_routing::ServiceDiscoveryClient::new(client);
    let ctx = rpc_pb::Context::default();
    for routing_key in routing_keys {
        let mut service_key = vec![];
        loop {
            let req = ResolveRequest {
                routing_key: &routing_key,
                service_key: &service_key,
                limit: 1,
            };
            let resp = sd.resolve(&ctx, req).as_z().pretty_unwrap();
            if resp.hosts.is_empty() {
                break;
            }
            for host in resp.hosts {
                println!("{host}");
            }
            service_key.clear();
            service_key.extend(resp.service_key);
        }
    }
}
