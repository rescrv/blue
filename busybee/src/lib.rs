extern crate prototk_derive;

use buffertk::{Buffer, Packable, Unpackable};

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
