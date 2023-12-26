//! The builtins RPC service.

use prototk_derive::Message;

///////////////////////////////////////////// BuiltIns /////////////////////////////////////////////

/// An Empty request or response.
#[derive(Clone, Debug, Default, Message)]
pub struct Empty {}

// The builtins RPC service, server, client.
rpc_pb::service! {
    name = Builtin;
    server = BuiltinServer;
    client = BuiltinClient;
    error = rpc_pb::Error;

    rpc nop(Empty) -> Empty;
}

/// The concrete builtins RPC service.
#[derive(Debug, Default)]
pub struct BuiltinService {}

impl BuiltinService {
    /// Create a new BuiltinService.
    pub fn new() -> Self {
        Self {}
    }
}

impl Builtin for BuiltinService {
    fn nop(&self, _: &rpc_pb::Context, _: Empty) -> Result<Empty, rpc_pb::Error> {
        Ok(Empty {})
    }
}
