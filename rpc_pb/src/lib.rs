use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};

use biometrics::Counter;

use one_two_eight::{generate_id, generate_id_prototk};

use prototk_derive::Message;

use zerror::{iotoz, Z};

use zerror_core::ErrorCore;

///////////////////////////////////////////// Constants ////////////////////////////////////////////

pub const MAX_REQUEST_SIZE: usize = 1usize << 20;
pub const MAX_RESPONSE_SIZE: usize = 1usize << 20;

//////////////////////////////////////////// Biometrics ////////////////////////////////////////////

pub static UNUSED_BUFFER: Counter = Counter::new("rpc_pb.unused_buffer");

////////////////////////////////////////////// TraceID /////////////////////////////////////////////

generate_id! {TraceID, "trace:"}
generate_id_prototk! {TraceID}

generate_id! {ClientID, "client:"}
generate_id_prototk! {ClientID}

////////////////////////////////////////////// Context /////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct Context {
    clients: Vec<ClientID>,
    trace_id: Option<TraceID>,
}

impl Context {
    pub fn clients(&self) -> Vec<ClientID> {
        self.clients.clone()
    }

    pub fn with_client(&self, client: ClientID) -> Self {
        let mut ctx = self.clone();
        ctx.clients.push(client);
        ctx
    }

    pub fn trace_id(&self) -> Option<TraceID> {
        self.trace_id
    }

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

#[derive(Clone, Debug, Message)]
pub enum Error {
    #[prototk(278528, message)]
    Success {
        #[prototk(1, message)]
        core: ErrorCore,
    },
    #[prototk(278529, message)]
    SerializationError {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, message)]
        err: prototk::Error,
        #[prototk(3, string)]
        context: String,
    },
    #[prototk(278530, message)]
    UnknownServerName {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        name: String,
    },
    #[prototk(278531, message)]
    UnknownMethodName {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        name: String,
    },
    #[prototk(278532, message)]
    RequestTooLarge {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, uint64)]
        size: u64,
    },
    #[prototk(278533, message)]
    TransportFailure {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        what: String,
    },
    #[prototk(278534, message)]
    EncryptionMisconfiguration {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        what: String,
    },
    #[prototk(278535, message)]
    UlimitParseError {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        what: String,
    },
    #[prototk(278536, message)]
    OsError {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        what: String,
    },
    #[prototk(278537, message)]
    LogicError {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        what: String,
    },
}

impl Error {
    fn core(&self) -> &ErrorCore {
        match self {
            Error::Success { core, .. } => core,
            Error::SerializationError { core, .. } => core,
            Error::UnknownServerName { core, .. } => core,
            Error::UnknownMethodName { core, .. } => core,
            Error::RequestTooLarge { core, .. } => core,
            Error::TransportFailure { core, .. } => core,
            Error::EncryptionMisconfiguration { core, .. } => core,
            Error::UlimitParseError { core, .. } => core,
            Error::OsError { core, .. } => core,
            Error::LogicError { core, .. } => core,
        }
    }

    fn core_mut(&mut self) -> &mut ErrorCore {
        match self {
            Error::Success { core, .. } => core,
            Error::SerializationError { core, .. } => core,
            Error::UnknownServerName { core, .. } => core,
            Error::UnknownMethodName { core, .. } => core,
            Error::RequestTooLarge { core, .. } => core,
            Error::TransportFailure { core, .. } => core,
            Error::EncryptionMisconfiguration { core, .. } => core,
            Error::UlimitParseError { core, .. } => core,
            Error::OsError { core, .. } => core,
            Error::LogicError { core, .. } => core,
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

impl Display for Error {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), std::fmt::Error> {
        match self {
            Error::Success { core: _ } => fmt.debug_struct("Success").finish(),
            Error::SerializationError {
                core: _,
                err,
                context,
            } => fmt
                .debug_struct("SerializationError")
                .field("context", context)
                .field("err", err)
                .finish(),
            Error::UnknownServerName { core: _, name } => fmt
                .debug_struct("UnknownServerName")
                .field("name", name)
                .finish(),
            Error::UnknownMethodName { core: _, name } => fmt
                .debug_struct("UnknownMethodName")
                .field("name", name)
                .finish(),
            Error::RequestTooLarge { core: _, size } => fmt
                .debug_struct("RequestTooLarge")
                .field("size", size)
                .finish(),
            Error::TransportFailure { core: _, what } => fmt
                .debug_struct("TransportFailure")
                .field("what", what)
                .finish(),
            Error::EncryptionMisconfiguration { core: _, what } => fmt
                .debug_struct("EncryptionMisconfiguration")
                .field("what", what)
                .finish(),
            Error::UlimitParseError { core: _, what } => fmt
                .debug_struct("UlimitParseError")
                .field("what", what)
                .finish(),
            Error::OsError { core: _, what } => fmt
                .debug_struct("OsError")
                .field("what", what)
                .finish(),
            Error::LogicError { core: _, what } => fmt
                .debug_struct("LogicError")
                .field("what", what)
                .finish(),
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

impl PartialEq for Error {
    fn eq(&self, other: &Error) -> bool {
        match (self, other) {
            (Error::Success { core: _ }, Error::Success { core: _ }) => { true }
            (Error::SerializationError { core: _, err: err_lhs, context: context_lhs },
             Error::SerializationError { core: _, err: err_rhs, context: context_rhs }) => {
                err_lhs == err_rhs && context_lhs == context_rhs
            },
            (Error::UnknownServerName { core: _, name: name_lhs },
             Error::UnknownServerName { core: _, name: name_rhs }) => {
                name_lhs == name_rhs
            },
            (Error::UnknownMethodName { core: _, name: name_lhs },
             Error::UnknownMethodName { core: _, name: name_rhs }) => {
                name_lhs == name_rhs
            },
            (Error::RequestTooLarge { core: _, size: size_lhs },
             Error::RequestTooLarge { core: _, size: size_rhs }) => {
                size_lhs == size_rhs
            },
            (Error::TransportFailure { core: _, what: what_lhs },
             Error::TransportFailure { core: _, what: what_rhs }) => {
                what_lhs == what_rhs
            },
            (Error::EncryptionMisconfiguration { core: _, what: what_lhs },
             Error::EncryptionMisconfiguration { core: _, what: what_rhs }) => {
                what_lhs == what_rhs
            },
            (Error::UlimitParseError { core: _, what: what_lhs },
             Error::UlimitParseError { core: _, what: what_rhs }) => {
                what_lhs == what_rhs
            },
            (Error::OsError { core: _, what: what_lhs },
             Error::OsError { core: _, what: what_rhs }) => {
                what_lhs == what_rhs
            },
            (Error::LogicError { core: _, what: what_lhs },
             Error::LogicError { core: _, what: what_rhs }) => {
                what_lhs == what_rhs
            },
            (_, _) => { false }
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

impl Z for Error {
    type Error = Self;

    fn long_form(&self) -> String {
        format!("{}\n", self) + &self.core().long_form()
    }

    fn with_token(mut self, identifier: &str, value: &str) -> Self::Error {
        self.core_mut().set_token(identifier, value);
        self
    }

    fn with_url(mut self, identifier: &str, url: &str) -> Self::Error {
        self.core_mut().set_url(identifier, url);
        self
    }

    fn with_variable<X: Debug>(mut self, variable: &str, x: X) -> Self::Error
    where
        X: Debug,
    {
        self.core_mut().set_variable(variable, x);
        self
    }
}

iotoz! {Error}

////////////////////////////////////////////// Status //////////////////////////////////////////////

pub type Status = Result<Result<Vec<u8>, Vec<u8>>, Error>;

////////////////////////////////////////////// Service /////////////////////////////////////////////

pub trait Service {}

////////////////////////////////////////////// Server //////////////////////////////////////////////

pub trait Server {
    fn call(&self, ctx: &Context, method: &str, req: &[u8]) -> Status;
}

////////////////////////////////////////////// Client //////////////////////////////////////////////

pub trait Client {
    fn call(&self, ctx: &Context, server: &str, method: &str, req: &[u8]) -> Status;
}

/////////////////////////////////////////////// Frame //////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
pub struct Frame {
    #[prototk(1, uint64)]
    pub size: u64,
    #[prototk(2, fixed32)]
    pub crc32c: u32,
}

////////////////////////////////////////////// Request /////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
pub struct Request<'a> {
    #[prototk(1, string)]
    pub service: &'a str,
    #[prototk(2, string)]
    pub method: &'a str,
    #[prototk(3, uint64)]
    pub seq_no: u64,
    #[prototk(4, bytes)]
    pub body: &'a [u8],
    #[prototk(5, message)]
    pub caller: Vec<ClientID>,
    #[prototk(6, message)]
    pub trace: Option<TraceID>,
}

///////////////////////////////////////////// Response /////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
pub struct Response<'a> {
    #[prototk(3, uint64)]
    pub seq_no: u64,
    #[prototk(6, message)]
    pub trace: Option<TraceID>,
    #[prototk(7, bytes)]
    pub body: Option<&'a [u8]>,
    #[prototk(8, bytes)]
    pub service_error: Option<&'a [u8]>,
    #[prototk(9, bytes)]
    pub rpc_error: Option<&'a [u8]>,
}

///////////////////////////////////////////// The Macro ////////////////////////////////////////////

#[macro_export]
macro_rules! service {
    (name = $service:ident; server = $server:ident; client = $client:ident; error = $error:ty; $(rpc $method:ident ($req:ident) -> $resp:ident;)+) => {
        pub trait $service {
            $(
                fn $method(&self, ctx: &rpc_pb::Context, req: $req) -> Result<$resp, $error>;
            )*
        }

        pub struct $client<C: rpc_pb::Client> {
            client: C,
        }

        impl<C: rpc_pb::Client> $service for $client<C> where {
            $(
                rpc_pb::client_method! { $service, $method, $req, $resp, $error }
            )*
        }

        pub struct $server<S: $service> {
            server: S,
        }

        impl<S: $service> $server<S> {
            pub fn bind(server: S) -> $server<S> {
                $server {
                    server,
                }
            }
        }

        impl<S: $service> rpc_pb::Server for $server<S> {
            rpc_pb::server_methods! { $service, $error, $($method, $req, $resp),* }
        }
    };
}

#[macro_export]
macro_rules! client_method {
    ($service:ident, $method:ident, $req:ident, $resp:ty, $error:ty) => {
        fn $method(&self, ctx: &rpc_pb::Context, req: $req) -> Result<$resp, $error> {
            use buffertk::{stack_pack, Packable, Unpackable};
            let req = stack_pack(req).to_vec();
            let status = self
                .client
                .call(ctx, stringify!($service), stringify!($method), &req);
            match status {
                Ok(Ok(msg)) => Ok(<$resp as Unpackable>::unpack(&msg)?.0),
                Ok(Err(msg)) => Err(<$error as Unpackable>::unpack(&msg)?.0),
                Err(err) => Err(err.into()),
            }
        }
    };
}

#[macro_export]
macro_rules! server_methods {
    ($service:ident, $error:ty, $($method:ident, $req:ident, $resp:ident),+) => {
        fn call(&self, ctx: &rpc_pb::Context, method: &str, req: &[u8]) -> rpc_pb::Status {
            use buffertk::{stack_pack, Packable, Unpackable};
            match method {
                $(
                stringify!($method) => {
                    let req = <$req as buffertk::Unpackable>::unpack(req)?.0;
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
                    Err(rpc_pb::Error::UnknownMethodName {
                        core: zerror_core::ErrorCore::default(),
                        name: method.to_string(),
                    }.into())
                },
            }
        }
    };
}

////////////////////////////////////////// ServerRegistry //////////////////////////////////////////

pub struct ServerRegistry {
    registry: HashMap<&'static str, Box<dyn Server>>,
}

impl ServerRegistry {
    pub fn register(&mut self, name: &'static str, server: Box<dyn Server>) {
        assert!(!self.registry.contains_key(name));
        self.registry.insert(name, server);
    }

    pub fn get_server(&self, name: &str) -> Option<&dyn Server> {
        self.registry.get(name).map(|x| x.as_ref())
    }
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
            }
        );
    }

    #[test]
    fn serialization_error() {
        do_test(
            "SerializationError { context: \"Some context\", err: Success }",
            Error::SerializationError {
                core: ErrorCore::default(),
                context: "Some context".to_owned(),
                err: prototk::Error::Success,
            }
        );
    }

    #[test]
    fn unknown_server_name() {
        do_test(
            "UnknownServerName { name: \"hostname\" }",
            Error::UnknownServerName {
                core: ErrorCore::default(),
                name: "hostname".to_owned(),
            }
        );
    }

    #[test]
    fn unknown_method_name() {
        do_test(
            "UnknownMethodName { name: \"method\" }",
            Error::UnknownMethodName {
                core: ErrorCore::default(),
                name: "method".to_owned(),
            }
        );
    }

    #[test]
    fn request_too_large() {
        do_test(
            "RequestTooLarge { size: 10 }",
            Error::RequestTooLarge {
                core: ErrorCore::default(),
                size: 10,
            }
        );
    }

    #[test]
    fn transport_failure() {
        do_test(
            "TransportFailure { what: \"socket closed\" }",
            Error::TransportFailure {
                core: ErrorCore::default(),
                what: "socket closed".to_owned(),
            }
        );
    }

    #[test]
    fn encryption_misconfiguration() {
        do_test(
            "EncryptionMisconfiguration { what: \"ssl misconfig\" }",
            Error::EncryptionMisconfiguration {
                core: ErrorCore::default(),
                what: "ssl misconfig".to_owned(),
            }
        );
    }

    #[test]
    fn ulimit_parse_error() {
        do_test(
            "UlimitParseError { what: \"could not read\" }",
            Error::UlimitParseError {
                core: ErrorCore::default(),
                what: "could not read".to_owned(),
            }
        );
    }

    #[test]
    fn os_error() {
        do_test(
            "OsError { what: \"some I/O error\" }",
            Error::OsError {
                core: ErrorCore::default(),
                what: "some I/O error".to_owned(),
            }
        );
    }

    #[test]
    fn logic_error() {
        do_test(
            "LogicError { what: \"some logic error\" }",
            Error::LogicError {
                core: ErrorCore::default(),
                what: "some logic error".to_owned(),
            }
        );
    }
}
