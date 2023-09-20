tuple_key
=========

A serialization format for lexicographically sorted tuples.  The keys have the property that any TypedTupleKey that
implements lexicographically sorted Ord, PartialOrd traits in field declaration order will serialize to a valid byte
string that sorts in the same way.

Status
------

Maintenance track.  The library is considered stable and will be put into maintenance mode if unchanged for one year.
The clock was last reset 2023-08-02.

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

- 0.2.0 -> 0.3.0: Add schema support.
- 0.1.1 -> 0.2.0: Added support for empty tuples in named structs.  Backwards-compatible otherwise.
