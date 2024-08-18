rc_conf
=======

Status
------

Maintenance track.  The library is considered stable and will be put into maintenance mode if unchanged for one year.

Scope
-----

This crate provides the `RcConf` type, `rcscript` interpreter, and other rc tools.

Warts
-----

- A string with `'{'` and `'}'` characters outside the variable declarations won't parse right now.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/rc_conf/latest/rc_conf/).
