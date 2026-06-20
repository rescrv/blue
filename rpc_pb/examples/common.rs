use prototk_derive::Message;

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Debug, Default, Message)]
pub enum Error {
    #[prototk(1, message)]
    #[default]
    Success,
    #[prototk(2, message)]
    Serialization {
        #[prototk(1, message)]
        err: prototk::SError,
        #[prototk(2, string)]
        context: String,
    },
    #[prototk(3, message)]
    Transport {
        #[prototk(1, message)]
        err: rpc_pb::SError,
        #[prototk(2, string)]
        context: String,
    },
}

impl From<rpc_pb::SError> for Error {
    fn from(err: rpc_pb::SError) -> Error {
        Error::Transport {
            err,
            context: "rpc error".to_string(),
        }
    }
}

#[allow(dead_code)]
fn main() {}
