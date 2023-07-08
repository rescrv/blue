guacamole
=========

Guacamole provides a linearly-seekable random number generator.  It gives you a long continous stream of bytes (`2**70`
bytes worth) and the ability to seek to any 64-byte aligned offset within that stream in constant time.  This is useful
for benchmarks and other trials that need pseudo-data as it recreates the same data over time, so long as the same index
and procedure are used.

The key example that guacamole is useful for is a large key-value-store workload.  Imagine having 1e12 key-value pairs
in a database.  How would you keep track of that many keys to perform queries over the data in the workload without a
second, large key-value store for them?

One answer is guacamole.  Divide the `2**64` offsets into N contiguous ranges.  Make them of equal size and it can be
done with division/modulus.  Each of the N ranges generates a different sequence of `2**64/N` bytes which should be used
as the sole basis of randomness for generating workload operations.  Each time the same seed is used the same operation
results.

Use Cases
---------

Please link yours here as appropriate.

key-value-store workload:  As described above, the guacamole is partitioned into N different keys and each acts as an
    independent stream.  Higher- level randomness can take control of which key gets generated when, picking numbers [0,
    N) using that other source of randomness and then acting like pseudo-fn(x: usize) -> [blargh; x]

distributed filesystem workload:  Files can be carved out of the seed-space, and then generated in parallel, with a 1:1
    correspondence between bytes of guacamole and bytes of the files written out.

Status
------

Maintenance track.  The library is considered stable and will be put into maintenance mode if unchanged for one year.

Scope
-----

This library provides the guacamole type and a tool for drawing from a Zipf distribution.

Warts
-----

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/guacamole/latest/guacamole/).
