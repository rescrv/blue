rustrc
======

rustrc is an `rc_conf`-based process supervisor library and binary.

Container Init
--------------

`rustrc` can run as PID 1 inside a container.  When the binary detects that it is process 1, it enables init behavior automatically: it reaps exited orphan children that are not rustrc-managed services, and on Linux it asks the kernel to make it a child subreaper.  Use `--container-init` to enable the same behavior when testing outside PID 1.

The control socket remains enabled by default.  Use `--no-control-sock` for minimal containers that only need signal-driven shutdown.

Status
------

Maintenance track.  The library is considered stable and will be put into maintenance mode if unchanged for one year.

Scope
-----

This library provides the rustrc binary.
It consumes `rc_conf` configuration files and therefore inherits any breaking parser behavior changes (for example anchored `source` resolution).

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/rustrc/latest/rustrc/).
