macarunes
=========

This library provides an implementation of Macaroons.  For an introduction to
macaroons, [check out the paper](https://research.google/pubs/pub41892/), or
the [Python README](https://github.com/rescrv/libmacaroons/blob/master/README).

Assumptions
-----------

- We rely upon the type system to provide memory safety.  Secrets are scrubbed
  after use, but there's no guarantee the linker won't optimize away such
  code.  Until an end-to-end solution emerges in the Rust compiler, it won't
  be possible to guarantee secrets don't leak in the presence of memory
  vulnerability.

- This library relies upon the determinism of the protocol buffers code.  This
  is guaranteed by prototk.

- Intentionally restrictive language compared to the Python implementation.
  The only cases you need in the core library are to make exact comparisons
  (which are born out as format strings), to set an expiration, or to use
  a third-party caveat that enforces some arbitrary predicate before the
  discharge macaroon is granted.

Third-Party Macaroons
---------------------

This is the algorithm for satisfying third-party secrets:

1) Obtain `signature` from the macaroon to which we want to add a third-party
   caveat.
2) Make an RPC to the third-party that exchanges `signature` for a
   `ThirdPartySecret`.
3) Call `Macaroon.add_third_party(location, identifier, third_party_secret)`.

About Locations
---------------

A location is a hint that is not part of the macaroons's signature.  The
library does this intentionally as the paper suggests that only keys speak.

For that reason, locations should be treated as hints that give a plain-text
description of how to use the endpoint.  The location in the macaroon is not
the endpoint for discharge; it's a opaque identifier that the `RequestBuilder`
will iterate over.

For each third party, a Loader should be developed that negotiates the
protocol to get discharge macaroons.  A location that's not known to a client
cannot be trusted.  Given that we have to trust servers that give us
macaroons, this is not a compromise or limitation.

Status
------

Active development.

Scope
-----

This library should provide a verifier and a client-side library.

Warts
-----

- This library is under-used and will see active development in the future.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/buffertk/latest/buffertk/).
