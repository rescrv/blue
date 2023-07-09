biometrics_pb
=============

biometrics_pb provides protocol buffers corresponding to biometric readings.

Status
------

Active development.  Biometrics is likely to change in the near future in backwards-incompatible ways and the readings
may change too.  Planned changes will affect how to register sensors in order to solve the dependency graph problem.

This library will be documented when it transitions to maintenance track.

Scope
-----

This library should provide one type for each type of reading supported by the biometrics crate.

Warts
-----

- Currently there's no support for T-digest.
- Currently there's no documentation.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/biometrics_pb/latest/biometrics_pb/).
