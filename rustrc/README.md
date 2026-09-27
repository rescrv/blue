rustrc
======

rustrc is an `rc_conf`-based process supervisor library and binary.

Container Init
--------------

When `rustrc` is process 1, or given `--container-init`, it splits in two before starting any
thread.  The parent is a small single-threaded init that forwards every signal to the supervisor and
reaps every orphan; the child is the supervisor, an ordinary process that never reaps anything it
did not spawn.  When the supervisor exits, the init sends SIGTERM to anything still running (as PID
1, everything in the namespace; as a Linux child subreaper outside PID 1, the children reparented to
it), reaps for up to 10s, sends SIGKILL to what remains, and exits with the supervisor's status
(128 plus the signal number if it was killed).  A supervisor panic therefore stops services with a
grace period, instead of the kernel SIGKILLing the namespace when PID 1 dies.

The control socket remains enabled by default.  Use `--no-control-sock` for minimal containers that
only need signals.

Signals
-------

- SIGTERM, SIGINT, SIGQUIT:  shut down.  Nothing new starts; every service's process group gets
  SIGTERM, then SIGKILL after its STOP_TIMEOUT; rustrc exits 0.
- SIGHUP:  reload rc.conf and rc.d, like `rustrcctl services -r`.  The plan is logged.
- Everything else is ignored.  Use `rustrcctl kill` to signal a service.

Control
-------

`rustrcctl` talks to a running rustrc over its control socket (`--control-sock PATH`, else
`$RUSTRC_CONTROL_SOCK`, else `rc.sock`).  With no command it reads one command per line from stdin.

The socket is created mode 0600 and accepts only peers running as rustrc's user or root.  A socket
left behind by a rustrc that died is replaced on startup; if another rustrc is listening on it,
rustrc exits with an error instead.  Startup takes the state directory lock first, then the socket,
then fences leftovers and starts services.

```text
rustrcctl status [SERVICE...]               state, pid, uptime, starts, last exit, backoff, log
rustrcctl services -l|-e [SERVICE...]       list known or enabled services
rustrcctl services -r [-n]                  reload and print what will change (-n: plan only)
rustrcctl services -s|-S|-R SERVICE...      start, stop, restart
rustrcctl kill [-s SIGNAL] SERVICE|PID...   signal a service (default TERM); "*" is refused
rustrcctl metrics                           rustrc's counters, Prometheus text format
```

The reload plan lists services to start (with any remaining backoff), services to restart and
which environment keys changed (never values), running services the new configuration disables
(rustrc leaves these running; stop them explicitly), and errors.

Logging
-------

rustrc logs to stderr, one line per event:  a level letter, a timestamp, the source location, and a
structured value.  `--verbosity` picks how much:  3 errors, 6 warnings, 9 (the default) lifecycle
events such as starts, exits with their status, restarts, and reloads, and 12 debug chatter.  Logged
execution contexts name environment keys but never values, and redact env(1)-style `KEY=VALUE`
words in WRAPPER and arguments.

Log lines go through a bounded queue to a writer thread, so a stalled reader of stderr can never
block rustrc.  When the queue is full, lines are dropped, counted in `rustrc.log.dropped`, and
announced in the log once there is room.

Per-Service Variables
---------------------

rustrc reads these alongside the stub's own variables; they resolve like any other rc_conf variable,
so a global default works:

- `LOG`:  append the service's stdout and stderr to this path.  `LOG="/var/log/rustrc/${NAME}.log"`
  gives every service its own file.  Changing it restarts the service.
- `STOP_TIMEOUT`:  seconds (fractions allowed) between SIGTERM and SIGKILL when stopping; default
  10.  SIGTERM is sent the moment a stop begins.  Changing it applies to the next stop without a
  restart.  Keep it below your orchestrator's grace period (Docker's default is 10s, Kubernetes'
  30s) or the orchestrator's SIGKILL arrives first.

A stop rustrc asks for (stop, restart, a reload that changes a service's context, shutdown) is not a
crash:  the replacement starts without a backoff.  A reload restarts changed services concurrently,
so one slow stop does not delay starting or respawning anything else.

Stubs
-----

To compute a service's environment, rustrc runs the stub as `STUB rcvar` and binds the variables it
names.  The stub must answer `rcvar` without starting the service (rcscript stubs do).  A stub that
has not answered within `--stub-timeout-ms` (default 10000) has its process group killed, and the
start counts as a failure.  rustrc never holds its state lock while a stub runs, so a slow stub
delays only its own service.

Services start with an empty signal mask and default signal dispositions, regardless of rustrc's
own mask.  Each service leads its own process group and reads stdin from `/dev/null`.  Stopping a
service signals its whole process group, and when a service's main process exits, rustrc kills
anything left in its group before reaping it; a service that wants a process to outlive it must move
that process to a new session or group.

Restarts
--------

A service that exits without being asked to is restarted after a delay.  Each such exit doubles the
service's penalty after crediting the uptime it had; the delay is the penalty clamped to 1-60s, with
+/-50% jitter.  A crash after a healthy run restarts in 0.5-1.5s.  A service that dies on every start
backs off to 30-90s, and about four minutes of uptime clears the penalty.  Failed starts
(a stub that errors or times out, posix_spawn failing) count as crashes with no uptime.

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
