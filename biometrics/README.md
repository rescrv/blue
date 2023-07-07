biometrics
==========

Biometrics provide the vitals of a process in the form of counters, gauges, moments, and T-digests.  Collectively, these
sensors paint a picture of what's happening within a process in timeseries form.

Status
------

Active development.  Biometrics is likely to change in the near future in backwards-incompatible ways.  Planned changes
will affect how to register sensors in order to solve the dependency graph problem.

Scope
-----

Biometrics will provide core sensor types and a plaintext emitter for counter, gauge, and moments types.  Protocol
buffer definitions for sensor readings can be found in the [biometrics_pb](https://crates.io/crates/biometrics_pb)

Warts
-----

- Currently there's no consensus about whether a crate's dependencies should be loaded to a `Collector`.  The
  `ingest_swizzle` function can be used to get singleton-like behavior without singleton collectors.  This part is under
  active development.  It is intended to be a feature hidden away in program initialization.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/biometrics/latest/biometrics/).
