// TODO(rescrv): better error handling than string.

use std::str::FromStr;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use prototk::FieldNumber;
use rpc_pb::{Host, HostID};
use tuple_key::{Direction, TupleKey};
use utf8path::Path;

mod parser;

use parser::ParseError;

//////////////////////////////////////////// RoutingConf ///////////////////////////////////////////

pub struct RoutingConf {
    schema: tuple_key::Schema<()>,
}

impl RoutingConf {
    pub fn schema(&self) -> &tuple_key::Schema<()> {
        &self.schema
    }

    pub fn keys(&self, routing: impl AsRef<str>) -> Result<Vec<TupleKey>, String> {
        let mut args = routing
            .as_ref()
            .split_whitespace()
            .map(String::from)
            .collect::<Vec<_>>();
        let mut keys = vec![];
        while !args.is_empty() {
            let key: TupleKey;
            (key, args) = self.key(args)?;
            keys.push(key);
        }
        Ok(keys)
    }

    pub fn args(&self, keys: &[TupleKey]) -> Result<Vec<String>, String> {
        let mut args = vec![];
        for key in keys {
            let a = self
                .schema
                .args_for_key(key)
                .map_err(|err| format!("{err:?}"))?;
            if !args.is_empty() {
                args.push("--".to_string());
            }
            args.extend(a.into_iter());
        }
        Ok(args)
    }

    fn key(&self, args: Vec<String>) -> Result<(TupleKey, Vec<String>), String> {
        let mut args = args.into_iter().peekable();
        let mut tk = TupleKey::default();
        let mut schema: &tuple_key::Schema<()> = &self.schema;
        while let Some(var_name) = args.next() {
            if !var_name.starts_with("--") {
                return Err(format!("expected --parameter, got {var_name:?}"));
            }
            if parser::parse_all(parser::identifier)(&var_name[2..]).is_err() {
                return Err(format!("expected identifier, got {:?}", &var_name[2..]));
            }
            let Some(field_number) = schema.field_number(&var_name[2..]) else {
                return Err(format!("{:?} not in routing table", &var_name[2..]));
            };
            if let Some(s) = schema.child(field_number) {
                schema = s;
            } else {
                return Err("internal inconsistency in routing table: field number succeeded, but schema failed".to_string());
            }
            if let Some(next) = args.next() {
                if next == "--" {
                    tk.extend(field_number);
                    break;
                } else if parser::parse_all(parser::identifier)(&next).is_err() {
                    return Err(format!("expected identifier, got {next:?}"));
                } else {
                    tk.extend_with_key(field_number, next, Direction::Forward);
                }
            } else {
                tk.extend(field_number);
                break;
            }
        }
        Ok((tk, args.collect()))
    }
}

impl Default for RoutingConf {
    fn default() -> Self {
        // SAFETY(rescrv):  This is tested.
        Self::from_str("").unwrap()
    }
}

impl FromStr for RoutingConf {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let schema = parser::parse_all(parser::schema)(s.trim())?;
        Ok(Self { schema })
    }
}

////////////////////////////////////////////// binding /////////////////////////////////////////////

/// Binding represents the relationship between a routing key, a service key, and a host.
#[derive(Clone, Debug, Default, Eq, PartialEq, prototk_derive::Message)]
pub struct Binding<'a> {
    #[prototk(1, bytes)]
    pub routing_key: &'a [u8],
    #[prototk(2, bytes)]
    pub service_key: &'a [u8],
    #[prototk(3, message)]
    pub host: Host,
}

////////////////////////////////////////// RegisterRequest /////////////////////////////////////////

/// RegisterRequest captures the request to register a host for the given bindings.
#[derive(Clone, Debug, Default, Eq, PartialEq, prototk_derive::Message)]
pub struct RegisterRequest<'a> {
    #[prototk(1, message)]
    pub bindings: Vec<Binding<'a>>,
}

///////////////////////////////////////// RegisterResponse /////////////////////////////////////////

/// RegisterResponse indicates the registration request succeeded.
#[derive(Clone, Debug, Default, Eq, PartialEq, prototk_derive::Message)]
pub struct RegisterResponse {
    #[prototk(2, uint64)]
    pub time_to_live_secs: u64,
}

///////////////////////////////////////// UnregisterRequest ////////////////////////////////////////

/// UnregisterRequest captures the request to unbind a host from all bindings in service discovery.
#[derive(Clone, Debug, Default, Eq, PartialEq, prototk_derive::Message)]
pub struct UnregisterRequest<'a> {
    #[prototk(1, message)]
    pub bindings: Vec<Binding<'a>>,
}

//////////////////////////////////////// UnregisterResponse ////////////////////////////////////////

/// UnregisterResponse indicates the request has been enqueued and will take effect shortly.
#[derive(Clone, Debug, Default, Eq, PartialEq, prototk_derive::Message)]
pub struct UnregisterResponse {}

////////////////////////////////////////// ResolveRequest //////////////////////////////////////////

/// ResolveRequest requests a set of hosts for a given routing key.
#[derive(Clone, Debug, Default, Eq, PartialEq, prototk_derive::Message)]
pub struct ResolveRequest<'a> {
    #[prototk(3, bytes)]
    pub routing_key: &'a [u8],
    #[prototk(4, bytes)]
    pub service_key: &'a [u8],
    #[prototk(5, uint32)]
    pub limit: u32,
}

////////////////////////////////////////// ResolveResponse /////////////////////////////////////////

/// ResolveResponse lists a set of hosts in response to a ResolveRequest.
#[derive(Clone, Debug, Default, prototk_derive::Message)]
pub struct ResolveResponse {
    #[prototk(4, bytes)]
    pub service_key: Vec<u8>,
    #[prototk(6, message)]
    pub hosts: Vec<Host>,
}

///////////////////////////////////////// ServiceDiscovery /////////////////////////////////////////

rpc_pb::service! {
    name = ServiceDiscovery;
    server = ServiceDiscoveryServer;
    client = ServiceDiscoveryClient;
    error = rpc_pb::Error;

    rpc register(RegisterRequest) -> RegisterResponse;
    rpc unregister(UnregisterRequest) -> UnregisterResponse;
    rpc resolve(ResolveRequest) -> ResolveResponse;
}

///////////////////////////////////////////// register /////////////////////////////////////////////

pub fn register<'a>(
    client: Arc<dyn rpc_pb::Client + Send + Sync>,
    routing_conf_path: impl Into<Path<'a>>,
    routing: impl AsRef<str>,
    service_key: TupleKey,
    host: Host,
    mut error_handling: impl FnMut(rpc_pb::Error) + Send + 'static,
) -> Result<impl FnOnce(), rpc_pb::Error> {
    let routing_conf = std::fs::read_to_string(routing_conf_path.into().as_str())?;
    let routing_conf = routing_conf.parse::<RoutingConf>().map_err(|err| {
        rpc_pb::Error::resolve_failure(format!("routing conf doesn't parse: {err}"))
    })?;
    let routing_keys = routing_conf
        .keys(routing.as_ref())
        .map_err(|err| rpc_pb::Error::resolve_failure(format!("routing key error: {err}")))?;
    let background = sync42::background::BackgroundThread::spawn(move |done| {
        let sd = ServiceDiscoveryClient::new(client);
        let ctx = rpc_pb::Context::default();
        let mut bindings = vec![];
        for routing_key in routing_keys.iter() {
            bindings.push(Binding {
                routing_key,
                service_key: &service_key,
                host: host.clone(),
            });
        }
        while !done.load(Ordering::Relaxed) {
            let req = RegisterRequest {
                bindings: bindings.clone(),
            };
            let time_to_live_secs = match sd.register(&ctx, req) {
                Ok(resp) => resp.time_to_live_secs,
                Err(err) => {
                    error_handling(err);
                    60
                }
            };
            for _ in 0..(time_to_live_secs * 10).clamp(0, 6000) {
                if done.load(Ordering::Relaxed) {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
        let req = UnregisterRequest {
            bindings: bindings.clone(),
        };
        let _ = sd.unregister(&ctx, req);
    });
    Ok(move || {
        background.join();
    })
}

////////////////////////////////////////// simple_resolver /////////////////////////////////////////

struct SimpleResolver {
    client: Arc<dyn rpc_pb::Client + Send + Sync>,
    routing_key: TupleKey,
}

impl rpc_pb::Resolver for SimpleResolver {
    fn resolve(&mut self) -> Result<Host, rpc_pb::Error> {
        let ctx = rpc_pb::Context::default();
        let sd = ServiceDiscoveryClient::new(Arc::clone(&self.client));
        let service_key = vec![];
        let req = ResolveRequest {
            routing_key: &self.routing_key,
            service_key: &service_key,
            limit: 1,
        };
        let mut resp = sd.resolve(&ctx, req)?;
        if let Some(host) = resp.hosts.pop() {
            Ok(host)
        } else {
            Err(rpc_pb::Error::resolve_failure("no hosts available"))
        }
    }
}

pub fn simple_resolver<'a>(
    client: Arc<dyn rpc_pb::Client + Send + Sync>,
    routing_conf_path: impl Into<Path<'a>>,
    routing: impl AsRef<str>,
) -> Result<impl rpc_pb::Resolver, rpc_pb::Error> {
    let routing_conf = std::fs::read_to_string(routing_conf_path.into().as_str())?;
    let routing_conf = routing_conf.parse::<RoutingConf>().map_err(|err| {
        rpc_pb::Error::resolve_failure(format!("routing conf doesn't parse: {err}"))
    })?;
    let mut routing_keys = routing_conf
        .keys(routing.as_ref())
        .map_err(|err| rpc_pb::Error::resolve_failure(format!("routing key error: {err}")))?;
    if routing_keys.len() != 1 {
        return Err(rpc_pb::Error::resolve_failure(format!(
            "expected exactly one routing key; got: {}",
            routing_keys.len()
        )));
    }
    // SAFETY(rescrv):  Length check immediately above.
    let routing_key = routing_keys.pop().unwrap();
    Ok(SimpleResolver {
        client,
        routing_key,
    })
}

/////////////////////////////////////// service key utilities //////////////////////////////////////

pub fn service_key_for_host_id(host_id: HostID) -> TupleKey {
    let mut service_key = TupleKey::default();
    service_key.extend_with_key(
        FieldNumber::must(1),
        host_id.prefix_free_readable(),
        Direction::Forward,
    );
    service_key
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
pub mod tests {
    use prototk::FieldNumber;

    use super::*;

    #[test]
    fn routing_conf() {
        let routing_conf = r#"
        metro = 1 {
            customer = 2 {
                srv = 3;
            }
        }
        srv = 4;
        "#
        .parse::<RoutingConf>()
        .unwrap();
        let mut expected3 = TupleKey::default();
        expected3.extend_with_key(FieldNumber::must(1), "sjc".to_string(), Direction::Forward);
        expected3.extend_with_key(FieldNumber::must(2), "acme".to_string(), Direction::Forward);
        expected3.extend(FieldNumber::must(3));
        assert_eq!(
            Ok(vec![expected3.clone()]),
            routing_conf.keys("--metro sjc --customer acme --srv")
        );
        let mut expected4 = TupleKey::default();
        expected4.extend(FieldNumber::must(4));
        assert_eq!(Ok(vec![expected4.clone()]), routing_conf.keys("--srv"));
        assert_eq!(
            Ok(vec![expected3, expected4]),
            routing_conf.keys("--metro sjc --customer acme --srv -- --srv")
        );
    }
}
