use prototk_derive::Message;

///////////////////////////////////////////// BuiltIns /////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
pub struct Empty {}

rpc_pb::service! {
    name = Builtin;
    server = BuiltinServer;
    client = BuiltinClient;
    error = rpc_pb::Error;

    rpc nop(Empty) -> Empty;
}

#[derive(Debug, Default)]
pub struct BuiltinService {}

impl BuiltinService {
    pub fn new() -> Self {
        Self {}
    }
}

impl Builtin for BuiltinService {
    fn nop(&self, _: &rpc_pb::Context, _: Empty) -> Result<Empty, rpc_pb::Error> {
        Ok(Empty {})
    }
}
