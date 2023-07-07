buffertk
========

Buffertk provides tooling for serializing and deserializing data.

Status
------

Maintenance track.  The library is considered stable and will be put into maintenance mode if unchanged for one year.

Scope
-----

This library is about serialization and deserialization patterns that are common.  It is chiefly intended to provide the
primitives used by the [prototk](https://crates.io/crates/prototk) crate.

Warts
-----

- Some patterns are used frequently and could be abstracted better.  Given that most of this library is used with code
  generation this is not a concern.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/buffertk/latest/buffertk/).
