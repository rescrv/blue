split_channel
=============

split_channel provides a stream-of-messages abstraction with split send/recv channels.

Status
------

Maintenance track.  The library is considered stable and will be put into maintenance mode if unchanged for one year.
Documentation changes excepted.

Scope
-----

This library provides the abstraction of split send/recv channels.  This is a hack of the type system, allowing two
`&mut` references to the same underlying types.

Warts
-----

- I'm not a fan of the name.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/split_channel/latest/split_channel/).
