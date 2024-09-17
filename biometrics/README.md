biometrics
==========

Biometrics provide the vitals of a process in the form of counters, gauges, moments, and histograms.  Collectively, these
sensors paint a picture of what's happening within a process in timeseries form.

For a prometheus-compatible emitter, see [biometrics_prometheus](https://crates.io/crates/biometrics_prometheus).

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

- The blue repo currently is not uniform in its register_biometrics functions.  The convention is that a public method
  should not call other public methods.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/biometrics/latest/biometrics/).

Updating
--------

- 0.2.0 -> 0.3.0:  API changes to remove `ingest_swizzle`.  It's recommended to have a crate transitively register its
  own modules and then have the main function register each crate's root registration function.
