macarunes
=========

This library provides an implementation of Macaroons.  For an introduction to
macaroons, [check out the paper](https://research.google/pubs/pub41892/), or
the [Python README](https://github.com/rescrv/libmacaroons/blob/master/README).

Quick Start
-----------

Mint a macaroon with a root secret, attenuate it with caveats, and verify it
with the same root secret plus the request context that satisfies those caveats.

```rust
use macarunes::{Macaroon, Secret, Verifier};

let secret = Secret::from_bytes([7; macarunes::SIGNATURE_BYTES]);
let mut macaroon = Macaroon::new(
    "https://issuer.example",
    "alice@example",
    secret.clone(),
);
macaroon.add_exact_string("role = admin");
macaroon.add_expires(4_102_444_800);

let verifier = Verifier::new()
    .with_context("role = admin")
    .with_current_time(1_700_000_000);

verifier.verify(&macaroon, &secret, &[])?;
# Ok::<_, macarunes::Error>(())
```

Why Macaroons?
--------------

Macaroons occupy the same practical space as cookies and bearer tokens: the
holder presents the token, and the receiver decides whether the token authorizes
the requested action.  The difference is that a macaroon can carry restrictions
that are added after it is minted.  That makes delegation safer.  A user can
hand a macaroon to another program after adding caveats that narrow it to one
account, one action, a short time window, or a proof from an authentication
service.

That gives macaroons several useful properties:

- Delegation can be confined to the context in which it should be valid.
- Attenuation is monotonic: holders can add caveats, but cannot remove caveats
  that have already been signed into the macaroon.
- Authorization proofs travel with the request, so the verifier does not need
  to keep a server-side copy of every attenuated macaroon.
- Third-party caveats let one service depend on another service's decision
  without putting that decision logic into the first service.
- The verifier stays small and reusable.  The policy lives in the macaroons
  that services mint and attenuate, while the verifier checks whether the proof
  and current request context satisfy that policy.

Core Ideas
----------

Macaroons are bearer credentials that can be attenuated.  A service mints a
root macaroon from a root secret and an identifier.  Whoever holds that
macaroon can add caveats that restrict where, when, or under which application
context it verifies.  Adding a caveat changes the macaroon signature, so a
receiver can detect tampering without keeping a copy of every macaroon it has
issued.

`macarunes` keeps the core language intentionally small:

- `Macaroon` carries a location hint, an identifier, a signature, and caveats.
- `Secret` holds 32 bytes of key material and scrubs its storage on drop.
- `Verifier` checks a root macaroon, its first-party caveats, and any discharge
  macaroons supplied for third-party caveats.
- `ThirdPartySecret` lets one service require proof from another service
  without revealing the discharge service's root secret.
- `RequestBuilder` and `Loader` assemble the root macaroon plus the transitive
  set of discharge macaroons a client should send with a request.

The caveat language has three cases:

- Exact-string caveats require a verifier context string with exactly the same
  bytes.
- Expiration caveats require `expiration > current_time`; equality is expired.
- Third-party caveats require a matching discharge macaroon, bound to the root
  macaroon for this request.

Installing
----------

Use the crate from this workspace or add it as a normal Rust dependency:

```toml
[dependencies]
macarunes = "0.11"
```

Secrets
-------

Use `Secret::random` when minting real credentials.  Use `Secret::from_bytes`
when loading configured key material, writing deterministic tests, or decoding a
secret from a protected storage system.

```rust
use macarunes::{Secret, SIGNATURE_BYTES};

let generated = Secret::random();
assert_eq!(SIGNATURE_BYTES * 2, generated.hexdigest().len());

let configured = Secret::from_bytes([0xab; SIGNATURE_BYTES]);
assert_eq!(
    "abababababababababababababababababababababababababababababababab",
    configured.hexdigest(),
);
```

`Secret` makes a best-effort attempt to scrub its internal memory when dropped.
That does not make logged, cloned, serialized, swapped, or otherwise copied
secrets safe.  Treat root secrets and discharge secrets as server-side keys.
The fixed width of `Secret` is intentional: use all `SIGNATURE_BYTES` bytes, and
draw them from a cryptographically strong random source.  Short, human-readable,
or predictable secrets make it easier to forge macaroons.

Minting a Root Macaroon
-----------------------

A root macaroon starts with a public location, a public identifier, and a secret
known to the service that will later verify it.

```rust
use macarunes::{Macaroon, Secret};

let root_secret = Secret::from_bytes([1; macarunes::SIGNATURE_BYTES]);
let macaroon = Macaroon::new(
    "https://files.example/macaroons",
    "file:alpha",
    root_secret,
);

assert_eq!("https://files.example/macaroons", macaroon.location());
assert_eq!("file:alpha", macaroon.identifier());
assert!(!macaroon.has_caveats());
```

The location is a routing hint.  It is intentionally not protected by the
macaroon signature, so do not use it as an authority decision.  The identifier
is the stable public value the issuing service uses to find or derive the root
secret needed for verification.  Common identifier strategies include a database
key, an encrypted record that only the issuer can open, or a structured name
that is enough to deterministically derive the secret.  The identifier can be
visible to anyone who holds the macaroon; it must not reveal the root secret.

The signature is also present in the macaroon.  It changes when caveats are
added and becomes the key material used for later attenuation.  In application
code, treat possession of a macaroon or its signature as possession of the
credential.

First-Party Caveats
-------------------

First-party caveats are checked directly by the final verifier.  They are useful
for request facts that the verifier can compute locally, such as method, path,
tenant, role, or deadline.

```rust
use macarunes::{Error, Macaroon, Secret, Verifier};

let secret = Secret::from_bytes([2; macarunes::SIGNATURE_BYTES]);
let mut macaroon = Macaroon::new("https://issuer.example", "alice", secret.clone());
macaroon.add_exact_string("method = GET");
macaroon.add_exact_string("path = /v1/accounts/alice");
macaroon.add_expires(1_900_000_000);

let accepted = Verifier::new()
    .with_context("method = GET")
    .with_context("path = /v1/accounts/alice")
    .with_current_time(1_800_000_000);
assert_eq!(Ok(()), accepted.verify(&macaroon, &secret, &[]));

let rejected = Verifier::new()
    .with_context("method = POST")
    .with_context("path = /v1/accounts/alice")
    .with_current_time(1_800_000_000);
assert_eq!(
    Err(Error::ProofInvalid),
    rejected.verify(&macaroon, &secret, &[]),
);
```

Exact-string caveats are not parsed by the library.  Define your own canonical
format, normalize request facts before adding them to the verifier, and keep
those strings stable across services.

Verifier Design
---------------

Build the verifier from facts about the request, not by inspecting the macaroon
and trying to make its caveats pass.  For example, an HTTP service can add facts
like `method = GET`, `path = /v1/accounts/alice`, `user = alice`,
`account = 3735928559`, and `action = deposit` for every request.  A macaroon
that mentions any subset of those true facts will verify; a macaroon that adds
an unknown or false fact will not.

This keeps authorization policy decoupled from enforcement.  You can mint a new
macaroon with a narrower policy without changing the verifier, provided the
verifier already knows how to state the relevant request facts.  This crate does
not expose libmacaroons-style general callback caveats.  Use exact strings for
canonical request facts and `add_expires` for the built-in time predicate.  If
you need a richer predicate, evaluate it in your application and add the
resulting canonical fact to the verifier context; or, use third-party caveats.

Expiration Caveats
------------------

`add_expires` takes an unsigned integer timestamp.  The verifier accepts the
macaroon only when the expiration is strictly greater than the verifier's
current time.

```rust
use macarunes::{Error, Macaroon, Secret, Verifier};

let secret = Secret::from_bytes([3; macarunes::SIGNATURE_BYTES]);
let mut macaroon = Macaroon::new("https://issuer.example", "alice", secret.clone());
macaroon.add_expires(1_700_000_000);

assert_eq!(
    Ok(()),
    Verifier::new()
        .with_current_time(1_699_999_999)
        .verify(&macaroon, &secret, &[]),
);
assert_eq!(
    Err(Error::ProofInvalid),
    Verifier::new()
        .with_current_time(1_700_000_000)
        .verify(&macaroon, &secret, &[]),
);
```

What Verification Rejects
-------------------------

`Verifier::verify` succeeds only when all of these conditions hold:

- The root secret matches the root macaroon identifier.
- Every first-party caveat is satisfied by the verifier context or current time.
- Every third-party caveat has a matching discharge macaroon.
- Every discharge macaroon verifies under the secret hidden in the third-party
  caveat.
- Every discharge has been bound to the root macaroon for this request.
- The final signatures match after replaying the caveat chain.

The verifier deliberately collapses most proof failures into
`Error::ProofInvalid`.  For authorization decisions, treat `ProofInvalid` as
"not authorized" rather than trying to distinguish wrong secrets, missing
contexts, expired caveats, tampering, or missing discharges.

Third-Party Caveats
-------------------

A third-party caveat lets the root service require a second service to issue a
discharge macaroon.  The root service does not learn the discharge service's
secret.  The discharge service receives the current root macaroon signature,
wraps its discharge secret in a `ThirdPartySecret`, and gives that value back to
the root service.  The root service then embeds the third-party caveat.

The usual flow is:

1. The root service decides that a request should require a third-party proof.
2. The third party chooses or records the predicate it will enforce, such as
   `user = alice`.
3. The third party creates a discharge secret and a public identifier that lets
   it recover the predicate and discharge secret later.
4. The root service embeds the public location, public identifier, and
   `ThirdPartySecret` in the root macaroon.
5. The client presents the location and identifier to the third party.
6. The third party checks its predicate and returns a discharge macaroon.
7. The client binds the discharge to the root macaroon and sends both to the
   verifier.

The root macaroon does not need to reveal the predicate that the third party
checked.  The identifier may be an opaque handle into the third party's storage.
That lets the final verifier accept the third-party proof without knowing the
third party's internal policy, user database, or authentication protocol.

```rust
use macarunes::{Macaroon, Secret, ThirdPartySecret, Verifier};

const ROOT_LOCATION: &str = "https://files.example/macaroons";
const AUTH_LOCATION: &str = "https://auth.example/discharges";

let root_secret = Secret::from_bytes([4; macarunes::SIGNATURE_BYTES]);
let auth_secret = Secret::from_bytes([5; macarunes::SIGNATURE_BYTES]);

let mut root = Macaroon::new(ROOT_LOCATION, "file:alpha", root_secret.clone());
let third_party_secret = ThirdPartySecret::random(root.signature(), &auth_secret);
root.add_third_party_caveat(
    AUTH_LOCATION,
    "authz:alice:file:alpha",
    third_party_secret,
);

let mut discharge = Macaroon::new(
    AUTH_LOCATION,
    "authz:alice:file:alpha",
    auth_secret,
);
discharge.add_exact_string("user = alice");

root.bind_discharge(&mut discharge);

let verifier = Verifier::new().with_context("user = alice");
verifier.verify(&root, &root_secret, &[discharge])?;
# Ok::<_, macarunes::Error>(())
```

Binding matters.  A discharge macaroon must be bound to the root macaroon before
verification so that a discharge created for one request cannot be replayed with
another root macaroon.  If you assemble discharges manually, call
`root.bind_discharge(&mut discharge)` for every discharge.  If you use
`RequestBuilder`, it performs this binding for you.

The upstream libmacaroons guide also describes a public-key variant in which the
third-party identifier can carry encrypted caveat material instead of requiring a
round trip to create an identifier.  `macarunes` does not provide that public-key
scheme directly.  You can still build a protocol with the same shape by making
the identifier an opaque ciphertext or lookup key understood by your
third-party service, then issuing a normal `Macaroon` as the discharge.

Preparing Requests with Loaders
-------------------------------

`RequestBuilder` is the client-side helper for request assembly.  Register one
`Loader` per location.  The builder loads the root macaroon, recursively follows
third-party caveats, loads each required discharge macaroon, and binds all
discharges to the root before returning them.

```rust
use macarunes::{
    Error, Loader, Macaroon, RequestBuilder, Secret, ThirdPartySecret, Verifier,
};

const ROOT_LOCATION: &str = "https://files.example/macaroons";
const AUTH_LOCATION: &str = "https://auth.example/discharges";

#[derive(Debug)]
struct StaticLoader {
    location: &'static str,
    macaroons: Vec<Macaroon>,
}

impl StaticLoader {
    fn new(location: &'static str, macaroons: Vec<Macaroon>) -> Self {
        Self { location, macaroons }
    }
}

impl Loader for StaticLoader {
    fn location(&self) -> &'static str {
        self.location
    }

    fn lookup(&self, identifier: &str) -> Result<Macaroon, Error> {
        self.macaroons
            .iter()
            .find(|macaroon| macaroon.identifier() == identifier)
            .cloned()
            .ok_or_else(|| Error::MissingLoader {
                what: format!("{}/{}", self.location, identifier),
            })
    }
}

let root_secret = Secret::from_bytes([6; macarunes::SIGNATURE_BYTES]);
let auth_secret = Secret::from_bytes([7; macarunes::SIGNATURE_BYTES]);

let mut root = Macaroon::new(ROOT_LOCATION, "file:alpha", root_secret.clone());
let third_party_secret = ThirdPartySecret::random(root.signature(), &auth_secret);
root.add_third_party_caveat(
    AUTH_LOCATION,
    "authz:alice:file:alpha",
    third_party_secret,
);

let mut discharge = Macaroon::new(
    AUTH_LOCATION,
    "authz:alice:file:alpha",
    auth_secret,
);
discharge.add_exact_string("user = alice");

let builder = RequestBuilder::new()
    .with_loader(StaticLoader::new(ROOT_LOCATION, vec![root]))
    .with_loader(StaticLoader::new(AUTH_LOCATION, vec![discharge]));

let (request_root, request_discharges) =
    builder.prepare_request(ROOT_LOCATION, "file:alpha")?;

Verifier::new()
    .with_context("user = alice")
    .verify(&request_root, &root_secret, &request_discharges)?;
# Ok::<_, macarunes::Error>(())
```

Production loaders usually wrap an RPC client, local cache, database lookup, or
service-discovery layer.  A loader should return macaroons for exactly one
location.  `RequestBuilder` rejects a macaroon if the loader returns a different
location than the one requested.

Selecting Discharges from a Cache
---------------------------------

Use `Macaroon::covering_set` when a client already has many candidate discharge
macaroons and only needs to select the transitive set referenced by a root
macaroon.  Selection uses public location and identifier data only.  It does not
verify signatures, decrypt third-party secrets, satisfy caveats, or bind
unbound discharges.

```rust
use macarunes::{Macaroon, Secret, ThirdPartySecret, Verifier};

const ROOT_LOCATION: &str = "https://files.example/macaroons";
const AUTH_LOCATION: &str = "https://auth.example/discharges";

let root_secret = Secret::from_bytes([8; macarunes::SIGNATURE_BYTES]);
let auth_secret = Secret::from_bytes([9; macarunes::SIGNATURE_BYTES]);

let mut root = Macaroon::new(ROOT_LOCATION, "file:alpha", root_secret.clone());
let third_party_secret = ThirdPartySecret::random(root.signature(), &auth_secret);
root.add_third_party_caveat(
    AUTH_LOCATION,
    "authz:alice:file:alpha",
    third_party_secret,
);

let mut discharge = Macaroon::new(
    AUTH_LOCATION,
    "authz:alice:file:alpha",
    auth_secret,
);
root.bind_discharge(&mut discharge);

let noise = Macaroon::new(
    "https://other.example/discharges",
    "unrelated",
    Secret::from_bytes([10; macarunes::SIGNATURE_BYTES]),
);
let candidates = vec![noise, discharge];
let selected = root.covering_set(&candidates)?;

assert_eq!(1, selected.len());
Verifier::new().verify(&root, &root_secret, &selected)?;
# Ok::<_, macarunes::Error>(())
```

Use `covering_set_refs` when you want references into the candidate slice
instead of cloned macaroons.

Serialization
-------------

`Macaroon`, `Secret`, and `ThirdPartySecret` implement the repository's
`prototk` message traits.  Use `buffertk` to encode and decode them for storage
or transport.

```rust
use buffertk::{stack_pack, Unpacker};
use macarunes::{Macaroon, Secret};

let secret = Secret::from_bytes([11; macarunes::SIGNATURE_BYTES]);
let mut macaroon = Macaroon::new("https://issuer.example", "alice", secret);
macaroon.add_exact_string("role = admin");

let bytes = stack_pack(&macaroon).to_vec();
let mut unpacker = Unpacker::new(&bytes);
let decoded: Macaroon = unpacker.unpack().expect("macaroon decodes");

assert!(unpacker.is_empty());
assert_eq!(macaroon, decoded);
```

The encoded bytes are suitable for structured storage or transport protocols
that already carry bytes.  If you need to place a macaroon in a cookie, header,
URL, or email body, wrap the encoded bytes in an ASCII-safe envelope such as
base64 and decode that envelope before calling `Unpacker`.

Verification Checklist
----------------------

On the service that verifies a request:

1. Recover or derive the root secret for the root macaroon identifier.
2. Build a `Verifier`.
3. Add every exact-string context that should be true for this request.
4. Set the verifier time before checking expiration caveats.
5. Pass the root macaroon, root secret, and all bound discharges to
   `Verifier::verify`.
6. Treat every `Error::ProofInvalid` as authentication failure.

On the client that prepares a request:

1. Start with the root macaroon the target service gave you.
2. Fetch every required discharge macaroon, including nested discharges.
3. Bind every discharge to the root macaroon, or use `RequestBuilder`.
4. Send the root macaroon and the bound discharge list together.

Errors
------

The public error type is `macarunes::Error`:

- `ProofInvalid` means the proof failed: wrong secret, missing context, expired
  caveat, missing discharge, unbound discharge, tampering, or any other
  verification failure.
- `Cycle` means verification detected recursive discharge structure deeper than
  the supplied discharge set.
- `MissingLoader` means `RequestBuilder` has no loader for a location, or a
  loader could not find the requested macaroon.
- `LocationMismatch` means a loader returned a macaroon whose location differs
  from the requested loader location.
- `MissingMacaroon` means `covering_set` could not find a candidate with the
  public location and identifier required by a third-party caveat.

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

About Locations
---------------

A location is a hint that is not part of the macaroon's signature.  The library
does this intentionally as the paper suggests that only keys speak.

For that reason, locations should be treated as hints that give a plain-text
description of how to use the endpoint.  The location in the macaroon is not the
endpoint for discharge; it is an opaque identifier that the `RequestBuilder`
will iterate over.

For each third party, a `Loader` should be developed that negotiates the
protocol to get discharge macaroons.  A location that is not known to a client
cannot be trusted.  Given that we have to trust servers that give us macaroons,
this is not a compromise or limitation.

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

The latest documentation is always available at [docs.rs](https://docs.rs/macarunes/latest/macarunes/).
