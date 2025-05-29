blue
====

blue is a mono-repo that serves as the home of a Rust-based key-value store.

## A Key-Value Store as the Building Block

I did my thesis on key-value stores, file systems, and scaling storage.  I have one design left over
that's evolved and grown over the years that didn't make my thesis:  An open-source exabyte scale
key-value store.  Let me break down precisely what I mean:  A key-value store differs from blob
storage in the size of the objects and access pattern.  While blob storage is used for large (e.g.,
128kiB to 5TiB) objects, key-value storage is smaller objects.  Less than 64kiB for sure, but more
like 10-100B per object.  That's 1.1e16 objects, or eleven quadrillion objects.

Small objects, big storage, many objects.

The current plan of record is to build a system capable of hosting a large, exabyte-scale LSM tree.
A single, exabyte-scale LSM tree.  Something so large that no company would reasonably need
something bigger, ever, unless engaging in data collection policies I'd count as unethical.

Why key-value storage?  Key-value programming, e.g. pointers and data, are the fundamentals of every
program ever written.  When given the right primitives, programmers know how to build data
structures.  Given the right systems, they will be productive.  Given the right guard rails, they
will not lose data.

Blue is about building the ecosystem necessary to scale key-value storage to an exabyte in the open
source world.  It is motivated by past experience working in terabytes to petabytes in which most
off-the-shelf storage falls flat in key ways.  It is motivated by first-principles thinking.  And
there are plenty of things re-implemented and cut in scope to avoid taking a dependency.

Surrounding the key-value storage are the necessary components to stream updates from storage---as
new updates come in to a key range, even a small key range, they can be streamed back to any number
of listeners.  Called tailog, this streaming API is the backbone of infrastructure for pretty much
everything.

The other component alongside live streaming is mapfold, a primitive that looks a lot like
map-reduce, except that it necessarily imposes structure on the types of keys involved.
Specifically, the key must be a tuple, map must preserve functional dependencies in the key, and
reduce must be an associative and commutative merge function that turns a key into a subset of the
key containing no additional data.  With these three constraints on structure, mapfold can be run
efficiently and in-parallel across an exabyte while guaranteeing many useful properties.

Finally, to put a bow on everything is an orm called protoql that marries protocol buffers, a query
language that looks like GraphQL, and key-value storage.  By carefully specifying the encoding of
key-value pairs on key-value storage, programmers can construct structs of dictionaries that
recursively contain structs of dictionaries.  The encoding ensures that updating a single leaf in
the recursive structure, or the value associated with an internal node, will involve O(1) writes
that are O(n) in the size of the local state being updated.  Each struct's keys can be retrieved in
lexicographic order to reconstruct a single protocol buffers message that corresponds to the logical
object fetched.  Thus, GraphQL-like queries turn into range scans that yield repeated protocol
buffers objects.  That's the API.  protobuf in, protobuf out, framed in RPC.

Why framed in RPC?  Recall that the map of mapfold is just a function that takes a key-value pair
and returns a new key-value pair with a key that is a functional dependency of the input key.  Well,
if we hold the key constant, RPC fits the bill as a map function.  And if we throw the key back in,
we get the ability to route by key, which means we can build hierarchical apps, and later, much
later, add multi-tenancy by the addition of an outer routing key that gets applied to the RPC
routing, the database, and the mapfold tasks.

If the distinction between RPC and mapfold is not clear:  It shouldn't be.  There's no reason
mapfold cannot invoke an RPC service in it's map step.  And there's no reason the output of a
mapfold job isn't the pre-computation of RPC for every key.  There's a synergy to be cultivated
here.

In a product-oriented company it should be easy to launch new services.  And some of those services
may want to trade between freshness and robustness.  In the event of a SEV, imagine being able to
replace a service with a read-only table that perfectly captures the semantics of the application.
When the platform is intended to blur the lines between RPC and the database, such things become
possible.

Lastly, our key-value store is designed to be serverless and scalable across multiple dimensions.
It should be possible for a small number of servers to serve all but the most demanding of
applications, but when that becomes necessary, there are no architectural bottlenecks that get in
the way of scaling.  At least in theory.

## protoql:  An OKVM

protoql is an object-key-value-mapper.  Specifically, it demonstrates that the right object
decomposition can give you most of the properties of a document store with most of the properties of
a key-value store.

The desired API is one that will enable applications like those I imagine buidling with blue.  To do
so, we provide a way to express plain old data (e.g. strings, integers, etc), records (e.g. structs
or groupings of plain old data), and maps from strings and integers to structs.  Concretely, this
looks very much like protocol buffers, the data interchange format used by Google.

That's the logic that led me to conclude that a row/record-oriented object layer built on protocol
buffers was the right abstraction.   Clients need to know only how to speak protocol buffers---I'd
wager it's one of the most richly supported interchange formats across a variety of languages.  That
opens the door to the question of how one models their storage.

One straw man approach that I would not recommend is to create separate tables for each type, tie
them together with foreign keys, and then try to scale that.  It can be done.  It has been done.
Many times.  But I don't recommend it because in a distributed setting it may mean that queries
exhibit a fan-out problem.  Consequently, it is desirable to colocate objects with an other object
with which they will be exclusively accessed.  Such colocation is a strict improvement as it is
always possible to create new anti-colocated objects and colocate the pointers to them.

Another straw man approach is to have a protocol buffers record per primary key, deserialize it,
update it, and reserialize it.  It can be done.  It has been done.  Many times.  But I don't
recommend it because it exhibits high write amplification and doesn't support large objects.

protoql enables object decomposition at the level of fields in structs and keys in dictionaries.
These serve as natural partition points for objects: Every object under a given field tautologically
is under that field forever, so we assign the key of the parent as a strict prefix of the key for a
child.  This means that, e.g., large objects become possible.  For a file sharing app this may mean
an unlimited number of files in a single directory instead of a mysterious limit, or glitches when
it extends beyond.  For a resource management UI, it means seamless pagination even for large map
objects without ever materializing the full object.  Because of the partitioning, a write in protoql
turns into a write to a single key, even when updating maps that may contain trillions of objects.

## FractalKV

Most key-value stores have an upper limit in size, constrained by the per-node size and the number
of nodes.  Last time I benchmarked RocksDB---the primary key-value store in use at Meta---it tops
out south of a terabyte, depending on the workload.  Which means the number of indexable partitions
in our exabyte-scale is on the order of 1e9.  If each partition takes more than a kilobyte, our
metadata for partitions exceeds the capabilities of a single paritition.  Realistically, a single
partition is not enough for metadata.

The solution is FractalKV, a recursive key-value store in which we work over a consistent key-value
interface.  Each layer of the recursion stores its metadata using the key-value store's interface as
its own metadata store.  Pluggable.  A single pointer for the root.  A key-value store on a single
machine in a single process.  A key value store replicated on few machines.  A key-value store on
zounds of machines.  Or, a serverless key-value store.  Each one storing its metadata in the
next-smallest store with desirable performance properties.

At the core of FractalKV is the notion of an sstable.  An sstable is a sorted string table of
key-value pairs; without the jargon, it is a set of key-value pairs that are consistently in some
order.  The smallest FractalKV is a single sstable---up to 1GiB.  Larger FractalKVs are a
composition of three things: One or more places to host sstables, a set of sstables, and metadata
describing how to arrange the sstables as a single, flat, ordered keyspace.

FractalKV is not the first key to being able to scale a key-value store past several petabytes.
There are many ways to do this.

## Triangular Compaction.

The second innovation that motivates blue is triangular compaction.  Triangular compaction solves
the problem of _write amplification_ in which each write becomes incrementally more expensive, until
the cost of the write is dominated not by the direct cost of the write itself, but the incidental,
indirect costs that accrue over time.  In a key-value store, write amplification is the ratio of
bytes written via the API to bytes written on disk.  If it takes 100GiB to write 10GiB, the write
amplification is 10x.

Most systems have worst-case write amplification that is 10x-100x.  The algorithm in use by blue has
write amplification that is bounded to 6.5x and with a practical extension that breaks the provable
upper bound, exhibits write amplification closer to 3x.

The key to triangular compaction lies in the way that most databases do writes.  In a traditional
database, you'd have what's called a B+-tree.  A B+-tree looks very much like if fingers and toes
had fingers and toes.  From the hand you can traverse to any one of five fingers and then from there
to any one of five finger tips.  A write will always progress from finger tip to hand.  It will
touch the interior finger about 1/5 of the time, and it will touch the hand 1/25 of the time.  The
write amplification, then, is the cost of writing to the finger tip, 1/5 the cost of writing to the
finger, and 1/25 the cost of writing to the hand.  Write amplification will shrink as the number of
fingers on each finger grows.

There's another source of write amplification, too:  Imagine if we could write microscopic messages
on the finger tip, but had to rewrite the entire finger tip in its entirety each time a message was
added or removed.  To write 1/4 of a finger-tip worth of messages requires finding the finger and
writing 4x the cost of the write.  Write amplification will grow as the data written shrinks in
relation to the finger tip's size.

To avoid these costs, one could imagine doing something different by starting from the hand.
Imagine if there were ants marching on our hand with fingers with fingers.  If we gave each ant a
message for each write and made them carry it from hand to the tip, we would have a different model
of execution in which writes always originate at the root and propagate downwards.  The key
advantage of this model is it can batch messages at the hand and have each ant carry a batch
of messages to the tip to amortize the cost of the ant trips

We can do even better.

Imagine distributing the ants so that some are carrying messages from hand to finger and some from
finger to tip.  Now, instead of having to wait for a batch of messages going to the same tip, we
wait for a batch of messages to the same finger---something five times more likely.  This is what a
log-structured-merge-tree or an LSM-tree will do to achieve higher write throughput than a B+-tree.
This cuts down on the second form of write amplification by amortizing the cost of writes.

The typical arrangement of an LSM-tree is to organize data in exponentially-growing buckets.  Data
moves from one level into the next in groups of keys.  If the cost of each write is invariant to
group size this has no effect, but in practice, batching in groups reduces the average cost across
all writes.  It does this by incrementally rewriting part of a bigger bucket with the contents of a
smaller bucket until some balance is achieved between the bucket sizes.  This process, termed
_compaction_, is where write amplification creeps into LSM-trees.  If the buckets are an order of
magnitude apart, and there are seven buckets, moving data between them will incur seven times a
single order of magnitude in write amplification to move a key from the smallest bucket to the
largest bucket.

Blue introduces a new form of compaction that cuts across the exponentially distributed buckets of
an LSM-tree to lower write amplification.  The core insight is to look for opportunities to compact
across levels so that it takes fewer compactions to move a file from the smallest bucket to the
largest bucket.

The concrete algorithm is implemented in blue and has an empirical write amplification of less than
three on a uniformly random workload.

## Not Consistent Hashing

One piece of research that I did for my thesis twice that I didn't publish was research in the area
of sharding.  Concretely, I had a simulation of a petabyte-scale system that exhibited the balance
properties of a distributed B+-tree with the rebalancing traffic of consistent hashing.  This is
innovative because it enables the range-scans that are necessary for protoql without compromising on
performance compared to traditional schemes.

The idea goes like this:  Subdivide the space into an order of magnitude more space than you have
servers.  Then, assign contiguous ranges of the space to the same server.  Servers are free to
re-subdivide this range any way they wish and pass ranges to adjacent servers for space or for load
reasons.  Re-subdividing the keyspace just requires rewriting every part of the LSM tree that
straddles the boundary---a constant number of files no matter the configuration.

At its core is the Gale-Shapley algorithm, typically used for arranging marriages and medical
students.  By forming a bipartite graph between old and new assignments and running the algorithm,
it becomes possible to find a minimal rebalancing that will transition from the old configuration to
the new one---no matter how many partial transfers take place and cause drift.

## Setsum

When dealing with data, checksums are key to detecting some data integrity issues.  The checksum in
blue extends to cover the entirety of the LSM-tree, no matter its size, without influencing
scalability significantly.  Every tree operation is O(1) checksum operations.  Consequently, blue
maintains checksums at all times, so that all data is covered by the checksum.

Yes, "the" checksum.  Each transaction has associated with it a single, all-encompassing checksum
that covers all of the data in the tree.  In this way, any mechanism that allows read snapshots of
the entire tree can be repurposed for validation where a scrub process can confirm the checksum of
the data matches.  In this way, the checksum gives end-to-end integrity properties.

But a single, end-to-end checksum doesn't say _where_ the corrupt data is.  A single bit over an
exabyte is both extremely useful and extremely useless, depending on the value of the bit.  So to
cover all cases, the system maintains a setsum for each sstable that is restricted to just that
sstable.  This localizes recovery efforts to finding the sstable with a bad checksum.

The key innovation behind setsum is that it composes:  Given two or more (multi)sets of data, the
union of their setsums is the setsum of their unions.  This union operator functions in O(1) time,
which means that, if the metadata for each file is tracked, the scrub operation can be done in two
pieces:  One piece that decomposes the global setsum into setsums per sstable, and one piece that
verifies individual sstable setsums.  The result is an algorithm amenable to divide-and-conquer
approaches and parallel execution.

Setsum was developed at Dropbox by the metadata team and is available to the public under the Apache
2.0 license.

## Tuples as Keys

The mapping between protocol buffers objects and key-value pairs maintained by protoql encodes each
object by its path from the root of the recursive structure.  Thus, the key for any possible piece
of data can be constructed by traversing from the root of the data structure through multiple maps
to an object.  The object need not be terminal; it may compose many more key-value pairs that form
its child objects.

Viewed another way, each key is a variable-length sequence---I call it a tuple after Python's tuple
type (not Rust's)---that conforms to some protoql schema.  Each member of the tuple may be a
different type, and tuples will be different lengths according to the data they index.

There are some important properties of tuple keys that we leverage in the implementation of blue:
Tuple keys are lexicographically ordered, so that keys with a shared prefix are grouped together in
the sort order.  This ensures that all the keys for an object will be collocated in storage.  It
also ensures that ranges of protoql objects---ranges of keys---can be scanned using sequential I/O
as well.  The buried lead is that you can sub-select any part of any object without paying for more
I/O than the amount of data you retrieve.  Further, tuple keys are strongly typed, so a tuple key
can be validated to uphold a particular schema.  All protobuf structures that blue supports (which
is most except, notably, oneof and groups) can be encoded as paths of string, int, and unit types.

## sst

sst is the crate that provides the sstable abstraction.  Recall that sstables are sorted string
tables---that is they are sorted by key and map each key to a value.  It has as at it's core a focus
on being very much like LevelDB's sstable abstraction, but with a format that is 100% protocol
buffers compatible.  That is to say that the on-disk format, headers, footers, and intermediary
structs are all defined as a concatenation of protocol buffers messages.  Further, every constant is
tuned so that there is no way for integer overflow to occur---all integer values fit i32 and u64
equally well.  This is out of sheer paranoia to make an easy-to-parse binary sstable format that
will (eventually) be easy to implement in every language.

To support sst-based applications, the sstable crate includes a set of primitives for cursoring over
sstables in all the ways one might need to do so to support protoql, mapfold, and tailog.

The goal of sst is to be the single interchange format for all things blue.  Have logs?  Put them in
sstables with protoql-compatible tuples for keys.  Have timeseries metrics?  Put them in sstables
encoded accordingly and be able to range scan the timestamps in the object at `map[label]
map[timestamp] point` to fetch all points for a range of time.  Writing a mapfold job?  Take as
input sstables and produce as output sstables.

sst natively supports setsum.

## saros

Saros is a prometheus-like query engine for timeseries data.  It was born out of a frustration with
every other time-series engine:  They are not a swiss army knife of tools; they are intended to be
run in their own way and are rigid.  Everything else is building to their way and it's frustratingly
rigid.

I want a flexible timeseries database that I can easily stand-up during a SEV.  I want to scrape
every host's /metrics with cURL into a file (one per host, let's say), and be able to stand up the
database against the metrics.  This is the recovery daemon for saros.  In the production service it
stores its data in sst.  But such a service needs to store its data somewhere and the recovery
format is sufficient.  And command-line-compatible.

I wrote a crate---biometrics_prometheus---that is compatible with the saros import format and the
Prometheus /metrics format.  I did one thing differently, however:  I made it write to disk so that
metrics are asynchronously propagated via S3 bucket rather than scraped via a prometheus scraper.
My intent was to be able to ingest _files_ into prometheus; a hack, for sure, but it would enable me
to get up and running without my own time series database and was much more amenable to the
architecture I need to engineer schizophrenia reliably.

What I found was that Prometheus and other time series databases I'd reasonably use are very much
set in their ways, and you have to use their ways to get things to work.  Exporting metrics into sst
was an intentional choice that enables something these other systems don't easily enable:  Feeding
metrics and logs back into an application so that it can take corrective action on a control signal.
Nowadays they call it, "agentic."

At the heart of saros is the assumption that the timeseries database should be just another table to
which all statistics about an application are written.  This just feels right.  To scale with a
small team (I designed blue for a team of three) you need to leverage every opportunity to pattern
match and produce harmony within your tools.  And this is one area ripe for that.  Feeding the
statistics into something that can be decoded via protoql just feels like the _right_ thing to do.
It opens up automation to work directly against the time series database.  Making it a first class
citizen, hosted on the same technology as the other key-value workloads is the only thing that makes
sense.

Too much of infrastructure gets given a pass for being below everything that it enables.

Why not fold the infrastructure over on itself and make it a fixed point in a principled fashion?

## indicio

I have a conjecture that the way we do structured logging is too unstructured.  People generally
interpret the term "structured logging" to mean something JSON or record-like, possibly nested.
When I hear structured logging, I think, "The same information must be represented exactly the same
in all contexts in which it occurs."  This is a corollary to the principle of maximum information
and the as-of-yet-unnamed principle of pattern matching: Making the string bytewise identical
confers speed advantages.

Why?  Imagine if you were writing a consensus protocol.  In a consensus protocol you agree upon a
sequence of values for index 0, 1, 2, and so on.  Each value is learned by everyone.  If, in log
messages you referred to this index consistently using the exact same object, you can then search
across log messages for the top objects causing trouble and paint a picture of their activity across
requests.  If you have an efficient top-k computation mechanism that finds the top-k sub-objects
across a range of time, then you have the ability to pinpoint the bulk of the traffic matching said
predicate.  Imagine having a single host's network go haywire and cause a cluster-wide problem when
every host queues up dropped packets to a hot key on said host.  With traditional tracing you'll see
this one host emerge.  With structured logging you'll see this emerge.  It's two sides of the same
coin, but no one builds tracing for an application to consume as its own signal.  Open telemetry,
despite being open, is largely an export-only format that you pay someone else to render for you.

indicio is my attempt at structured logging.  There's not a lot of structure right now.  I wrote a
macro that makes something that looks like regularized JSON and outputs binary structured log
messages.  It is decidedly not JSON for a good reason:  By using a custom type we can unilaterally
declare that every type must implement a conversion to the indicio value type and that no conversion
will ever convert personally-identifiable information (PII).  With a single conversion per higher
level type, the types will always appear the same in every sub expression that they encounter.  And
with the restriction on PII, the logging pipeline can treat all PII incidents as SEVs worthy of data
scrubbing.

Taking the cue from saros, the output format of indicio should be sst so that applications can be
written that consume their logs and give a better user experience than apps like honeycomb.io are
capable of providing.

## analogize

If indicio relies upon efficient top-k exemplar search in nested structured documents, then
analogize is about giving that functionality and giving the ability to query documents over time.

analogize operates on compressed log files to give top-k exemplar search.  Ironically, this makes
the problem easier, not harder.   The details are in a crate called scrunch, but it's basically an
entropy-compressed full-text search engine over which we run autoregression like a large language
model to "predict" the next-most-likely symbol.  Start and stop symbols encode each unique
sub-document object allowing the autoregression to have anchor points.  When a stop symbol that
matches the sequence's starting symbol is produced, the exemplar is returned.

The performance details of analogize are still under development and analysis.

Analogize relies upon the scrunch crate.  Scrunch is an implementation of a paper titled,
"High-order entropy-compressed text indexes" by Roberto Grossi, Ankur Gupta, and Jeffrey Scott
Vitter.  Scrunch implements full-text-search at the same time as compression.  The act of
compressing the document is the same as the act of indexing it for full-text search.  In
non-scientific experiments, an inverted index exhibits bloat to 950% its size while a scrunched
index over the same data exhibits a reduction to 40% the original size.

Further, while scrunch and analogize work together to give top-k exemplar search, other structures
are possible.  The delta between analogize and scrunch is an offset-to-timestamp datastructure that
operates in O(1) time, which means that if there are O(n) records that satisfy a search, it's O(n)
to construct the time series for that search, without having to search all of the data or retrieve
the complete record.  In theory the search cost is O(k log sigma) where k is the length of the
search text and sigma the alphabet.  In practice, I don't know yet.  Assuming this holds, a
timeseries would be reconstructible in O(n + k log sigma) operations where n is the number of
matching records.  The records themselves need not be materialized; it is sufficient to use their
offsets and the offset-to-timestamp data structure to produce a timeseries.

## tatl

"Hey!  Listen!"  It's usually Navi that's credited with this line, but it also comes from Tatl,
the assistant fairy that calls attention to the environment in The Legend of Zelda: Majora's Mask.
In blue, tatl is the alerting library.

Where most alerting libraries operate on a rollup of data, measuring the timeseries and alerting
when conditions are met, tatl operates directly in process, adjacent to the sensors collecting the
timeseries.  The advantage of doing this is it allows for higher cardinality metrics than is
possible with a traditional aggregate-and-query mechanism.

The difference lies in how tatl works:  Tatl monitors sensors for a variety of stable properties and
trips a condition when an alert should fire for a configurable reason.  This is useful for tracking
values that must remain stationary or otherwise not move.  The intended output is a log entry and a
time series for the firing metric.  Aggregation is then easy as there's no combinatorial querying
step for breaking down an aggregate into values by label.

That last bit is important.  When metrics get aggregated and then disaggregated for alerting,
information and fidelity is lost.  The principle of maximum information says to give yourself the
advantage.

## Significant Figures

In a time series database, histograms are useful for tracking, accurately, the distribution of a
value.  They provide an idea of the distribution.  Unfortunately, conventional histograms suffer
from a problem where larger values will occupy more space and smaller values will have lower
precision.  To counter this, OpenTelemetry and other projects ship exponential histograms---a way of
bucketing histograms so that larger values fall into larger buckets.  This means that fewer buckets
are necessary to represent a large range of values.

There's just one wart I identified with exponential histograms:  The bucket boundaries are
meaningless to humans.

To avoid this problem, sig_fig_histogram introduces an exponentially distributed histogram type that
is configurable to round to between two and four significant figures and use the rounded value as
the bucket label.  The result is an exponential histogram that also displays well on log-10 scales.
