busybee
=======

BusyBee provides synchronous/thread-pool implementations of rpc_pb.

Status
------

Active development.  Busybee is likely to change in the near future in backwards-incompatible ways.  Planned changes
will affect how errors are handled.  Currently they are not well-handled.  This library is currently beta at best.

Scope
-----

BusyBee will provide the RPC types for client and server to glue rpc_pb types to the wire.

Warts
-----

- The error handling is not complete.  The error will not cause channel closure and cleanup.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/busybee/latest/busybee/).
