use std::str::FromStr;

use rpc_pb::sd::Host;

use super::Resolver;

////////////////////////////////////////// StringResolver //////////////////////////////////////////

/// A StringResolver provides round-robin resolution from a set of hosts.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StringResolver {
    hosts: Vec<Host>,
    index: usize,
}

impl StringResolver {
    /// Create a new string resolver from the connect string.
    pub fn new(connect_str: &str) -> Result<Self, rpc_pb::Error> {
        let mut hosts = Vec::new();
        for host in connect_str.split(',') {
            hosts.push(host.parse::<Host>()?);
        }
        let index = hosts.len();
        Ok(Self { hosts, index })
    }
}

impl FromStr for StringResolver {
    type Err = rpc_pb::Error;

    fn from_str(s: &str) -> Result<StringResolver, rpc_pb::Error> {
        Self::new(s)
    }
}

impl Resolver for StringResolver {
    fn resolve(&mut self) -> Result<Host, rpc_pb::Error> {
        self.index += 1;
        if self.index >= self.hosts.len() {
            self.index = 0;
        }
        Ok(self.hosts[self.index].clone())
    }
}

impl std::fmt::Display for StringResolver {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let mut connect = String::new();
        for host in self.hosts.iter() {
            if !connect.is_empty() {
                connect += ",";
            }
            connect += &host.host_id().human_readable();
            connect += ";";
            connect += host.connect();
        }
        write!(fmt, "{}", connect)
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_resolver() {
        let mut string_resolver = StringResolver::new("host:d2ed2c22-c16a-2443-e401-4c6bd698afdf;127.0.0.1:8000,host:1c4e2a90-9290-2920-cb6c-c221807d666f;127.0.0.1:8001,host:76ae3e09-4e2f-2d6d-bd40-490f6fc15411;127.0.0.1:8002").unwrap();
        let host1 =
            Host::from_str("host:d2ed2c22-c16a-2443-e401-4c6bd698afdf;127.0.0.1:8000").unwrap();
        let host2 =
            Host::from_str("host:1c4e2a90-9290-2920-cb6c-c221807d666f;127.0.0.1:8001").unwrap();
        let host3 =
            Host::from_str("host:76ae3e09-4e2f-2d6d-bd40-490f6fc15411;127.0.0.1:8002").unwrap();
        assert_eq!(Ok(host1.clone()), string_resolver.resolve());
        assert_eq!(Ok(host2.clone()), string_resolver.resolve());
        assert_eq!(Ok(host3.clone()), string_resolver.resolve());
        assert_eq!(Ok(host1.clone()), string_resolver.resolve());
        assert_eq!(Ok(host2.clone()), string_resolver.resolve());
        assert_eq!(Ok(host3.clone()), string_resolver.resolve());
        assert_eq!(Ok(host1.clone()), string_resolver.resolve());
    }
}
