sig_fig_histogram
=================

This library provides an exponentially distributed histogram type.  This is desirable for two reasons.  First, it
provides the [advantages of exponential histograms](https://newrelic.com/blog/best-practices/opentelemetry-histograms).
Second, it provides a human-readable set of buckets that are congruent to displaying on a log10 scale.  Humans don't
operate on log2.  Except maybe SREs :-).

Status
------

Maintenance track.  The library is considered stable and will be put into maintenance mode if unchanged for one year.

It is new, however, so the clock will likely reset if problems are discovered.

Scope
-----

This library provides the bucket functions, an unbounded histogram, and a lock-free histogram.

Warts
-----

None.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/sig_fig_histogram/latest/sig_fig_histogram/).

#histogram #instrumentation
