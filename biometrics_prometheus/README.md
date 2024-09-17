biometrics_prometheus
=====================

biometrics_prometheus provides a Prometheus emitter for biometrics.  It is a crate that is part of the
[biometrics](https://crates.io/crates/biometrics) project.

The emitter takes a prefix and appends "<epoch_millis>.prom" to the prefix to determine where to write next.  For
example, the following will write a path like, `tmp.foo.1726547192.prom`:

```rust
let mut emitter = Emitter::new(Options {
    segment_size: 1024,
    flush_interval: Duration::from_secs(1),
    prefix: Path::new("tmp.foo."),
});
emitter.emit_counter(&Counter::new("foo"), 42).unwrap();
drop(emitter);
```

The file is opened using `create_new` to guarantee it won't overwrite an existing file.  The file is locked before any
data is written.  Consequently, a reader that uses `flock` after opening the file will be able to read the file only
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

Warts
-----

Documentation
-------------

The latest documentation is always available at
[docs.rs](https://docs.rs/biometrics_prometheus/latest/biometrics_prometheus/).

Updating
--------
