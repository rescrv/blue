sync42
======

sync42 provides synchronization tools.

Status
------

Active development.  sync 42 is likely to expand to include new data structures.  Existing data structures are
maintenance-track.

Scope
-----

sync42 provides core types for synchronization.  Any synchronization pattern general enough to be generalized is
candidate for inclusion.

Warts
-----

- The library is currently missing some critical data structures.
- The documentation needs work.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/sync42/latest/sync42/).

Updating
--------

- 0.2.0 -> 0.3.0:  Added the `StateHashTable`.  Backwards compatible for existing structures.
