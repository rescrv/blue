prototk
=======

prototk provides a toolkit for prototcol buffers.

Status
------

Maintenance track.  The library is considered stable and will be put into maintenance mode if unchanged for one year.
The clock was last reset 2023-09-19.

Scope
-----

This library is about serialization and deserialization of messages.  It strives to distil protocol buffers to this:

```ignore
use handled::SError;

#[derive(Debug, Default, Message)]
pub enum Response {
    #[prototk(278528, message)]
    #[default]
    Success,
    #[prototk(278529, message)]
    Failure {
        #[prototk(1, message)]
        err: SError,
    },
}

// serialize
let msg = Response::Failure {
    err: SError::new("unknown-server-name").with_string_field("name", "FooRpcServer"),
};
let buf = stack_pack(msg).to_vec()

// deserialize
let up = Unpacker::new(&buf);
let msg: Response = up.unpack()?;
```

Warts
-----

- The derive macro's errors are not the most easy to understand.

Reserved Field Ranges
---------------------

The error types in my libraries all have diffrent field numbers.  Here is where I track them.

- 262144..262400 prototk::Error
- 278528..278784 rpc_pb structured values
- 294912..295168 macarunes structured values
- 311296..311552 tuple_key structured values
- 376832..377088 mani structured values
- 442368..442624 sst structured values
- 507904..508160 protoql structured values
- 573440..573696 paxos_pb structured values

Maps
----

Maps are not supported by prototk natively because the typing is too complicated.  Make a MapEntry type that has field
number 1 for the key and 2 for the value.  Put it in a `Vec` tagged as a `message`.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/prototk/latest/prototk/).
