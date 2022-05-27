setsum
======

setsum is an unordered checksum that operates on sets of data.  Where most
checksum algorithms process data in a streaming fashion and produce a checksum
that's unique for each stream, setsum processes a stream of discrete elements
and produces a checksum that is unique to those elements, regardless of the
order in which they occur in the stream.

In code it looks like this:

```rust
    let mut setsum1 = Setsum::default();
    setsum1.insert("A".as_bytes())
    setsum1.insert("B".as_bytes())

    let mut setsum2 = Setsum::default();
    setsum2.insert("B".as_bytes())
    setsum2.insert("A".as_bytes())

    assert_eq!(setsum1.digest(), setsum2.digest())
```

Practical Use Cases
===================

There are a number of ways that setsum solves practical problems that cannot be
easily solved with ordinary checksums.  This section highlights the two main
patterns of setsum use

Replication Stream Checksum
---------------------------

Replication-based systems have the potential for divergence between replicas.
There are tools to solve this problem (like MySQL's pt-tc) but these tools
require scrubbing the entire cluster to look for differences between a leader
and its followers, and the scrub itself can take days to catch problems.

It is possible to modify the replication stream to use setsum to catch these
problems early; effectively, the setsum maintains a rolling checksum of the
whole data set.

The way to do this is to have one setsum object that represents the state of the
database and gets periodically persisted so its recovery is possible.  Then, for
each transaction, the setsum gets updated with the pre- and post-image of the
data that's being replicated.  At the end of these updates, the setsum reflects
the checksum of the entire database that's been replicated.

There's a trade-off here compared to other schemes.  If using a replication
scheme with built-in identifiers (like MySQL's GTIDs or Raft's log), this scheme
allows instant comparison of all replicas.  What it won't do is protect against
corruption of the data that's been written to disk.  For that, one would need to
take a snapshot and compare the setsum of the snapshot with a freshly computed
setsum; else, it's always possible to continue running tools like pt-tc.

Bookkeeping an LSM tree
-----------------------

Log-structured merge (LSM) trees are ubiquitous nowadays, with the most common
ones being derived from Google's LevelDB.  The defining characteristic of these
variants is that the majority of data is stored in immutable files, and new data
is integrated into the tree via something called "compaction".  The process of
compaction replaces a set of these immutable files with a different set.  The
data gets rewritten in the process, and old data gets garbage collected.
Because data is otherwise immutable, compaction is the weakest link for
software-driven data corruption in an LSM tree.  

One way to use setsum to solve this problem is to do simple bookkeeping of the
inputs and outputs to compaction.  If we view compaction as a transformative
process, data is neither created nor destroyed, so the checksum of the inputs
must equal the checksum of the outputs.  Of course, this view of compaction
doesn't directly take into account garbage collection---to do so we need to
account for the garbage collected data along with the remaining outputs.

What's necessary afterwards is to add a verifier that makes sure that the
contents of files matches their associated setsum.  If the files don't match,
compaction went awry.  If the inputs are saved until after verification, the
mistake can be safely reverted.

There's a trade-off here as well:  An LSM tree protected by setsums is not able
to do more complicated compaction logic that merges values together.  While it
could track that the files match their checksums, the compaction would be unable
to balance the input and output checksums.
