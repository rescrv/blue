rc_conf
=======

`rc_conf` provides an rc-like configuration syntax inspired by FreeBSD's init system.  The core principle of `rc_conf`
is that the configuration should be separated from the runtime.  `rc_conf` achieves exactly that by putting the
configuration into an `rc.conf` file (much like /etc/rc.conf on BSD) and then using the configuration to run programs
via `rc.d` service stubs.

For example:  A memcached service deploy starts with two pieces.  The first is the `rc.d` service stub.  It looks like
this:

```text
#!/usr/bin/env rcscript

DESCRIBE=A single memcached instance.
COMMAND=memcached -v
```

The format is intended to be self-describing.  The only two gotchas two beware are that this file should be executable
and the command should run in the foreground and not daemonize.

If we try to run this service as-is it won't run.  By default, every service is disabled.  To enable memcached, create
an `rc.conf` with the following:

```text
memcached_ENABLED="YES"
```

This will cause [`rustrc`](https://crates.io/crates/rustrc) (the supervisor written on `rc_conf`) to keep the memcached
daemon running with some provisions for backing off during failure.

This isn't a very useful example as our end state, provides a starting point.  We may want to configure memcached and
going back to the `rc.d` file and redeploying it every time won't be fun.

Enter the first principle of `rc_conf`:  A separation between configuration and where the value gets used.  `rc.d`
scripts provide the "hooks" for the configuration by taking command line applications and binding them to `rc.conf`
files.  For example, if we wanted to configure the port and hostname for memcached, we modify the `rc.d` script to look
like:

```text
COMMAND=memcached -v ${HOST:+-i ${HOST}} ${PORT:+-p ${PORT}}
```

This will pull the values of HOST and PORT from the `rc_conf` where running memcached.  But globals get messy fast.
Whose HOST; whose PORT?  This is where `rc_conf` comes in.  To set the hostname, add this to `rc.conf`:

```text
memcached_HOST="memcached.example.org"
memcached_PORT="11211"
```

If either of these were absent, the "memcache_"-prefix-free versions would be substituted in their place.  So to have a
default host for all services that can be overridden, specify `HOST=default.example.org`.

## Aliasing

To spin up a second memcached on 22122, we can add this to our `rc.conf`:

```
memcached_two_INHERIT="YES"
memcached_two_ALIASES="memcached"
memcached_two_PORT="22122"
```

An alias such as this one uses one `rc.d` stub to launch two memcached instances.

Status
------

Maintenance track.  The library is considered stable and will be put into maintenance mode if unchanged for one year.

Scope
-----

This crate provides the `RcConf` type, `rcscript` interpreter, and other rc tools.

Tools
-----

- rcdebug:  Show a debug struct of an `rc_conf` file.
  Usage:  `rcdebug rc.conf`
- rcexamine:  Show the `rc_conf` as the parser sees it.
  Usage:  `rcexamine rc.conf:rc.conf.local`
- rcinvoke:  Run a service in the foreground as `rc_conf` would prescribe.
  Usage:  `rcinovke --rc-conf-path rc.conf:rc.conf.local --rc-d-path rc.d:/srv/rc.d memcached`
- rclist:  List the rc.d scripts available in an rc.d path
  Usage:  `rclist rc.d /srv/rc.d`
- rcvar:  Output the rcvariables a service looks to for its configuration.
  Usage:  `rcvar --rc-conf-path rc.conf:rc.conf.local --rc-d-path rc.d:/srv/rc.d memcached`
- rcscript:  An interpreter for rc.d shell stubs.
  Usage:  as an interpreter

Warts
-----

- A string with `'{'` and `'}'` characters outside the variable declarations won't parse right now.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/rc_conf/latest/rc_conf/).
