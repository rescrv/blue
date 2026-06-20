use std::ops::Deref;
use std::sync::Mutex;

use prototk_derive::Message;

use rpc_pb::{Context, SError, service};

//////////////////////////////////////////// The Service ///////////////////////////////////////////

#[derive(Debug, Default, Message)]
pub struct CacheLoad<'a> {
    #[prototk(1, bytes)]
    key: &'a [u8],
}

#[derive(Debug, Default, Message)]
pub struct CacheResponse {
    #[prototk(2, bytes)]
    val: Option<Vec<u8>>,
}

#[derive(Debug, Default, Message)]
pub struct CacheStore<'a> {
    #[prototk(1, bytes)]
    key: &'a [u8],
    #[prototk(2, bytes)]
    val: &'a [u8],
}

#[derive(Debug, Default, Message)]
pub struct CacheEmpty {}

service! {
    name = Cache; // No magic.  The name of the trait for this service.
    server = CacheServer; // No magic.  The name of the type for the server.
    client = CacheClient; // No magic.  The name of the type for the client.
    error = SError; // No magic.  The name of the error type.  Must implement From<rpc_pb::SError>.
    rpc load(CacheLoad) -> CacheResponse;
    rpc store(CacheStore) -> CacheEmpty;
}

////////////////////////////////////////// Implementation //////////////////////////////////////////

pub struct CachedRegister {
    value: Mutex<(Vec<u8>, Vec<u8>)>,
}

impl Cache for CachedRegister {
    fn load(&self, _: &Context, req: CacheLoad) -> Result<CacheResponse, SError> {
        let guard = self.value.lock().unwrap();
        let (key, value) = guard.deref();
        if key == req.key {
            let val = Some(value.clone());
            Ok(CacheResponse { val })
        } else {
            Ok(CacheResponse { val: None })
        }
    }

    fn store(&self, _: &Context, req: CacheStore) -> Result<CacheEmpty, SError> {
        let key = req.key.to_vec();
        let val = req.val.to_vec();
        *self.value.lock().unwrap() = (key, val);
        Ok(CacheEmpty {})
    }
}

fn main() {}
