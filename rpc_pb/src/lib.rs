#![doc = include_str!("../README.md")]

use std::collections::HashMap;

pub use handled::SError;
use one_two_eight::{generate_id, generate_id_prototk};
use prototk_derive::Message;

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
    type Err = SError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<String> = s.split('=').map(String::from).collect();
        if parts.len() != 2 {
            return Err(resolve_failure("could not parse string").with_debug_field("parts", parts));
        }
        let host_id: HostID = match parts[0].parse::<HostID>() {
            Ok(host_id) => host_id,
            Err(err) => {
                return Err(resolve_failure("could not parse HostID")
                    .with_debug_field("err", err)
                    .with_string_field("host_id", &parts[0]));
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
        write!(f, "{self:?}")
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

/////////////////////////////////////////////// Errors /////////////////////////////////////////////

const PHASE: &str = "rpc-pb";

/// Serialization or deserialization failed.
pub const CODE_SERIALIZATION_ERROR: &str = "serialization-error";
/// A request named an unknown server.
pub const CODE_UNKNOWN_SERVER_NAME: &str = "unknown-server-name";
/// A request named an unknown method.
pub const CODE_UNKNOWN_METHOD_NAME: &str = "unknown-method-name";
/// The request is larger than the RPC limit.
pub const CODE_REQUEST_TOO_LARGE: &str = "request-too-large";
/// The transport failed.
pub const CODE_TRANSPORT_FAILURE: &str = "transport-failure";
/// Encryption setup is invalid.
pub const CODE_ENCRYPTION_MISCONFIGURATION: &str = "encryption-misconfiguration";
/// Parsing the process file descriptor limit failed.
pub const CODE_ULIMIT_PARSE_ERROR: &str = "ulimit-parse-error";
/// An operating system error occurred.
pub const CODE_OS_ERROR: &str = "os-error";
/// The RPC implementation hit an internal logic error.
pub const CODE_LOGIC_ERROR: &str = "logic-error";
/// A requested object was not found.
pub const CODE_NOT_FOUND: &str = "not-found";
/// Host or route resolution failed.
pub const CODE_RESOLVE_FAILURE: &str = "resolve-failure";

fn error(code: &str) -> SError {
    SError::new(PHASE).with_code(code)
}

pub fn serialization_error(err: impl std::fmt::Debug, context: impl AsRef<str>) -> SError {
    error(CODE_SERIALIZATION_ERROR)
        .with_message("RPC serialization error")
        .with_debug_field("cause", err)
        .with_string_field("context", context.as_ref())
}

pub fn unknown_server_name(name: impl AsRef<str>) -> SError {
    error(CODE_UNKNOWN_SERVER_NAME)
        .with_message("unknown RPC server name")
        .with_string_field("name", name.as_ref())
}

pub fn unknown_method_name(name: impl AsRef<str>) -> SError {
    error(CODE_UNKNOWN_METHOD_NAME)
        .with_message("unknown RPC method name")
        .with_string_field("name", name.as_ref())
}

pub fn request_too_large(size: usize) -> SError {
    error(CODE_REQUEST_TOO_LARGE)
        .with_message("RPC request is too large")
        .with_atom_field("size", size)
        .with_atom_field("limit", MAX_REQUEST_SIZE)
}

pub fn transport_failure(what: impl AsRef<str>) -> SError {
    error(CODE_TRANSPORT_FAILURE)
        .with_message("RPC transport failure")
        .with_string_field("what", what.as_ref())
}

pub fn encryption_misconfiguration(what: impl AsRef<str>) -> SError {
    error(CODE_ENCRYPTION_MISCONFIGURATION)
        .with_message("RPC encryption misconfiguration")
        .with_string_field("what", what.as_ref())
}

pub fn ulimit_parse_error(what: impl AsRef<str>) -> SError {
    error(CODE_ULIMIT_PARSE_ERROR)
        .with_message("failed to parse process file descriptor limit")
        .with_string_field("what", what.as_ref())
}

pub fn os_error(err: impl ToString) -> SError {
    error(CODE_OS_ERROR)
        .with_message("RPC operating system error")
        .with_string_field("cause", &err.to_string())
}

pub fn logic_error(what: impl AsRef<str>) -> SError {
    error(CODE_LOGIC_ERROR)
        .with_message("RPC logic error")
        .with_string_field("what", what.as_ref())
}

pub fn not_found(what: impl AsRef<str>) -> SError {
    error(CODE_NOT_FOUND)
        .with_message("RPC object not found")
        .with_string_field("what", what.as_ref())
}

pub fn resolve_failure(what: impl AsRef<str>) -> SError {
    error(CODE_RESOLVE_FAILURE)
        .with_message("RPC resolution failure")
        .with_string_field("what", what.as_ref())
}

#[cfg(feature = "indicio")]
pub fn error_to_indicio(err: &SError) -> indicio::Value {
    indicio::value!({
        error: err.to_string(),
    })
}

////////////////////////////////////////////// Status //////////////////////////////////////////////

/// A status represents the return value of an RPC function.
///
/// At the outer-most level is a Result that captures RPC errors.
///
/// Inside the OK(_) branch of the outer-most level is a Result that switches over client requests.
/// They are either OK(_) with a body or Err(_) with a serialized error type.
pub type Status = Result<Result<Vec<u8>, Vec<u8>>, SError>;

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
                    Err($crate::unknown_method_name(method).into())
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
    fn resolve(&mut self) -> Result<Host, SError>;
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use buffertk::{Unpackable, stack_pack};

    use super::*;

    fn do_test(exp: SError) {
        let buf = stack_pack(&exp).to_vec();
        let got = SError::unpack(&buf).unwrap().0;
        assert_eq!(exp, got);
    }

    #[test]
    fn serialization_error_round_trips() {
        do_test(serialization_error(prototk::success(), "Some context"));
    }

    #[test]
    fn unknown_server_name_round_trips() {
        do_test(unknown_server_name("hostname"));
    }

    #[test]
    fn unknown_method_name_round_trips() {
        do_test(unknown_method_name("method"));
    }

    #[test]
    fn request_too_large_round_trips() {
        do_test(request_too_large(10));
    }

    #[test]
    fn transport_failure_round_trips() {
        do_test(transport_failure("socket closed"));
    }

    #[test]
    fn encryption_misconfiguration_round_trips() {
        do_test(encryption_misconfiguration("ssl misconfig"));
    }

    #[test]
    fn ulimit_parse_error() {
        do_test(super::ulimit_parse_error("could not read"));
    }

    #[test]
    fn os_error() {
        do_test(super::os_error("some I/O error"));
    }

    #[test]
    fn logic_error() {
        do_test(super::logic_error("some logic error"));
    }

    #[test]
    fn not_found_error() {
        do_test(not_found("deployment"));
    }
}
