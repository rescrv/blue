rustrc
======

rustrc is an `rc_conf`-based process supervisor library and binary.

Container Init
--------------

`rustrc` can run as PID 1 inside a container.  When the binary detects that it is process 1, it enables init behavior automatically: it reaps exited orphan children that are not rustrc-managed services, and on Linux it asks the kernel to make it a child subreaper.  Use `--container-init` to enable the same behavior when testing outside PID 1.

The control socket remains enabled by default.  Use `--no-control-sock` for minimal containers that only need signal-driven shutdown.

Control
-------

`rustrcctl` talks to a running rustrc over its control socket (`--control-sock PATH`, else
`$RUSTRC_CONTROL_SOCK`, else `rc.sock`).  With no command it reads one command per line from stdin.

```text
rustrcctl status [SERVICE...]               state, pid, uptime, starts, last exit, backoff, log
rustrcctl services -l|-e [SERVICE...]       list known or enabled services
rustrcctl services -r [-n]                  reload and print what will change (-n: plan only)
rustrcctl services -s|-S|-R SERVICE...      start, stop, restart
rustrcctl kill [-s SIGNAL] SERVICE|PID...   signal a service (default TERM); "*" is refused
```

The reload plan lists services to start (with any remaining backoff), services to restart and
which environment keys changed (never values), running services the new configuration disables
(rustrc leaves these running; stop them explicitly), and errors.

Per-Service Variables
---------------------

rustrc reads these alongside the stub's own variables; they resolve like any other rc_conf variable,
so a global default works:

- `LOG`:  append the service's stdout and stderr to this path.  `LOG="/var/log/rustrc/${NAME}.log"`
  gives every service its own file.  Changing it restarts the service.
- `STOP_TIMEOUT`:  seconds (fractions allowed) between SIGTERM and SIGKILL when stopping.  Without
  it, rustrc keeps its original schedule (SIGTERM after 2s, 6s, and 14s, then SIGKILL).  Changing it
  applies to the next stop without a restart.

Services start with an empty signal mask and default signal dispositions, regardless of rustrc's
own mask.

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
