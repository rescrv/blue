#[macro_use]
extern crate prototk_derive;

use std::fmt::{Debug, Display, Formatter};

use buffertk::Buffer;

use zerror:: Z;

use error_core::ErrorCore;

////////////////////////////////////////////// TraceID /////////////////////////////////////////////

id::generate_id! {TraceID, "trace:"}
id::generate_id_prototk! {TraceID}

////////////////////////////////////////////// Context /////////////////////////////////////////////

pub struct Context {}

////////////////////////////////////////// ResponseHolder //////////////////////////////////////////

pub trait ResponseHolder {}

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Debug, Message)]
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
    // TODO(rescrv):  This seems like a catch-all.  Make a better one.
    #[prototk(278533, message)]
    Errno {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, int32)]
        errno: i32,
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
            Error::Errno { core, .. } => { core } ,
        }
    }

    fn core_mut(&mut self) -> &mut ErrorCore {
        match self {
            Error::Success { core, .. } => { core },
            Error::SerializationError { core, .. } => { core } ,
            Error::UnknownServerName { core, .. } => { core } ,
            Error::UnknownMethodName { core, .. } => { core } ,
            Error::RequestTooLarge { core, .. } => { core } ,
            Error::Errno { core, .. } => { core } ,
        }
    }
}

impl Error {
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
            Error::Errno { core: _, errno } => {
                write!(f, "errno {}", errno)
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

////////////////////////////////////////////// Service /////////////////////////////////////////////

pub trait Service {
}

////////////////////////////////////////////// Server //////////////////////////////////////////////

pub trait Server {
    fn call(&self, ctx: &Context, method: &str, req: &[u8]) -> Result<Buffer, Error>;
}

////////////////////////////////////////////// Client //////////////////////////////////////////////

pub trait Client {
    fn call(&self, ctx: &Context, server: &str, method: &str, req: &[u8]) -> Result<Buffer, Error>;
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
    #[prototk(5, uint64)]
    pub issued_ms: u64,
    #[prototk(6, uint64)]
    pub expires_ms: u64,
    #[prototk(7, string)]
    pub caller: &'a str,
    #[prototk(8, message)]
    pub trace: Option<TraceID>,
}

///////////////////////////////////////////// Response /////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
pub struct Response<'a> {
    #[prototk(3, uint64)]
    pub seq_no: u64,
    #[prototk(4, bytes)]
    pub body: &'a [u8],
}

///////////////////////////////////////////// The Macro ////////////////////////////////////////////

#[macro_export]
macro_rules! service {
    (name = $service:ident; server = $server:ident; client = $client:ident; $(rpc $method:ident ($req:ident) -> $resp:ident;)+) => {
        trait $service {
            $(
                fn $method(&self, ctx: &rpc::Context, req: $req) -> Result<$resp, Error>;
            )*
        }

        struct $client<C: rpc::Client> {
            client: C,
        }

        impl<C: rpc::Client> $service for $client<C> {
            $(
                rpc::client_method! { $service, $method, $req, $resp }
            )*
        }

        struct $server<S: $service> {
            server: S,
        }

        impl<S: $service> $server<S> {
            fn bind(server: S) -> $server<S> {
                $server {
                    server,
                }
            }
        }

        impl<S: $service> rpc::Server for $server<S> {
            rpc::server_methods! { $service, $($method, $req, $resp),* }
        }
    };
}

#[macro_export]
macro_rules! client_method {
    ($service:ident, $method:ident, $req:ident, $resp:ident) => {
        fn $method(&self, ctx: &rpc::Context, req: $req) -> Result<$resp, Error> {
            use buffertk::{Packable, Unpackable};
            let req = buffertk::stack_pack(req).to_vec();
            let buf = self.client.call(ctx, stringify!($service), stringify!($method), &req)?;
            let (resp, buf) = <Result<$resp, Error> as buffertk::Unpackable>::unpack(buf.as_bytes())?;
            // TODO(rescrv): Log if buf is non-empty.
            resp
        }
    };
}

#[macro_export]
macro_rules! server_methods {
    ($service:ident, $($method:ident, $req:ident, $resp:ident),+) => {
        fn call(&self, ctx: &rpc::Context, method: &str, req: &[u8]) -> Result<buffertk::Buffer, rpc::Error> {
            use buffertk::{stack_pack, Packable, Unpackable};
            match method {
                $(
                stringify!($method) => {
                    let req = <$req as buffertk::Unpackable>::unpack(req)?.0;
                    let resp = self.server.$method(ctx, req);
                    Ok::<buffertk::Buffer, rpc::Error>(buffertk::stack_pack(resp).to_buffer())
                }
                ),*
                _ => {
                    Err(rpc::Error::UnknownMethodName {
                        core: error_core::ErrorCore::default(),
                        name: method.to_string(),
                    })
                }
            }
        }
    };
}
