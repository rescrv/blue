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
)?;
macaroon.add_fact("role", "admin")?;
macaroon.add_expires(4_102_444_800)?;

let verifier = Verifier::new()
    .with_fact("role", "admin")
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
- `PreparedRequest` carries a root macaroon plus the bound discharge macaroons
  a client should send with a request.
- `RequestBuilder`, `AsyncRequestBuilder`, and loaders assemble prepared
  requests from root and discharge locations.

The caveat language has four cases:

- Exact-string caveats require a verifier context string with exactly the same
  bytes.
- Expiration caveats require a verifier current time and `expiration >
  current_time`; equality is expired.
- Not-before caveats require a verifier current time and `current_time >=
  not_before`; equality is valid.
- Third-party caveats require a matching discharge macaroon, bound to the root
  macaroon for this request.

Installing
----------

Use the crate from this workspace or add it as a normal Rust dependency:

```toml
[dependencies]
macarunes = "0.11"
```

Enable the optional `base64` feature if you want the `to_base64` and
`from_base64` convenience helpers:

```toml
[dependencies]
macarunes = { version = "0.11", features = ["base64"] }
```

Secrets
-------

Use `Secret::random` when minting real credentials.  Use `Secret::from_base64`,
`Secret::from_hex`, or `Secret::try_from_slice` when loading configured key
material from a protected storage system.  Use `Secret::from_bytes` for
deterministic tests and lower-level integrations.  The Base64 helpers require
the `base64` feature.

```rust
# #[cfg(feature = "base64")]
# fn run() -> Result<(), macarunes::Error> {
use macarunes::Secret;

let generated = Secret::random()?;
assert_eq!(43, generated.to_base64().len());

let configured = Secret::from_base64(
    "q6urq6urq6urq6urq6urq6urq6urq6urq6urq6urq6s",
)?;
assert_eq!(
    "abababababababababababababababababababababababababababababababab",
    configured.hexdigest(),
);
# Ok(())
# }
# #[cfg(feature = "base64")]
# run()?;
# Ok::<_, macarunes::Error>(())
```

`to_base64` and `hexdigest` return raw key material.  Keep them for controlled
configuration, diagnostics, and deterministic tests; do not log them for live
credentials.  `Debug` for `Secret` is intentionally redacted.

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
)?;

assert_eq!("https://files.example/macaroons", macaroon.location());
assert_eq!("file:alpha", macaroon.identifier());
assert!(!macaroon.has_caveats());
# Ok::<_, macarunes::Error>(())
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
let mut macaroon = Macaroon::new("https://issuer.example", "alice", secret.clone())?;
macaroon.add_fact("method", "GET")?;
macaroon.add_fact("path", "/v1/accounts/alice")?;
macaroon.add_expires(1_900_000_000)?;

let accepted = Verifier::new()
    .with_fact("method", "GET")
    .with_fact("path", "/v1/accounts/alice")
    .with_current_time(1_800_000_000);
assert_eq!(Ok(()), accepted.verify(&macaroon, &secret, &[]));

let rejected = Verifier::new()
    .with_fact("method", "POST")
    .with_fact("path", "/v1/accounts/alice")
    .with_current_time(1_800_000_000);
assert_eq!(
    Err(Error::ProofInvalid),
    rejected.verify(&macaroon, &secret, &[]),
);
# Ok::<_, macarunes::Error>(())
```

Exact-string caveats are not parsed by the library.  Define your own canonical
format, normalize request facts before adding them to the verifier, and keep
those strings stable across services.  For the common `name = value` shape, use
`add_fact` and `with_fact`; the fact name must be `&'static str` so application
code controls the fact vocabulary.

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
not expose libmacaroons-style general callback caveats.  Use exact strings or
`add_fact` for canonical request facts, and use `add_not_before` and `add_expires` for
verifier-time bounds.  If you need a richer predicate, evaluate it in your
application and add the resulting canonical fact to the verifier context; or,
use third-party caveats.

Expiration Caveats
------------------

`add_expires` takes an unsigned integer timestamp.  The verifier accepts the
macaroon only when the expiration is strictly greater than the verifier's
current time.  An expiration caveat fails if verifier time has not been set.
Use `add_expires_at` when you already have a `SystemTime`, and use `add_ttl`
when issuing a macaroon relative to the current system time.  `add_ttl_from`
does the same calculation from an explicit `SystemTime`, which is useful for
tests and request-scoped clocks.

```rust
use macarunes::{Error, Macaroon, Secret, Verifier};

let secret = Secret::from_bytes([3; macarunes::SIGNATURE_BYTES]);
let mut macaroon = Macaroon::new("https://issuer.example", "alice", secret.clone())?;
macaroon.add_expires(1_700_000_000)?;

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
# Ok::<_, macarunes::Error>(())
```

Calling `add_ttl` or `add_ttl_from` appends a new expiration caveat.  It does
not remove or rewrite earlier caveats.  Verification requires every expiration
caveat to hold, so the effective expiration is the minimum of all expiration
caveats.

```rust
use std::time::{Duration, UNIX_EPOCH};
use macarunes::{Error, Macaroon, Secret, Verifier};

let secret = Secret::from_bytes([13; macarunes::SIGNATURE_BYTES]);
let mut macaroon = Macaroon::new("https://issuer.example", "alice", secret.clone())?;
macaroon.add_expires(200)?;
macaroon.add_ttl_from(
    UNIX_EPOCH + Duration::from_secs(100),
    Duration::from_secs(50),
)?;

assert_eq!(
    Ok(()),
    Verifier::new()
        .with_current_time(149)
        .verify(&macaroon, &secret, &[]),
);
assert_eq!(
    Err(Error::ProofInvalid),
    Verifier::new()
        .with_current_time(150)
        .verify(&macaroon, &secret, &[]),
);
# Ok::<_, macarunes::Error>(())
```

Not-Before Caveats
------------------

`add_not_before` takes an unsigned integer timestamp.  The verifier accepts the
macaroon only when the verifier's current time is greater than or equal to that
timestamp.  Equality is valid.  A not-before caveat fails if verifier time has
not been set.

This is the intrinsic caveat to use when an issued credential should place an
inclusive lower bound on the verification time.  Use `add_not_before_at` to add
the caveat from a `SystemTime`, and use `Verifier::with_current_system_time` or
`Verifier::with_system_time_now` when building a verifier from system time.

```rust
use macarunes::{Error, Macaroon, Secret, Verifier};

let secret = Secret::from_bytes([12; macarunes::SIGNATURE_BYTES]);
let mut macaroon = Macaroon::new("https://issuer.example", "alice", secret.clone())?;
macaroon.add_not_before(1_700_000_000)?;

assert_eq!(
    Err(Error::ProofInvalid),
    Verifier::new()
        .with_current_time(1_699_999_999)
        .verify(&macaroon, &secret, &[]),
);
assert_eq!(
    Ok(()),
    Verifier::new()
        .with_current_time(1_700_000_000)
        .verify(&macaroon, &secret, &[]),
);
# Ok::<_, macarunes::Error>(())
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
contexts, expired caveats, tampering, or missing discharges.  For controlled
diagnostics, log the `Debug` output of the macaroon and verifier together:
`Macaroon` shows public location, identifier, and caveats while redacting the
signature, and `Verifier` shows the request context and verifier time.

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

Use `ThirdPartySecret::random` for production third-party caveats.  If a
protocol has to provide nonce material explicitly, use `Nonce::from_bytes`;
nonce bytes are public, but they must be unique for a given macaroon signature.

```rust
use macarunes::{Macaroon, Secret, ThirdPartySecret, Verifier};

const ROOT_LOCATION: &str = "https://files.example/macaroons";
const AUTH_LOCATION: &str = "https://auth.example/discharges";

let root_secret = Secret::from_bytes([4; macarunes::SIGNATURE_BYTES]);
let auth_secret = Secret::from_bytes([5; macarunes::SIGNATURE_BYTES]);

let mut root = Macaroon::new(ROOT_LOCATION, "file:alpha", root_secret.clone())?;
let third_party_secret = ThirdPartySecret::random(root.signature(), &auth_secret)?;
root.add_third_party_caveat(
    AUTH_LOCATION,
    "authz:alice:file:alpha",
    third_party_secret,
)?;

let mut discharge = Macaroon::new(
    AUTH_LOCATION,
    "authz:alice:file:alpha",
    auth_secret,
)?;
discharge.add_exact_string("user = alice")?;

root.bind_discharge(&mut discharge)?;

let verifier = Verifier::new().with_context("user = alice");
verifier.verify(&root, &root_secret, &[discharge])?;
# Ok::<_, macarunes::Error>(())
```

Binding matters.  A discharge macaroon must be bound to the root macaroon before
verification so that a discharge created for one request cannot be replayed with
another root macaroon.  If you assemble discharges manually, call
`root.bind_discharge(&mut discharge)` for one mutable discharge,
`root.bind_discharge_owned(discharge)` when you want to consume and return a
bound discharge, or `root.bind_discharges(&mut discharges)` for a whole slice.
If you use `RequestBuilder`, it performs this binding for you.

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

Loader locations are static trusted code capabilities: `Loader::location`
returns `&'static str`, and `RequestBuilder` is keyed by those static strings.
The public locations embedded in macaroons remain untrusted routing hints.  They
select among discharge mechanisms already registered in code; they do not create
new mechanisms from macaroon data.

```rust
use macarunes::{
    Macaroon, RequestBuilder, Secret, StaticLoader, ThirdPartySecret, Verifier,
};

const ROOT_LOCATION: &str = "https://files.example/macaroons";
const AUTH_LOCATION: &str = "https://auth.example/discharges";

let root_secret = Secret::from_bytes([6; macarunes::SIGNATURE_BYTES]);
let auth_secret = Secret::from_bytes([7; macarunes::SIGNATURE_BYTES]);

let mut root = Macaroon::new(ROOT_LOCATION, "file:alpha", root_secret.clone())?;
let third_party_secret = ThirdPartySecret::random(root.signature(), &auth_secret)?;
root.add_third_party_caveat(
    AUTH_LOCATION,
    "authz:alice:file:alpha",
    third_party_secret,
)?;

let mut discharge = Macaroon::new(
    AUTH_LOCATION,
    "authz:alice:file:alpha",
    auth_secret,
)?;
discharge.add_exact_string("user = alice")?;

let builder = RequestBuilder::new()
    .with_loader(StaticLoader::new(ROOT_LOCATION, vec![root]))?
    .with_loader(StaticLoader::new(AUTH_LOCATION, vec![discharge]))?;

let request = builder.prepare(ROOT_LOCATION, "file:alpha")?;

Verifier::new()
    .with_context("user = alice")
    .verify_request(&request, &root_secret)?;
# Ok::<_, macarunes::Error>(())
```

Production loaders usually wrap an RPC client, local cache, database lookup, or
service-discovery layer.  A loader should return macaroons for exactly one
location.  `RequestBuilder` rejects a macaroon if the loader returns a different
location than the one requested, and it rejects duplicate loader registration
for the same static location.

The crate provides small loader affordances for common cases:

- `StaticLoader` scans a vector and is convenient for examples and tests.
- `MapLoader` stores macaroons by identifier and checks locations on insert.
- `FnLoader` and `RequestBuilder::with_lookup` adapt a closure into a loader.

`RequestBuilder::prepare` returns a `PreparedRequest` with named `root` and
`discharges` fields.  `prepare_request` remains available when you want the old
`(Macaroon, Vec<Macaroon>)` shape.

Use `AsyncRequestBuilder` with `AsyncLoader` when a loader naturally performs
RPC, database, or cache work asynchronously.  `StaticLoader`, `MapLoader`, and
`FnLoader` also work with `AsyncRequestBuilder`; for native async closures, use
`AsyncFnLoader` or `AsyncRequestBuilder::with_lookup`.

```rust
use macarunes::{AsyncRequestBuilder, Macaroon, Secret, StaticLoader};

const ROOT_LOCATION: &str = "https://files.example/macaroons";

# async fn example() -> Result<(), macarunes::Error> {
let root_secret = Secret::from_bytes([14; macarunes::SIGNATURE_BYTES]);
let root = Macaroon::new(ROOT_LOCATION, "file:alpha", root_secret)?;
let builder = AsyncRequestBuilder::new()
    .with_loader(StaticLoader::new(ROOT_LOCATION, vec![root]))?;

let request = builder.prepare(ROOT_LOCATION, "file:alpha").await?;
assert_eq!("file:alpha", request.root.identifier());
# Ok(())
# }
```

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

let mut root = Macaroon::new(ROOT_LOCATION, "file:alpha", root_secret.clone())?;
let third_party_secret = ThirdPartySecret::random(root.signature(), &auth_secret)?;
root.add_third_party_caveat(
    AUTH_LOCATION,
    "authz:alice:file:alpha",
    third_party_secret,
)?;

let mut discharge = Macaroon::new(
    AUTH_LOCATION,
    "authz:alice:file:alpha",
    auth_secret,
)?;
root.bind_discharge(&mut discharge)?;

let noise = Macaroon::new(
    "https://other.example/discharges",
    "unrelated",
    Secret::from_bytes([10; macarunes::SIGNATURE_BYTES]),
)?;
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

`Macaroon`, `PreparedRequest`, `Secret`, and `ThirdPartySecret` implement the
repository's `prototk` message traits.  Use `Macaroon::to_bytes` and
`Macaroon::from_bytes` to encode and decode macaroons for structured storage or
transport.  `from_bytes` rejects trailing bytes.

```rust
use macarunes::{Macaroon, Secret};

let secret = Secret::from_bytes([11; macarunes::SIGNATURE_BYTES]);
let mut macaroon = Macaroon::new("https://issuer.example", "alice", secret)?;
macaroon.add_exact_string("role = admin")?;

let bytes = macaroon.to_bytes();
let decoded = Macaroon::from_bytes(&bytes)?;

assert_eq!(macaroon, decoded);
# Ok::<_, macarunes::Error>(())
```

Use `to_base64` and `from_base64` for cookies, headers, URLs, email bodies, or
configuration systems that need ASCII text.  The encoding is URL-safe Base64
without padding; decoders accept padded URL-safe or standard Base64 input.
These helpers require the `base64` feature.

```rust
# #[cfg(feature = "base64")]
# fn run() -> Result<(), macarunes::Error> {
use macarunes::{Macaroon, PreparedRequest, Secret};

let secret = Secret::from_bytes([15; macarunes::SIGNATURE_BYTES]);
let macaroon = Macaroon::new("https://issuer.example", "alice", secret)?;

let encoded = macaroon.to_base64();
let decoded = Macaroon::from_base64(&encoded)?;
assert_eq!(macaroon, decoded);

let request = PreparedRequest::new(macaroon, Vec::new());
let encoded_request = request.to_base64();
assert_eq!(request, PreparedRequest::from_base64(&encoded_request)?);
# Ok(())
# }
# #[cfg(feature = "base64")]
# run()?;
# Ok::<_, macarunes::Error>(())
```

Verification Checklist
----------------------

On the service that verifies a request:

1. Recover or derive the root secret for the root macaroon identifier.
2. Build a `Verifier`.
3. Add every exact-string context or typed fact that should be true for this
   request.
4. Set the verifier time before checking expiration or not-before caveats.
5. Pass the root macaroon, root secret, and all bound discharges to
   `Verifier::verify`, or pass a `PreparedRequest` to `Verifier::verify_request`.
6. Treat every `Error::ProofInvalid` as authentication failure.

On the client that prepares a request:

1. Start with the root macaroon the target service gave you.
2. Fetch every required discharge macaroon, including nested discharges.
3. Bind every discharge to the root macaroon, or use `RequestBuilder` /
   `AsyncRequestBuilder`.
4. Send the root macaroon and the bound discharge list together, often as a
   serialized `PreparedRequest`.

Errors
------

The public error type is `macarunes::Error`:

- `ProofInvalid` means the proof failed: wrong secret, missing context, expired
  or not-yet-valid caveat, missing discharge, unbound discharge, tampering, or
  any other verification failure.
- `Cycle` means verification detected recursive discharge structure deeper than
  the supplied discharge set.
- `MissingLoader` means `RequestBuilder` has no loader for a location, or a
  loader could not find the requested macaroon.
- `LocationMismatch` means a loader returned a macaroon whose location differs
  from the requested loader location.
- `MissingMacaroon` means `covering_set` could not find a candidate with the
  public location and identifier required by a third-party caveat.
- `DuplicateLoader` means `RequestBuilder` already has a loader registered for
  that static location.
- `InvalidEncoding` means `Macaroon::from_bytes` or
  `PreparedRequest::from_bytes` could not decode exactly one value from the
  provided bytes.
- `RandomGenerationFailed` means secure random secret generation failed.
- `EncryptionFailed` means third-party secret encryption failed.
- `CryptoOperationFailed` means a required cryptographic operation failed.
- `InvalidBase64` means a Base64 envelope could not be decoded.
- `InvalidHex` means a hex-encoded secret could not be decoded.
- `InvalidSecretLength` means configured secret material was not exactly
  `SIGNATURE_BYTES` bytes.
- `InvalidTime` means a `SystemTime` could not be represented as a macarunes
  timestamp.

Assumptions
-----------

- We rely upon the type system to provide memory safety.  Secrets are scrubbed
  after use with libsodium's memory clearing API, but that does not make logged,
  cloned, serialized, swapped, or otherwise copied secret material safe.

- This library relies upon the determinism of the protocol buffers code.  This
  is guaranteed by prototk.

- Intentionally restrictive language compared to the Python implementation.
  The only cases you need in the core library are to make exact comparisons
  (which are born out as format strings), to set verifier-time bounds, or to
  use a third-party caveat that enforces some arbitrary predicate before the
  discharge macaroon is granted.  Verifier time is the first-party intrinsic
  that cannot collapse onto stringified exact caveats because the verifier must
  compare the environment's current time against a signed bound.

About Locations
---------------

A location is a hint that is not part of the macaroon's signature.  Root
macaroon locations, discharge macaroon locations, and third-party caveat
locations are all intentionally unsigned.  The verifier makes authority
decisions from signed cryptographic material: identifiers, third-party
identifiers, encrypted verification-key material, and the signature chain.

For that reason, locations should be treated as untrusted routing hints.  They
can affect request assembly because a client uses them to choose a registered
loader, but changing a location does not by itself make a proof valid or
invalid.

For each third party, a `Loader` should be developed that negotiates the
protocol to get discharge macaroons.  Loader locations are static strings
registered by application code.  A location from untrusted macaroon data only
selects among those registered loaders; unknown locations fail with
`MissingLoader`.

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
