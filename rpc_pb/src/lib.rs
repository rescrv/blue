use std::fmt::{Debug, Display, Formatter};

use buffertk::Buffer;

use biometrics::Counter;

use one_two_eight::{generate_id, generate_id_prototk};

use prototk_derive::Message;

use zerror:: Z;

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
            Error::Success { core, .. } => { core },
            Error::SerializationError { core, .. } => { core } ,
            Error::UnknownServerName { core, .. } => { core } ,
            Error::UnknownMethodName { core, .. } => { core } ,
            Error::RequestTooLarge { core, .. } => { core } ,
            Error::TransportFailure { core, .. } => { core } ,
            Error::EncryptionMisconfiguration { core, .. } => { core } ,
            Error::UlimitParseError { core, .. } => { core } ,
            Error::OsError { core, .. } => { core } ,
            Error::LogicError { core, .. } => { core } ,
        }
    }

    fn core_mut(&mut self) -> &mut ErrorCore {
        match self {
            Error::Success { core, .. } => { core },
            Error::SerializationError { core, .. } => { core } ,
            Error::UnknownServerName { core, .. } => { core } ,
            Error::UnknownMethodName { core, .. } => { core } ,
            Error::RequestTooLarge { core, .. } => { core } ,
            Error::TransportFailure { core, .. } => { core } ,
            Error::EncryptionMisconfiguration { core, .. } => { core } ,
            Error::UlimitParseError { core, .. } => { core } ,
            Error::OsError { core, .. } => { core } ,
            Error::LogicError { core, .. } => { core } ,
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
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        match self {
            Error::Success { core: _ } => {
                write!(f, "success or default error")
            },
            Error::SerializationError { core: _, err, context } => {
                write!(f, "serialization error in {}: {}", context, err)
            },
            Error::UnknownServerName { core: _, name } => {
                write!(f, "unknown server name {}", name)
            },
            Error::UnknownMethodName { core: _, name } => {
                write!(f, "unknown method {}", name)
            },
            Error::RequestTooLarge { core: _, size } => {
                write!(f, "request too large: {} bytes", size)
            },
            Error::TransportFailure { core: _, what } => {
                write!(f, "transport failure: {}", what)
            }
            Error::EncryptionMisconfiguration { core: _, what } => {
                write!(f, "encyrption misconfiguration: {}", what)
            }
            Error::UlimitParseError { core: _, what } => {
                write!(f, "ulimit parse failure: {}", what)
            }
            Error::OsError { core: _, what } => {
                write!(f, "os error: {}", what)
            }
            Error::LogicError { core: _, what } => {
                write!(f, "logic error: {}", what)
            }
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

impl Z for Error {
    type Error = Self;

    fn long_form(&self) -> String {
        format!("{}\n", self) + &self.core().long_form()
    }

    fn with_token(mut self, identifier: &str, value: &str) -> Self::Error {
        self.set_token(identifier, value);
        self
    }

    fn set_token(&mut self, identifier: &str, value: &str) {
        self.core_mut().set_token(identifier, value);
    }

    fn with_url(mut self, identifier: &str, url: &str) -> Self::Error {
        self.set_url(identifier, url);
        self
    }

    fn set_url(&mut self, identifier: &str, url: &str) {
        self.core_mut().set_url(identifier, url);
    }

    fn with_variable<X: Debug>(mut self, variable: &str, x: X) -> Self::Error where X: Debug {
        self.set_variable(variable, x);
        self
    }

    fn set_variable<X: Debug>(&mut self, variable: &str, x: X) {
        self.core_mut().set_variable(variable, x);
    }
}

////////////////////////////////////////////// Status //////////////////////////////////////////////

pub type Status = Result<Result<Buffer, Buffer>, Error>;

////////////////////////////////////////////// Service /////////////////////////////////////////////

pub trait Service {
}

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
    pub server: &'a str,
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
            let status = self.client.call(ctx, stringify!($service), stringify!($method), &req);
            match status {
                Ok(Ok(msg)) => {
                    Ok(<$resp as Unpackable>::unpack(msg.as_bytes())?.0)
                },
                Ok(Err(msg)) => {
                    Err(<$error as Unpackable>::unpack(msg.as_bytes())?.0)
                },
                Err(err) => {
                    Err(err.into())
                },
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
                            Ok(Ok(stack_pack(resp).to_buffer()))
                        }
                        Err(err) => {
                            Ok(Err(stack_pack(err).to_buffer()))
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
