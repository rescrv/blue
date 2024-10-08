#![doc = include_str!("../README.md")]

use std::collections::HashMap;
use std::fmt::Debug;

use one_two_eight::{generate_id, generate_id_prototk};
use prototk_derive::Message;
use zerror::{iotoz, Z};
use zerror_core::ErrorCore;

///////////////////////////////////////////// Constants ////////////////////////////////////////////

/// The maximum request size allowed.
pub const MAX_REQUEST_SIZE: usize = 1usize << 20;
/// The maximum response size allowed.
pub const MAX_RESPONSE_SIZE: usize = 1usize << 20;
/// The maximum body size.
pub const MAX_BODY_SIZE: usize = 1usize << 20;

////////////////////////////////////////////// TraceID /////////////////////////////////////////////

generate_id! {TraceID, "trace:"}
generate_id_prototk! {TraceID}

generate_id! {ClientID, "client:"}
generate_id_prototk! {ClientID}

generate_id! {HostID, "host:"}
generate_id_prototk! {HostID}

/////////////////////////////////////////////// Host ///////////////////////////////////////////////

/// A Host captures a process-unique, stable identifier with its connection string.
#[derive(Clone, Default, Eq, PartialEq, prototk_derive::Message)]
pub struct Host {
    #[prototk(1, message)]
    host_id: HostID,
    #[prototk(2, string)]
    connect: String,
}

impl Host {
    pub fn new(host_id: HostID, connect: String) -> Self {
        Self { host_id, connect }
    }

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

impl std::str::FromStr for Host {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<String> = s.split('=').map(String::from).collect();
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

impl std::fmt::Debug for Host {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}={}", self.host_id().human_readable(), self.connect())
    }
}

impl std::fmt::Display for Host {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{:?}", self)
    }
}

#[cfg(feature = "indicio")]
impl From<Host> for indicio::Value {
    fn from(host: Host) -> Self {
        indicio::value!({
            host_id: host.host_id().prefix_free_readable(),
            connect: host.connect(),
        })
    }
}

////////////////////////////////////////////// Context /////////////////////////////////////////////

/// A context passed by the RPC server into a service.
#[derive(Clone, Debug, Default)]
pub struct Context {
    clients: Vec<ClientID>,
    trace_id: Option<TraceID>,
}

impl Context {
    /// The list of clients chained to make this request.
    pub fn clients(&self) -> Vec<ClientID> {
        self.clients.clone()
    }

    /// Extend the context with an additional client.
    ///
    /// This makes a copy.
    pub fn with_client(&self, client: ClientID) -> Self {
        let mut ctx = self.clone();
        ctx.clients.push(client);
        ctx
    }

    /// The trace ID of the request.
    pub fn trace_id(&self) -> Option<TraceID> {
        self.trace_id
    }

    /// Extend the context with a trace ID.
    ///
    /// This make a copy.
    pub fn with_trace_id(&self, trace: TraceID) -> Self {
        let mut ctx = self.clone();
        ctx.trace_id = Some(trace);
        ctx
    }
}

impl<'a> From<&Request<'a>> for Context {
    fn from(req: &Request<'a>) -> Self {
        Self {
            clients: req.caller.clone(),
            trace_id: req.trace,
        }
    }
}

/////////////////////////////////////////////// Error //////////////////////////////////////////////

/// RPC Error.
#[derive(Clone, Message, zerror_derive::Z)]
pub enum Error {
    /// The default error type.  Necessary to support protobuf, but should otherwise not be
    /// constructed.
    #[prototk(278528, message)]
    Success {
        /// The error core.
        #[prototk(1, message)]
        core: ErrorCore,
    },
    /// An error was encountered while serializing or deserializing data.
    #[prototk(278529, message)]
    SerializationError {
        /// The error core.
        #[prototk(1, message)]
        core: ErrorCore,
        /// The error that was encountered.
        #[prototk(2, message)]
        err: prototk::Error,
        /// Additional context for what was happening.
        #[prototk(3, string)]
        context: String,
    },
    /// The request asks for an unknown server.
    #[prototk(278530, message)]
    UnknownServerName {
        /// The error core.
        #[prototk(1, message)]
        core: ErrorCore,
        /// The server name requested.
        #[prototk(2, string)]
        name: String,
    },
    /// The request asks for an unknown method.
    #[prototk(278531, message)]
    UnknownMethodName {
        /// The error core.
        #[prototk(1, message)]
        core: ErrorCore,
        /// The method name requested.
        #[prototk(2, string)]
        name: String,
    },
    /// The request exceeds the allowable size.
    #[prototk(278532, message)]
    RequestTooLarge {
        /// The error core.
        #[prototk(1, message)]
        core: ErrorCore,
        /// the size requested.
        #[prototk(2, uint64)]
        size: u64,
    },
    /// There was an error at the transport layer.
    #[prototk(278533, message)]
    TransportFailure {
        /// The error core.
        #[prototk(1, message)]
        core: ErrorCore,
        /// The string representation of the error.
        #[prototk(2, string)]
        what: String,
    },
    /// Encryption is misconfigured.
    #[prototk(278534, message)]
    EncryptionMisconfiguration {
        /// The error core.
        #[prototk(1, message)]
        core: ErrorCore,
        /// A hint as to what went wrong.
        #[prototk(2, string)]
        what: String,
    },
    /// It wasn't possible to probe ulimit -n.
    #[prototk(278535, message)]
    UlimitParseError {
        /// The error core.
        #[prototk(1, message)]
        core: ErrorCore,
        /// A hint as to what went wrong.
        #[prototk(2, string)]
        what: String,
    },
    /// An OS/IO error.
    #[prototk(278536, message)]
    OsError {
        /// The error core.
        #[prototk(1, message)]
        core: ErrorCore,
        /// The string representation of the error.
        #[prototk(2, string)]
        what: String,
    },
    /// A logic error in the RPC implementation.
    #[prototk(278537, message)]
    LogicError {
        /// The error core.
        #[prototk(1, message)]
        core: ErrorCore,
        /// A hint as to what went wrong.
        #[prototk(2, string)]
        what: String,
    },
    /// Service discovery failed to find the host.
    #[prototk(278538, message)]
    NotFound {
        /// The error core.
        #[prototk(1, message)]
        core: ErrorCore,
        /// A hint as to what went wrong.
        #[prototk(2, string)]
        what: String,
    },
    /// Resolution failure.
    #[prototk(278539, message)]
    ResolveFailure {
        /// The error core.
        #[prototk(1, message)]
        core: ErrorCore,
        /// A hint as to what went wrong.
        #[prototk(2, string)]
        what: String,
    },
}

impl Error {
    pub fn resolve_failure(what: impl Into<String>) -> Self {
        Self::ResolveFailure {
            core: ErrorCore::default(),
            what: what.into(),
        }
    }
}

impl Default for Error {
    fn default() -> Error {
        Error::Success {
            core: ErrorCore::default(),
        }
    }
}

impl From<buffertk::Error> for Error {
    fn from(err: buffertk::Error) -> Error {
        Error::SerializationError {
            core: ErrorCore::default(),
            err: err.into(),
            context: "buffertk unpack error".to_string(),
        }
    }
}

impl From<prototk::Error> for Error {
    fn from(err: prototk::Error) -> Error {
        Error::SerializationError {
            core: ErrorCore::default(),
            err,
            context: "prototk unpack error".to_string(),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error::OsError {
            core: ErrorCore::default(),
            what: format!("{}", err),
        }
    }
}

iotoz! {Error}

#[cfg(feature = "indicio")]
impl From<Error> for indicio::Value {
    fn from(err: Error) -> Self {
        match err {
            Error::Success { core: _ } => {
                indicio::value!({
                    success: true,
                })
            }
            Error::SerializationError {
                core: _,
                err,
                context,
            } => {
                indicio::value!({
                    serialization: {
                        // TODO(rescrv): implement indicio::Value for prototk::Error;
                        what: format!("{:?}", err),
                        context: context,
                    },
                })
            }
            Error::UnknownServerName { core: _, name } => {
                indicio::value!({
                    unknown_server: name,
                })
            }
            Error::UnknownMethodName { core: _, name } => {
                indicio::value!({
                    unknown_method: name,
                })
            }
            Error::RequestTooLarge { core: _, size } => {
                indicio::value!({
                    request_too_large: {
                        size: size,
                        limit: MAX_REQUEST_SIZE,
                    },
                })
            }
            Error::TransportFailure { core: _, what } => {
                indicio::value!({
                    transport_failure: what,
                })
            }
            Error::EncryptionMisconfiguration { core: _, what } => {
                indicio::value!({
                    encryption_misconfiguration: what,
                })
            }
            Error::UlimitParseError { core: _, what } => {
                indicio::value!({
                    ulimit_parse_error: what,
                })
            }
            Error::OsError { core: _, what } => {
                indicio::value!({
                    os_error: what,
                })
            }
            Error::LogicError { core: _, what } => {
                indicio::value!({
                    logic_error: what,
                })
            }
            Error::NotFound { core: _, what } => {
                indicio::value!({
                    not_found: what,
                })
            }
            Error::ResolveFailure { core: _, what } => {
                indicio::value!({
                    resolve_failure: what,
                })
            }
        }
    }
}

////////////////////////////////////////////// Status //////////////////////////////////////////////

/// A status represents the return value of an RPC function.
///
/// At the outer-most level is a Result that captures RPC errors.
///
/// Inside the OK(_) branch of the outer-most level is a Result that switches over client requests.
/// They are either OK(_) with a body or Err(_) with a serialized error type.
pub type Status = Result<Result<Vec<u8>, Vec<u8>>, Error>;

////////////////////////////////////////////// Server //////////////////////////////////////////////

/// A Server is an object that can be called.
///
/// An RPC server will wrap many Server objects and dispatch calls to them appropriately.
pub trait Server {
    /// Call the server.
    fn call(&self, ctx: &Context, method: &str, req: &[u8]) -> Status;
}

////////////////////////////////////////////// Client //////////////////////////////////////////////

/// A Client is an object that can be called.
///
/// Many Client will wrap a single RPC client to make remote calls.
pub trait Client {
    /// Call the client.
    fn call(&self, ctx: &Context, server: &str, method: &str, req: &[u8]) -> Status;
}

/////////////////////////////////////////////// Frame //////////////////////////////////////////////

/// Messages on the wire are preceded by frames.  Frames are framed by a solitary varint.
#[derive(Clone, Debug, Default, Message)]
pub struct Frame {
    /// The size of the message this frame represents.
    #[prototk(1, uint64)]
    pub size: u64,
    /// The crc32c of this frame.
    #[prototk(2, fixed32)]
    pub crc32c: u32,
}

impl Frame {
    /// Given a buffer, create the frame that should precede it on the wire.
    pub fn from_buffer(buf: &[u8]) -> Self {
        Self {
            size: buf.len() as u64,
            crc32c: crc32c::crc32c(buf),
        }
    }
}

////////////////////////////////////////////// Request /////////////////////////////////////////////

/// A request to a server.
#[derive(Clone, Debug, Default, Message)]
pub struct Request<'a> {
    /// The service this request is intended for.
    #[prototk(1, string)]
    pub service: &'a str,
    /// The method this request intends to call.
    #[prototk(2, string)]
    pub method: &'a str,
    /// A client-provided sequence number used to match requests to responses.
    #[prototk(3, uint64)]
    pub seq_no: u64,
    /// The body of the request.
    #[prototk(4, bytes)]
    pub body: &'a [u8],
    /// A chain of callers.
    #[prototk(5, message)]
    pub caller: Vec<ClientID>,
    /// The trace ID for this request.
    #[prototk(6, message)]
    pub trace: Option<TraceID>,
}

///////////////////////////////////////////// Response /////////////////////////////////////////////

/// A response to a client.
#[derive(Clone, Debug, Default, Message)]
pub struct Response<'a> {
    /// The sequence number provided in the [Request].
    #[prototk(3, uint64)]
    pub seq_no: u64,
    /// The trace ID for this request.  Can be Some, even if Request was None.
    #[prototk(6, message)]
    pub trace: Option<TraceID>,
    /// The body of the response.
    #[prototk(7, bytes)]
    pub body: Option<&'a [u8]>,
    /// The error at service level.
    #[prototk(8, bytes)]
    pub service_error: Option<&'a [u8]>,
    /// The error at the RPC level.
    #[prototk(9, bytes)]
    pub rpc_error: Option<&'a [u8]>,
}

///////////////////////////////////////////// The Macro ////////////////////////////////////////////

/// Create typed server/client methods.
#[macro_export]
macro_rules! service {
    (name = $service:ident; server = $server:ident; client = $client:ident; error = $error:ty; $(rpc $method:ident ($req:ty) -> $resp:ty;)*) => {
        /// A typed RPC service.
        pub trait $service: Send + Sync + 'static {
            $(
                /// Auto-generated service method generated by service!.
                fn $method(&self, ctx: &$crate::Context, req: $req) -> Result<$resp, $error>;
            )*
        }

        /// A typed RPC client.
        pub struct $client {
            client: std::sync::Arc<dyn $crate::Client + Send + Sync + 'static>,
        }

        impl $client where {
            /// Create a new client.
            pub fn new(client: std::sync::Arc<dyn $crate::Client + Send + Sync + 'static>) -> Self {
                Self {
                    client,
                }
            }
        }

        impl $service for $client where
            $client: Send + Sync + 'static
        {
            $(
                $crate::client_method! { $service, $method, $req, $resp, $error }
            )*
        }

        /// A typed RPC server.
        pub struct $server<S: $service> {
            server: S,
        }

        impl<S: $service> $server<S> {
            /// Bind the provided server.
            pub fn bind(server: S) -> $server<S> {
                $server {
                    server,
                }
            }
        }

        impl<S: $service> $crate::Server for $server<S> {
            $crate::server_methods! { $service, $error, $($method, $req, $resp),* }
        }
    };
}

/// Create a client method.
#[macro_export]
macro_rules! client_method {
    ($service:ident, $method:ident, $req:ty, $resp:ty, $error:ty) => {
        /// Auto-generated method generated by client_method!.
        fn $method(&self, ctx: &$crate::Context, req: $req) -> Result<$resp, $error> {
            let req = ::buffertk::stack_pack(req).to_vec();
            let status = self
                .client
                .call(ctx, stringify!($service), stringify!($method), &req);
            match status {
                Ok(Ok(msg)) => Ok(<$resp as ::buffertk::Unpackable>::unpack(&msg)?.0),
                Ok(Err(msg)) => Err(<$error as ::buffertk::Unpackable>::unpack(&msg)?.0),
                Err(err) => Err(err.into()),
            }
        }
    };
}

/// Generate the server `call` method.
#[macro_export]
macro_rules! server_methods {
    ($service:ident, $error:ty, $($method:ident, $req:ty, $resp:ty),*) => {
        /// Auto-generated method generated by server_methods!.
        fn call(&self, ctx: &$crate::Context, method: &str, req: &[u8]) -> $crate::Status {
            use buffertk::stack_pack;
            match method {
                $(
                stringify!($method) => {
                    let req = <$req as ::buffertk::Unpackable>::unpack(req)?.0;
                    let ans: Result<$resp, $error> = self.server.$method(ctx, req);
                    match ans {
                        Ok(resp) => {
                            Ok(Ok(stack_pack(resp).to_vec()))
                        }
                        Err(err) => {
                            Ok(Err(stack_pack(err).to_vec()))
                        }
                    }
                }
                ),*
                _ => {
                    Err($crate::Error::UnknownMethodName {
                        core: zerror_core::ErrorCore::default(),
                        name: method.to_string(),
                    }.into())
                },
            }
        }
    };
}

////////////////////////////////////////// ServerRegistry //////////////////////////////////////////

/// A ServerRegistry multiplexes servers to dispatch calls by server name.
pub struct ServerRegistry {
    registry: HashMap<&'static str, Box<dyn Server>>,
}

impl ServerRegistry {
    /// Register the provided server with this ServerRegistry.
    pub fn register<S: Server + 'static>(&mut self, name: &'static str, server: S) {
        assert!(!self.registry.contains_key(name));
        self.registry.insert(name, Box::new(server));
    }

    /// Get the server registered with `name`.
    pub fn get_server(&self, name: &str) -> Option<&dyn Server> {
        self.registry.get(name).map(|x| x.as_ref())
    }
}

///////////////////////////////////////////// Resolver /////////////////////////////////////////////

/// A trait for resolving hosts.
pub trait Resolver {
    /// Resolve one Host.
    fn resolve(&mut self) -> Result<Host, Error>;
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use buffertk::{stack_pack, Unpackable};

    use super::*;

    fn do_test(s: &str, exp: Error) {
        assert_eq!(s, exp.to_string());
        let buf = stack_pack(&exp).to_vec();
        let got = Error::unpack(&buf).unwrap().0;
        assert_eq!(exp, got);
    }

    #[test]
    fn success() {
        do_test(
            "Success",
            Error::Success {
                core: ErrorCore::default(),
            },
        );
    }

    #[test]
    fn serialization_error() {
        do_test(
            "SerializationError { err: Success, context: \"Some context\" }",
            Error::SerializationError {
                core: ErrorCore::default(),
                context: "Some context".to_owned(),
                err: prototk::Error::Success,
            },
        );
    }

    #[test]
    fn unknown_server_name() {
        do_test(
            "UnknownServerName { name: \"hostname\" }",
            Error::UnknownServerName {
                core: ErrorCore::default(),
                name: "hostname".to_owned(),
            },
        );
    }

    #[test]
    fn unknown_method_name() {
        do_test(
            "UnknownMethodName { name: \"method\" }",
            Error::UnknownMethodName {
                core: ErrorCore::default(),
                name: "method".to_owned(),
            },
        );
    }

    #[test]
    fn request_too_large() {
        do_test(
            "RequestTooLarge { size: 10 }",
            Error::RequestTooLarge {
                core: ErrorCore::default(),
                size: 10,
            },
        );
    }

    #[test]
    fn transport_failure() {
        do_test(
            "TransportFailure { what: \"socket closed\" }",
            Error::TransportFailure {
                core: ErrorCore::default(),
                what: "socket closed".to_owned(),
            },
        );
    }

    #[test]
    fn encryption_misconfiguration() {
        do_test(
            "EncryptionMisconfiguration { what: \"ssl misconfig\" }",
            Error::EncryptionMisconfiguration {
                core: ErrorCore::default(),
                what: "ssl misconfig".to_owned(),
            },
        );
    }

    #[test]
    fn ulimit_parse_error() {
        do_test(
            "UlimitParseError { what: \"could not read\" }",
            Error::UlimitParseError {
                core: ErrorCore::default(),
                what: "could not read".to_owned(),
            },
        );
    }

    #[test]
    fn os_error() {
        do_test(
            "OsError { what: \"some I/O error\" }",
            Error::OsError {
                core: ErrorCore::default(),
                what: "some I/O error".to_owned(),
            },
        );
    }

    #[test]
    fn logic_error() {
        do_test(
            "LogicError { what: \"some logic error\" }",
            Error::LogicError {
                core: ErrorCore::default(),
                what: "some logic error".to_owned(),
            },
        );
    }

    #[test]
    fn not_found_error() {
        do_test(
            "NotFound { what: \"deployment\" }",
            Error::NotFound {
                core: ErrorCore::default(),
                what: "deployment".to_owned(),
            },
        );
    }
}
