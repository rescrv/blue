tuple_key
=========

A serialization format for lexicographically sorted tuples.  The keys have the property that any TypedTupleKey that
implements lexicographically sorted Ord, PartialOrd traits in field declaration order will serialize to a valid byte
string that sorts in the same way.

Status
------

Active development.  This library has seen recent changes to match the types of keys supported by protobuf maps.
Version 0.4 reset the serialization in backwards-incompatible ways.

Scope
-----

This crate provides everything necessary to convert a struct to and from a tuple key.

Warts
-----

- The documentation is lacking.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/tuple_key/latest/tuple_key/).

Updating
--------

- 0.2.0 -> 0.3.0: Add schema support.  Changed the encoding format in a backwards-incompatible way.
- 0.1.1 -> 0.2.0: Added support for empty tuples in named structs.  Backwards-compatible otherwise.
