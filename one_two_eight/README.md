one_two_eight
=============

one_two_eight provides typed 128-bit identifiers.  Use `generate_id` to create a type, and possibly `generate_id_protok`
to implement `prototk::Message`.

Status
------

Maintenance track.  The library is considered stable and will be put into maintenance mode if unchanged for one year.
The clock was last reset 2023-09-19.

Scope
-----

This library provides the `generate_id` and `generate_id_prototk` macros.

Warts
-----

- Macros duplicate code, but that's what they do.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/one_two_eight/latest/one_two_eight/).

Updating
--------

- 0.1.1 -> 0.2.0:  API expansion; otherwise backwards compatible.
