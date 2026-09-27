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
Prometheus reading before canonicalization.  Input labels beginning with `__`
are rejected so callers cannot spoof Saros-internal tags or the metric name.
This is the namespace Prometheus itself reserves.

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
keeps the file basename and ingest timestamp for debugging, plus a bitmask of
the metric types that were storable when it was written.  A checkpoint suppresses
re-ingest only when its mask covers every type the running build can store, so
widening support re-opens exactly the files that were partially discarded.

Warts
-----

- Only counters are ingested.  Gauges, histograms, and moments are parsed and
  dropped; `saros.ingest.skipped_unsupported` counts them.  `fetch_gauges` and
  `fetch_histograms` return empty responses rather than an unimplemented error.
- The canonical tag encoding is a delimiter-joined string with no escaping, so
  `:` is not representable in a tag key or value.  That excludes the Prometheus
  recording-rule naming convention, which uses `:` in metric names.  Construction
  rejects these rather than storing a value that cannot be parsed back.
- The tag index is not time-scoped.  Posting keys span all history, so query cost
  grows with the number of series that have ever existed rather than with the
  number alive in the queried window.  Under short-lived series this is the
  dominant cost.
- A series' storage frontier is found by walking segments backwards, bounded by
  `SarosStoreOptions::max_lookback_segments`.  A series silent for longer than
  the bound is treated as new, so a sample older than its true last timestamp is
  accepted rather than rejected.  `saros.store.lookback_exhausted` counts every
  occurrence.
- Unflushed chunks are held against a process-wide byte budget.  Crossing it
  flushes the largest pending chunks early, which produces chunks below the 4 KiB
  target and costs read amplification.  `saros.store.pending_budget_flush` counts
  these.
- `median` and `median_absolute_deviation` sort the full point set and cannot be
  computed incrementally, so they are the two aggregates that do not survive
  pushing aggregation into the store.
- Compaction is not time-series-aware.  This is stated in the storage format
  section as a description; it is also a limitation.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/saros/latest/saros/).
