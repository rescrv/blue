saros
=====

saros is a time series database and plotting front-end.

Status
------

Active development.

Scope
-----

This tool provides everything necessary to ingest data and plot it.

Storage Format
--------------

Saros stores time series as ordinary `sst` key-value records inside an
`lsmtk::LsmTree`.  Compaction is not time-series-aware.  The durable format is
the key layout plus the value messages described here.

All storage keys use `tuple_key2` encoding.  Counter data is currently the only
ingested metric type, but every series key includes `metric_type` so gauges,
histograms, and moments can use the same physical layout.

Physical series identity is the folded `setsum` digest of canonical tags.  Each
canonical tag is inserted into the set as a self-delimiting `tuple_key2` item
`(1, tag_key, tag_value)`.  The 32-byte setsum digest is folded to 16 bytes by
XORing the upper and lower halves.  The metric type is not part of this
fingerprint; it is a separate key component.

Saros adds `__name__=<metric_name>` and `__saros_source__=<source_id>` to every
Prometheus reading before canonicalization.  Input labels matching
`__saros_*__` are rejected so callers cannot spoof Saros-internal tags.

The store uses these top-level key families:

- Data chunk: `(0, 0, metric_type, segment_start_ts, series_fingerprint,
  last_sample_ts) -> SeriesChunk`.
- Series metadata: `(0, 1, metric_type, series_fingerprint) -> canonical tag
  string`.
- Tag index posting: `(1, metric_type, tag_key, tag_value,
  series_fingerprint) -> empty`.
- File checkpoint: `(2, content_hash_32) -> FileCheckpoint`.

`segment_start_ts` is the start of a two-hour segment.  Chunks never cross
segment boundaries.  The writer targets 4 KiB encoded chunks and rejects any
single chunk larger than 32 KiB.

`SeriesChunk` is a versioned protobuf-compatible message.  It stores the first
sample as absolute header fields, then stores all later timestamps in a
concatenable delta stream and all later `f64` values in a Gorilla XOR bitstream.
Values are encoded from `f64::to_bits()`, so `NaN` payloads and `-0.0` survive
round trips.

Prometheus file ingestion is idempotent by content hash.  The checkpoint value
keeps the file basename and ingest timestamp for debugging, but only the
content hash determines whether a file has already been processed.

Warts
-----

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/saros/latest/saros/).
