protoql
=======

ProtoQL is the protocol buffers query language.  It provides an object mapping for a key value store with the following
properties:

- ProtoQL defines a table-set abstraction that provides a mapping from key-value pairs to protocol buffers objects.
- Writes operate on key-value pairs so that individual fields or map elements can be updated without having to update
  large objects.
- Every range of keys starting with a valid tuple-key prefix encode deterministically to a protocol buffers object.
- ProtoQL definitions are mechanically translatable to protocol buffers 2.

Status
------

Active development.  The API is likely to grow and change.

Scope
-----

This crate provides everything related to the protoql query language and execution.

Warts
-----

- There's currently no query executor.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/protoql/latest/protoql/).
