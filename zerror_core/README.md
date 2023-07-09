zerror_core
===========

A complete implementation of the zerror:Z trait.

Status
------

Maintenance track.  The library is considered stable and will be put into maintenance mode if unchanged for one year.

Scope
-----

This library is scoped to provide the [ErrorCore](https://docs.rs/zerror_core/latest/zerror_core/struct.ErrorCore.html)
struct.

Warts
-----

- There has to be a default implementation, but ideally every instantiation of ErrorCore should be non-default for easy
  error tracking.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/zerror_core/latest/zerror_core/).
