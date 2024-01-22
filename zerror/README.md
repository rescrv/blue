zerror
======

zerror provides an error interface for context-aware error-reporting.

Status
------

Maintenance track.  The library is considered stable and will be put into maintenance mode if unchanged for one year.

Scope
-----

This library is scoped to provide the Z trait.

Warts
-----

- zerror_core is a separate crate that provides a wrappable struct for implementing Z.  This is mildly inconvenient, but
  was done to separate this library from the zerror_core dependencies.

Upgrading
---------

- 0.3 -> 0.4:  The `with_*` methods have been consolidated into a single `with_info` and `with_lazy_info`
  implementation.  They will be removed in 0.5.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/zerror/latest/zerror/).
