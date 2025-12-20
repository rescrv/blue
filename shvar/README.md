shvar
=====

shvar is the SHell VARiable expansion library.

Status
------

Maintenance track.  The library is considered stable and will be put into maintenance mode if unchanged for one year.

Scope
-----

This library provides the `quote`, `split`, `expand`, and `rcvar` functions.

Warts
-----

- A string with `'{'` and `'}'` characters outside the variable declarations won't parse right now.

Breaking Changes
----------------

- `split("")` now returns `[]` instead of `[""]`. Empty or whitespace-only input yields an empty vector.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/shvar/latest/shvar/).
