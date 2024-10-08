use std::collections::BTreeMap;
use std::ops::Bound;
use std::sync::Mutex;

use indicio::{clue, ERROR, INFO};
use rpc_pb::{Host, HostID};
use tuple_key::TupleKey;
use tuple_routing::{
    Binding, RegisterRequest, RegisterResponse, ResolveRequest, ResolveResponse, RoutingConf,
    ServiceDiscovery, UnregisterRequest, UnregisterResponse,
};
use zerror_core::ErrorCore;

////////////////////////////////////////////// indicio /////////////////////////////////////////////

pub static COLLECTOR: indicio::Collector = indicio::Collector::new();

////////////////////////////////////// MemoryServiceDiscovery //////////////////////////////////////

pub struct MemoryServiceDiscovery {
    conf: RoutingConf,
    hosts: Mutex<BTreeMap<TupleKey, Host>>,
}

impl MemoryServiceDiscovery {
    pub fn new(conf: RoutingConf) -> Self {
        let hosts = Mutex::default();
        Self { conf, hosts }
    }

    fn key_and_host_for_binding(
        &self,
        binding: Binding,
    ) -> Result<(TupleKey, TupleKey, Host), rpc_pb::Error> {
        self.check_host(&binding.host)?;
        let routing_key = self.check_routing_key(binding.routing_key)?;
        let mut service_key = routing_key.clone();
        service_key.append(&mut TupleKey::from(binding.service_key));
        Ok((routing_key, service_key, binding.host))
    }

    fn check_routing_key(&self, routing_key: &[u8]) -> Result<TupleKey, rpc_pb::Error> {
        let routing_key = TupleKey::from(routing_key);
        if self.conf.schema().lookup(&routing_key).is_err() {
            return Err(rpc_pb::Error::NotFound {
                core: ErrorCore::default(),
                what: "routing key doesn't match any hosts in the host map".to_string(),
            });
        }
        // SAFETY(rescrv):  lookup succeeded and schema guarantees that this will succeed for any
        // call lookup succeeds for.
        if !self.conf.schema().is_terminal(&routing_key).unwrap() {
            return Err(rpc_pb::Error::NotFound {
                core: ErrorCore::default(),
                what: "routing key is not terminal".to_string(),
            });
        }
        Ok(routing_key)
    }

    fn check_host(&self, host: &Host) -> Result<(), rpc_pb::Error> {
        if host.host_id() == HostID::BOTTOM {
            Err(rpc_pb::Error::resolve_failure(
                "host id not permitted to be bottom",
            ))
        } else if host.host_id() == HostID::TOP {
            Err(rpc_pb::Error::resolve_failure(
                "host id not permitted to be top",
            ))
        } else {
            Ok(())
        }
    }
}

impl ServiceDiscovery for MemoryServiceDiscovery {
    fn register(
        &self,
        _: &rpc_pb::Context,
        req: RegisterRequest,
    ) -> Result<RegisterResponse, rpc_pb::Error> {
        let RegisterRequest { bindings } = req;
        let mut keys = vec![];
        for binding in bindings.into_iter() {
            keys.push(self.key_and_host_for_binding(binding)?);
        }
        let to_log = keys.clone();
        {
            let mut hosts = self.hosts.lock().unwrap();
            for (_, service_key, host) in keys.into_iter() {
                hosts.insert(service_key, host);
            }
        }
        for (routing_key, _, host) in to_log.into_iter() {
            let Some(route) = self.conf.args(&[routing_key]).ok() else {
                clue!(COLLECTOR, ERROR, {
                    error: {
                        human: "invalid key generated by key_and_host_for_binding",
                    }
                });
                continue;
            };
            clue!(COLLECTOR, INFO, {
                register: {
                    args: route,
                    host: indicio::Value::from(host),
                },
            });
        }
        Ok(RegisterResponse {
            time_to_live_secs: 60,
        })
    }

    fn unregister(
        &self,
        _: &rpc_pb::Context,
        req: UnregisterRequest,
    ) -> Result<UnregisterResponse, rpc_pb::Error> {
        let UnregisterRequest { bindings } = req;
        let mut keys = vec![];
        for binding in bindings.into_iter() {
            keys.push(self.key_and_host_for_binding(binding)?);
        }
        let to_log = keys.clone();
        let mut removed = vec![false; keys.len()];
        {
            let mut hosts = self.hosts.lock().unwrap();
            for (idx, (_, service_key, host)) in keys.iter().enumerate() {
                if let Some(h) = hosts.get(service_key) {
                    if h == host {
                        removed[idx] = true;
                        hosts.remove(service_key);
                    }
                }
            }
        }
        for ((routing_key, _, host), removed) in to_log.into_iter().zip(removed) {
            if !removed {
                continue;
            }
            let Some(route) = self.conf.args(&[routing_key]).ok() else {
                clue!(COLLECTOR, ERROR, {
                    error: {
                        human: "invalid key generated by key_and_host_for_binding",
                    }
                });
                continue;
            };
            clue!(COLLECTOR, INFO, {
                unregister: {
                    args: route,
                    host: indicio::Value::from(host),
                },
            });
        }
        Ok(UnregisterResponse {})
    }

    fn resolve(
        &self,
        _: &rpc_pb::Context,
        req: ResolveRequest,
    ) -> Result<ResolveResponse, rpc_pb::Error> {
        let ResolveRequest {
            routing_key,
            service_key,
            limit,
        } = req;
        let routing_key = self.check_routing_key(routing_key)?;
        let Some(route) = self.conf.args(&[routing_key.clone()]).ok() else {
            clue!(COLLECTOR, ERROR, {
                    error: {
                    human: "invalid key generated by check_routing_key",
                }
            });
            return Err(rpc_pb::Error::resolve_failure("invalid routing key"));
        };
        let mut start_key = routing_key.clone();
        start_key.append(&mut TupleKey::from(service_key));
        let mut service_key = vec![];
        let mut hosts = vec![];
        let mut broke_early = false;
        {
            let all = self.hosts.lock().unwrap();
            for (tk, host) in all.range((Bound::Included(&start_key), Bound::Unbounded)) {
                if !tk.starts_with(&routing_key) {
                    break;
                }
                if hosts.len() < limit as usize {
                    hosts.push(host.clone());
                } else {
                    service_key.clear();
                    service_key.extend(&tk.as_bytes()[routing_key.len()..]);
                    broke_early = true;
                    break;
                }
            }
        }
        if !broke_early {
            service_key = b"\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff".to_vec();
        }
        clue!(COLLECTOR, INFO, {
            resolve: {
                args: route,
                limit: limit,
                hosts: hosts.clone(),
                service_key: format!("{service_key:?}"),
            },
        });
        Ok(ResolveResponse { service_key, hosts })
    }
}
