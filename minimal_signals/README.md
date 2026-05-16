minimal_signals
===============

Minimal signals provides a small set of tools to block signals, receive them synchronously, or install a Ctrl-C-style callback handler.

Examples
--------

Install a simple Ctrl-C handler:

```rust
minimal_signals::set_handler(|| {
    eprintln!("shutting down");
})?;
# Ok::<(), minimal_signals::Error>(())
```

Handle several termination signals with one callback:

```rust
minimal_signals::set_handler_for(minimal_signals::SignalSet::termination(), |signal| {
    eprintln!("received {signal}");
})?;
# Ok::<(), minimal_signals::Error>(())
```

Wait synchronously from a dedicated thread:

```rust
minimal_signals::block();

std::thread::spawn(|| {
    let signals = minimal_signals::SignalSet::termination();
    while let Some(signal) = minimal_signals::wait(signals.clone()) {
        eprintln!("received {signal}");
    }
});
```

Status
------

Maintenance track.  The library is considered stable and will be put into maintenance mode if unchanged for one year.

Scope
-----

This library provides `set_handler`, `set_handler_for`, `block`, `unblock`, `kill`, `raise`, `pending`, and `wait`; the `SignalSet`; and named signal constants for common Unix signals.

Warts
-----

None discovered yet.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/minimal_signals/latest/minimal_signals/).
