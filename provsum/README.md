# provsum

Fixed-size digests of error provenance, for checking that a Rust port raises
the *same errors for the same reasons* as the code it replaces.

## Construction

Every error-origin, wrap, and pick site in the source gets a 16-byte GUID,
carried verbatim across both implementations. Each GUID is hashed to a nonzero
point in F_p (p = 2^61−1) per lane (two lanes). The digest of an error is its
N[X] provenance polynomial evaluated at those points:

`Site` is the GUID's 16 bytes in canonical order, not a native-endian integer.
Point derivation reads its two consecutive 8-byte chunks as little-endian, and
digest serialization is little-endian too, so neither operation depends on the
host architecture. A byte-array literal therefore produces the same result on
every supported platform. When starting from textual UUID syntax, use the
parser's canonical/RFC byte sequence in every language; in particular, do not
mix a Windows GUID's field-wise little-endian representation with RFC bytes.

| operation                      | polynomial                 | digest      |
|--------------------------------|----------------------------|-------------|
| `Digest::origin(site)`         | `x_site`                   | `r_site`    |
| `d.wrap(site)`                 | `x_site · d`               | `r_site · d`|
| `a.merge(b)`                   | `a + b`                    | `a + b`     |
| `Digest::pick(site, [d0,d1..])`| `Σ x_{site,i} · d_i`       | `Σ r_{site,i} · d_i` |

`pick` records an ordered choice: slot 0 is the error that was returned, later
slots were considered and discarded. Order enters the commutative semiring
through per-slot variables, not through the operators, so swapping which error
wins changes the polynomial. Plain `wrap` order is *not* recorded (`x·y = y·x`);
that is intentional — a port that restructures control flow but preserves
which sites contribute and how deeply should digest identically.

Digests are 18 bytes: two 8-byte lanes plus a 2-byte depth bound. Equality
compares lanes only. Two digests of distinct polynomials of depth ≤ d collide
with probability ≤ (d/(p−1))² ≈ 2^(2·log2 d − 122). This is a non-adversarial
construction: it assumes nobody is choosing GUIDs or points to force a
collision. `Digest::log2_collision_bound()` reports the bound for a given depth.

## Diagnostics

`provsum::poly::Poly` is the expanded polynomial with the same API.
`Poly::digest()` evaluates it at the same points, so `poly.digest() ==
streaming_digest` is a testable invariant, and `Poly::diff` names the monomials
that differ between two implementations. Keep it behind a debug flag; its size
is the number of distinct derivations, which is fine at a few thousand sites
and shallow depth and unbounded in general.

## Cross-language

`python/provsum.py` is a reference implementation. `cargo run --example
vectors` and `python3 python/provsum.py` must print identical output;
`tests/vectors.rs` pins those values. Point derivation is a splitmix64 finalizer
over `(seed, lane, slot)` then the two GUID halves, rejection-sampled to be
nonzero mod p — deliberately dependency-free so it is trivial to mirror.

## Usage sketch

```rust
use provsum::{Digest, Site};

const PARSE: Site = Site(*b"\x01\x02..............");   // real GUIDs in practice
const READ:  Site = Site(*b"\x03\x04..............");
const RETRY: Site = Site(*b"\x05\x06..............");

struct Error { msg: String, prov: Digest }

fn read(path: &str) -> Result<Vec<u8>, Error> {
    std::fs::read(path).map_err(|e| Error { msg: e.to_string(), prov: Digest::origin(&READ) })
}

fn parse(path: &str) -> Result<Config, Error> {
    let bytes = read(path).map_err(|e| Error { prov: e.prov.wrap(&PARSE), ..e })?;
    // ...
}

fn with_fallback(primary: &str, fallback: &str) -> Result<Config, Error> {
    match (parse(primary), parse(fallback)) {
        (Ok(c), _) | (_, Ok(c)) => Ok(c),
        (Err(a), Err(b)) => Err(Error { prov: Digest::pick2(&RETRY, &a.prov, &b.prov), ..a }),
    }
}
```

The Python side does the same with `Digest.origin(READ).wrap(PARSE)` etc.;
the differential harness runs both on the same inputs and compares
`prov.to_bytes()`.
