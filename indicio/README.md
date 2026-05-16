indicio
=======

Indicio provides a framework for emitting clues that may be used for debugging.

Status
------

Active development.  Indicio is likely to change in the near future in backwards-incompatible ways.

Scope
-----

Indicio provides:

- A `Value` tree for structured clue payloads.
- The `value!` and `clue!` macros for lazy clue construction.
- `Collector` for registering an emitter and filtering by verbosity.
- `StdioEmitter` for human-readable stderr output.
- `ProtobufEmitter` for append-only clue files when the `prototk` feature is enabled.

The protobuf file format is a sequence of field-1 clue messages.  It remains readable as a
`ClueVector` so existing consumers can continue to decode files written by `ProtobufEmitter`.

Warts
-----

- `Map` preserves insertion order and duplicate keys.  Lookup returns the first matching key.
- `Value` equality gives `f64` values a total ordering semantics, so values such as `-0.0` and
  `0.0` compare differently.
- `puzzle_piece!` is intentionally small.  Use `try_extract` when callers need a missing-path or
  type-mismatch diagnostic; `extract` remains available for the older `Option`-returning API.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/indicio/latest/indicio/).
