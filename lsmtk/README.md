lsmtk
=====

This library provides an implementation of an log-structured merge tree;  this
merge tree uses a new compaction algorithm called *triangular compaction* that
achieves a factor of 6.5x write amplification both in theory and in practice.

Triangular Compaction
---------------------

Triangular compaction takes note of a single observation:  moving from one level
to the next is inefficent; instead, the compaction should spread across levels
to move data across as many levels as is possible.

The core of the compaction algorithm comes from the intuition embedded in this
Python snippet:

```ignore
sums = 0
count = 0
ingest = 0
for i in range(2**ceil(LSM_NUM_LEVELS)):
    bits = 0
    while i and (1 << bits) & i:
        bits += 1
    sums += (1 << bits) * TARGET_FILE_SIZE * LSM_LEVEL_0_NUM_FILES
    ingest += TARGET_FILE_SIZE * LSM_LEVEL_0_NUM_FILES
    count += 1
COMPACTION_AVERAGE_SIZE = sums / count + LSM_LEVEL_0_NUM_FILES * TARGET_FILE_SIZE # bytes
WRITE_AMPLIFICATION = sums / ingest
```

In this compaction we select the lowest N levels that are full, stopping at the
first level that's empty.  In this way we amortize the cost of a compaction
similar to how a `Vec` works in Rust or `vector` in C++.

The triangular compaction algorithm generalizes this intuition to select
triangles from the LSM tree such that the transitive closure of files under that
level will be included in the compaction.

Check `src/tree/mod.rs` for the compaction algorithm.

Trade-Offs
----------

Write amplification is bounded above by 6.5x.

- Read amplification is unbounded, but stays low empirically.
- Space amplification is bounded by 2x.

In an empirical benchmark where we ingested 44GiB of data we saw the write
amplification stayed under a factor of 3x (trash is the ssts that were
compacted away; it will be emptied periodically in a real system):

```ignore 
4.0K    db/compaction
71M     db/ingest
8.8M    db/mani
44G     db/sst
127G    db/trash
```

In process counters confirms a 2.9x write amplification.

```ignore
lsmtk.bytes_ingested = 46552563486
lsmtk.compaction.bytes_written = 135401535775
```

Status
------

Active development.

Scope
-----

This library provides pieces of an lsm graph.  It will eventually grow to
support everything necessary to create an embedded lsm graph analogous to
LevelDB and RocksDB.

Warts
-----

- This library is under-tested and will see active development in the future.
- Tricks used in LevelDB (grandfather overlap) and PebblesDB (guard pages) are
  not used.  These are 100% compatible and would only improve the compaction
  algorithm's performance.
- There is no back pressure against excessive ingest.
- There's a concurrency bug that shows up around the point of 40GiB where
  compaction will stall.  This is just at the prototype phase and I've run out
  of funds to continue developing it.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/lsmtk/latest/lsmtk/).
