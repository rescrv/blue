prototk
=======

prototk provides a toolkit for prototcol buffers.

Status
------

Maintenance track.  The library is considered stable and will be put into maintenance mode if unchanged for one year.
The clock was last reset 2023-09-07.

Scope
-----

This library is about serialization and deserialization of messages.  It strives to distil protocol buffers to this:

```
#[derive(Debug, Default, Message)]
pub enum Error {
    #[prototk(278528, message)]
    #[default]
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
}

// serialize
let err = Error::UnknownServerName {
    core: ErrorCore::new("robert@rescrv.net", "unknown server name", &UNKOWN_SERVER_NAME_COUNTER),
    name: "FooRpcServer",
};
let buf = stack_pack(err).to_vec()

// deserialize
let up = Unpacker::new(&buf);
let err: Error = up.unpack()?;
```

Warts
-----

- The derive macro's errors are not the most easy to understand.

Reserved Field Ranges
---------------------

The error types in my libraries all have diffrent field numbers.  Here is where I track them.

- 262144..262400 prototk::Error
- 278528..278784 rpc_pb::Error
- 294912..295168 macarunes::Error
- 311296..311552 tuple_key::Error
- 376832..377088 mani::Error
- 442368..442624 sst::Error


Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/prototk/latest/prototk/).
