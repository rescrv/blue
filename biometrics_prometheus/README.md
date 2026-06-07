biometrics_prometheus
=====================

biometrics_prometheus provides a Prometheus emitter for biometrics.  It is a crate that is part of the
[biometrics](https://crates.io/crates/biometrics) project.

The emitter takes a prefix and appends "<epoch_millis>.prom" to the prefix to determine where to write next.  For
example, the following will write a path like, `tmp.foo.1726547192.prom`:

```rust,no_run
use std::time::Duration;

use biometrics::{Counter, Emitter as _};
use biometrics_prometheus::{Emitter, Options};
use utf8path::Path;

let mut emitter = Emitter::new(Options {
    segment_size: 1024,
    flush_interval: Duration::from_secs(1),
    prefix: Path::new("tmp.foo."),
});
emitter.emit_counter(&Counter::new("foo"), 42).unwrap();
drop(emitter);
```

The file is opened using `create_new` to guarantee it won't overwrite an existing file.  The file is locked before any
data is written.  Consequently, a reader that acquires a lock after opening the file will be able to read the file only
after all data has been written to the file.  The included `Reader` does exactly that.

There's a pitfall to using `Reader`, however.  If the reader is opened before the writer finishes writing, the reader
will block and wait.  A naive implementation that collects emitted files might have emitters put them into one
directory, and have a script read each file in that directory.  If the script reads the file before the writer finishes,
it will block and wait.  A locked process that's not rotating its logs in time would then halt system activity.

To avoid this pitfall, use the included `Watcher`.  The Watcher will watch a directory for files, locking each one in
turn and reading it.  The Watcher will not block on a file that's being written to.  The Watcher will also allow files
to be removed once they have been processed in an idiomatic way.

Status
------

Active development.

Scope
-----

The crate is intended to be used as a Prometheus emitter for biometrics.

The `with-system-metrics` binary wraps a command and emits child process
`biometrics_sys` counters to the default metrics file path.
Usage:

```text
with-system-metrics [--poll-in-seconds <seconds>] <command> [args...]
```

`--poll-in-seconds` controls the interval (in whole seconds) used to
sample and emit child resource usage while the wrapped command is running.
When the wrapped command exits, one final sample is emitted immediately.
It defaults to `1`.

Warts
-----

Documentation
-------------

The latest documentation is always available at
[docs.rs](https://docs.rs/biometrics_prometheus/latest/biometrics_prometheus/).

Updating
--------
