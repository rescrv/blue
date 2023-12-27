blue
====

blue is a mono-repo that serves as the home of a Rust-based key-value store.

Why you can begin to trust this code
------------------------------------

This repo has a focus on correctness first, performance second.  The key-value store is end-to-end checksummed according
to the setsum algorithm.  Setsum provides an order-agnostic hash function.  Every sst written is written with a setsum
over the data.  Every tree operation asserts that the checksums match what's expected.  Compaction merely rewrites data
and doesn't delete it.  Garbage collection must list the hash of the data discarded.  Adding new ssts must list the hash
of the data added (technically it subtracts).

The lsmtk key-value store comes in two parts that must run in tandem.  The LsmTree (or key-value store) writes data,
performs compaction, and performs garbage collection.  Files that are obsoleted by the LsmTree's compaction get moved to
a trash directory.  A verifier checks the setsums of each transaction over the key-value store and unlinks files only
after they've been verified.  The garbage collection policy is specified to the verifier and the LsmTree and the
verifier must retain strictly less data than the LsmTree for garbage collection to be valid.

That being said, this code is new.  And the read path is not covered by setsums.

It needs more time to bake to be asserted as correct.  That being said, many bugs (mostly off-by-ones and invalid
boundary conditions) have been found using setsums that cause the process to crash or hang when the setsum doesn't
balance.

What's Here
-----------

It consists of the following components, listed in a topological sort of their dependencies.

It all builds up to lsmtk right now, which is a stand-in for RocksDB or LevelDB.
Eventually it will be a distributed key-value store.

- buffertk: Tools for packing and unpacking data manually.
- derive_util:  A helper for writing derive macros.
- arrrg_derive:  Automatically derive arrrg::CommandLine.
- arrrg:  An opinionated command-line library with an eye to safety.
- bloomcalc:  A calculator for figuring out bloom filter parameters.
- guacamole:  A linearly-seekable random number generator.
- armnod:  A random strings generator.
- keyvalint:  KEY-VALue INTerface traits for key-value stores.
- prototk_derive:  Derive macro for prototk::Message.
- setsum:  An order-agnostic/set-of-strings hash function.
- skipfree:  A lock-free skip list.
- utilz:  Miscellaneous utilities.
- biometrics:  Counters, gauges, and moments for instrumenting processes.
- sync42:  Synchronization.  Spinlock, monitor, work-coalescing-queue, wait-list and more.
- texttale:  A tool for writing shells and expect scripts for said shells.
- tiny_lfu:  An admission-control algorithm for cache admission.
- zerror:  An error type.
- prototk:  Protocol buffers library.
- macarunes:  Macarunes provide capability-based authorization.
- one_two_eight:  128-bit identifiers.
- tatl:  A library for monitoring and alerting.
- indicio:  A library for tracking clues and building typed debuggers.
- tuple_key_derive:  A derive macro for tuple_key::TypedTupleKey.
- zerror_derive:  A derive macro for zerror_core.
- zerror_core:  A core for easily implementing zerror.
- mani:  Text-based write-ahead logging.
- rpc_pb:  Protocol buffers for RPC.
- busyrpc:  A basic RPC library.
- split_channel:  Bidirectional pipes.
- sst:  A sorted-string tables library.
- lsmtk:  An LSM-tree with triangular compaction.
- keyvalint_bench:  Benchmarks for interfaces implementing keyvalint.
- tuple_key:  Lexicographical tuple encoding for keys.
- biometrics_pb:  Protocol buffers definitions for biometrics.
