use prototk_derive::Message;

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Debug, Default, Message)]
pub enum Error {
    #[prototk(1, message)]
    #[default]
    Success,
    #[prototk(2, message)]
    SerializationError {
        #[prototk(1, message)]
        err: prototk::Error,
        #[prototk(2, string)]
        context: String,
    },
    #[prototk(3, message)]
    TransportError {
        #[prototk(1, message)]
        err: rpc_pb::Error,
        #[prototk(2, string)]
        context: String,
    },
}

impl From<buffertk::Error> for Error {
    fn from(err: buffertk::Error) -> Error {
        Error::SerializationError {
            err: err.into(),
            context: "buffertk unpack error".to_string(),
        }
    }
}

impl From<prototk::Error> for Error {
    fn from(err: prototk::Error) -> Error {
        Error::SerializationError {
            err,
            context: "prototk unpack error".to_string(),
        }
    }
}

impl From<rpc_pb::Error> for Error {
    fn from(err: rpc_pb::Error) -> Error {
        Error::TransportError {
            err,
            context: "prototk unpack error".to_string(),
        }
    }
}

#[allow(dead_code)]
fn main() {}
