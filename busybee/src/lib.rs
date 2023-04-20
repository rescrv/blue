extern crate prototk_derive;

use std::ops::Deref;
use std::sync::Mutex;

use buffertk::{stack_pack, Buffer, Packable, Unpackable};

use prototk::Error as ProtoTKError;
use prototk::field_types::*;
use prototk_derive::Message;

////////////////////////////////////////////// Context /////////////////////////////////////////////

pub struct Context {
}

////////////////////////////////////////// ResponseHolder //////////////////////////////////////////

pub trait ResponseHolder {
}

/////////////////////////////////////////////// Error //////////////////////////////////////////////

pub enum Error {
}

///////////////////////////////////////////// RPCServer ////////////////////////////////////////////

pub trait RPCServer {
    type Error: Packable;

    fn call(&self, ctx: Context, func: String, req: &[u8]) -> Result<Buffer, Self::Error>;
}

///////////////////////////////////////////// RPCClient ////////////////////////////////////////////

pub trait RPCClient {
    type Error<'a>: Unpackable<'a>;

    fn call<'a>(&self, ctx: Context, func: String, req: &[u8]) -> Result<Buffer, Self::Error<'a>>;
}
