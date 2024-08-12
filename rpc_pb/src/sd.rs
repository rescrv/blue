//! Service discovery is the act of translating a [Host] into a real-world address.

use std::collections::BTreeMap;
use std::ops::Bound;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use one_two_eight::{generate_id, generate_id_prototk};

use prototk_derive::Message;

use zerror::Z;
use zerror_core::ErrorCore;

use super::{service, Context, Error};

//////////////////////////////////////////////// IDs ///////////////////////////////////////////////

generate_id! {EnvironmentID, "env:"}
generate_id_prototk! {EnvironmentID}

generate_id! {HostID, "host:"}
generate_id_prototk! {HostID}

/////////////////////////////////////////////// Host ///////////////////////////////////////////////

/// A Host captures a process-unique, stable identifier with its connection string.
#[derive(Clone, Debug, Default, Message, Eq, PartialEq)]
pub struct Host {
    #[prototk(1, message)]
    host_id: HostID,
    #[prototk(2, string)]
    connect: String,
}

impl Host {
    /// Get the ID for this host.
    pub fn host_id(&self) -> HostID {
        self.host_id
    }

    /// Get the connection string for this host.
    pub fn connect(&self) -> &str {
        &self.connect
    }

    /// Get the hostname for this host, inferring if a port can be stripped.
    pub fn hostname_or_ip(&self) -> &str {
        let connect = &self.connect;
        fn strip_port(connect: &str) -> &str {
            if let Some((host, _)) = connect.rsplit_once(':') {
                host
            } else {
                connect
            }
        }
        if connect.starts_with('[') {
            let connect = strip_port(connect);
            if connect.ends_with(']') {
                &connect[1..connect.len() - 1]
            } else {
                &self.connect
            }
        } else {
            strip_port(connect)
        }
    }
}

impl FromStr for Host {
    type Err = Error;

    fn from_str(s: &str) -> Result<Host, Error> {
        let parts: Vec<String> = s.split(';').map(String::from).collect();
        if parts.len() != 2 {
            return Err(Error::ResolveFailure {
                core: ErrorCore::default(),
                what: "could not parse string".to_owned(),
            }
            .with_info("parts", parts));
        }
        let host_id: HostID = match parts[0].parse::<HostID>() {
            Ok(host_id) => host_id,
            Err(err) => {
                return Err(Error::ResolveFailure {
                    core: ErrorCore::default(),
                    what: "could not parse HostID".to_owned(),
                }
                .with_info("err", err)
                .with_info("host_id", parts[0].to_owned()));
            }
        };
        Ok(Host {
            host_id,
            connect: parts[1].to_owned(),
        })
    }
}

///////////////////////////////////////// ServiceDiscovery /////////////////////////////////////////

/// RegisterRequest captures the request to register a host in a given environment.
#[derive(Clone, Debug, Default, Message, Eq, PartialEq)]
pub struct RegisterRequest {
    #[prototk(1, message)]
    env_id: EnvironmentID,
    #[prototk(2, string)]
    deployment: String,
    #[prototk(3, message)]
    host_id: HostID,
    #[prototk(4, message)]
    host: Host,
}

/// RegisterResponse indicates the registration request succeeded.
#[derive(Clone, Debug, Default, Message, Eq, PartialEq)]
pub struct RegisterResponse {}

/// ResolveRequest requests a set of hosts for a given environment/deployment.
#[derive(Clone, Debug, Default, Message, Eq, PartialEq)]
pub struct ResolveRequest {
    #[prototk(1, message)]
    env_id: EnvironmentID,
    #[prototk(2, string)]
    deployment: String,
    #[prototk(3, message)]
    consistent_hash: HostID,
    #[prototk(4, uint32)]
    count: u32,
}

/// ResolveResponse lists a set of hosts in response to a ResolveRequest.
#[derive(Clone, Debug, Default, Message)]
pub struct ResolveResponse {
    #[prototk(1, message)]
    hosts: Vec<Host>,
}

////////////////////////////////////// ServiceDiscoveryService /////////////////////////////////////

service! {
    name = ServiceDiscoveryService;
    server = ServiceDiscoveryServer;
    client = ServiceDiscoveryClient;
    error = Error;

    rpc register(RegisterRequest) -> RegisterResponse;
    rpc resolve(ResolveRequest) -> ResolveResponse;
}

///////////////////////////////////////// ServiceDiscovery /////////////////////////////////////////

/// An in-memory implementation of service discovery.
#[derive(Default)]
pub struct ServiceDiscovery {
    env_id: EnvironmentID,
    #[allow(clippy::type_complexity)]
    by_deployment: Mutex<BTreeMap<String, Arc<Mutex<BTreeMap<HostID, Host>>>>>,
}

impl ServiceDiscovery {
    /// Create a new service discovery instance for the given environment.
    pub fn new(env_id: EnvironmentID) -> Result<Self, Error> {
        if env_id == EnvironmentID::default() {
            Err(Error::NotFound {
                core: ErrorCore::default(),
                what: "default EnvironmentID will never be found".to_owned(),
            })
        } else {
            Ok(Self {
                env_id,
                by_deployment: Mutex::default(),
            })
        }
    }

    fn check_environment(&self, env_id: EnvironmentID) -> Result<(), Error> {
        if self.env_id != env_id {
            Err(Error::NotFound {
                core: ErrorCore::default(),
                what: "environment".to_owned(),
            })
        } else {
            Ok(())
        }
    }

    fn get_deployment(
        &self,
        deployment: String,
    ) -> Result<Arc<Mutex<BTreeMap<HostID, Host>>>, Error> {
        let mut by_deployment = self.by_deployment.lock().unwrap();
        if !by_deployment.contains_key(&deployment) {
            by_deployment.insert(deployment.clone(), Arc::new(Mutex::new(BTreeMap::new())));
        }
        Ok(Arc::clone(by_deployment.get(&deployment).ok_or(
            Error::NotFound {
                core: ErrorCore::default(),
                what: "deployment".to_owned(),
            },
        )?))
    }
}

impl ServiceDiscoveryService for ServiceDiscovery {
    fn register(&self, _: &Context, req: RegisterRequest) -> Result<RegisterResponse, Error> {
        self.check_environment(req.env_id)?;
        let deployment = self.get_deployment(req.deployment)?;
        let mut deployment = deployment.lock().unwrap();
        deployment.insert(req.host_id, req.host);
        Ok(RegisterResponse {})
    }

    fn resolve(&self, _: &Context, req: ResolveRequest) -> Result<ResolveResponse, Error> {
        self.check_environment(req.env_id)?;
        let deployment = self.get_deployment(req.deployment)?;
        let deployment = deployment.lock().unwrap();
        let to_take = std::cmp::min(req.count, 5) as usize;
        let first = deployment.range((Bound::Included(req.consistent_hash), Bound::Unbounded));
        let second = deployment.range((Bound::Unbounded, Bound::Included(req.consistent_hash)));
        let hosts: Vec<Host> = first
            .chain(second)
            .take(to_take)
            .map(|x| x.1.clone())
            .collect();
        Ok(ResolveResponse { hosts })
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty() {
        let _sd = ServiceDiscovery::default();
    }

    #[test]
    fn standard_flow() {
        let env_id = EnvironmentID::generate().unwrap();
        let mut hosts = Vec::new();
        let sd = ServiceDiscovery::new(env_id).unwrap();
        for i in 0..16 {
            let host_id = HostID::generate().unwrap();
            hosts.push(host_id);
            assert_eq!(
                RegisterResponse {},
                sd.register(
                    &Context::default(),
                    RegisterRequest {
                        env_id,
                        deployment: "some-deployment".to_owned(),
                        host_id,
                        host: Host {
                            host_id,
                            connect: format!("127.0.0.1:{}", 2049 + i),
                        },
                    }
                )
                .unwrap()
            );
        }
        hosts.sort();
        for idx in 0..hosts.len() {
            let host_ids: Vec<HostID> = sd
                .resolve(
                    &Context::default(),
                    ResolveRequest {
                        env_id,
                        deployment: "some-deployment".to_owned(),
                        consistent_hash: hosts[idx],
                        count: 3,
                    },
                )
                .unwrap()
                .hosts
                .iter()
                .map(|h| h.host_id)
                .collect();
            for i in 0..3 {
                assert_eq!(hosts[(idx + i) % hosts.len()], host_ids[i]);
            }
        }
    }
}
