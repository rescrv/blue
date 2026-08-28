# ordsum

An order-sensitive, concatenation-composable checksum: the sequence sibling of
[setsum](https://crates.io/crates/setsum).

Where setsum digests a *multiset* (insertion order cannot matter), ordsum
digests a *sequence* (insertion order must matter).  Both compose: the checksum
of a whole can be computed from checksums of its parts without rehashing.

```rust
use ordsum::Ordsum;

let mut left = Ordsum::default();
left.push(b"fragment 1");
let mut right = Ordsum::default();
right.push(b"fragment 2");

let mut whole = Ordsum::default();
whole.push(b"fragment 1");
whole.push(b"fragment 2");

assert_eq!(whole, left.concat(&right));
assert_ne!(whole, right.concat(&left));       // order matters
assert_eq!(right, whole.remove_prefix(&left)); // prefixes subtract
assert_eq!(left, whole.remove_suffix(&right)); // suffixes subtract
```

## How it works

Each item is hashed with SHA3-256 and mapped to an affine transformation
`t -> a*t + b` over the field of integers modulo the Mersenne prime
`p = 2^127 - 1` (with `a` forced nonzero).  A sequence's checksum is the
composition of its items' maps, earliest item innermost, stored as the pair
`(A, B)` in a 32-byte digest.

Composition of affine maps is associative but not commutative, which is
exactly the algebra of concatenation.  Associativity means checksums of chunks
reduce in any tree shape (parallel reduction is safe); non-commutativity means
reordered streams produce different checksums.  Every affine map with `a != 0`
is invertible, which is what makes `remove_prefix` and `remove_suffix` work.

The `A` component is the product of every item's `a` and is itself
order-independent; the `B` component carries order.  Consequently equal
digests imply agreement on both the multiset of items and their order.

## Diagnosing mismatches

Because the `A` component is order-independent and `B` is not, a mismatch is
more than one bit of information.  `diagnose` classifies it:

```rust
use ordsum::{Diagnosis, Ordsum};

let mut local = Ordsum::default();
local.push(b"fragment 1");
local.push(b"fragment 2");
let mut remote = Ordsum::default();
remote.push(b"fragment 2");
remote.push(b"fragment 1");

match local.diagnose(&remote) {
    Diagnosis::Equal => {}       // same items, same order
    Diagnosis::Reordered => {}   // same items, wrong splice order
    Diagnosis::Divergent => {}   // the items themselves differ
}
```

`Divergent` is a deterministic claim (equal multisets cannot produce unequal
`A`); `Reordered` holds unless `A` collided (~2^-127).  The two arms route to
different repairs: `Divergent` pairs with set-reconciliation to decode which
items differ, while `Reordered` means the symmetric difference is empty and
order must be re-derived from other metadata.  The enum is exhaustive: the
three arms cover every way two checksums can relate.

## What it guarantees, and what it does not

ordsum targets the same threat model as setsum: detecting *corruption*, not
resisting *adversaries*.  Under the heuristic that SHA3-256 outputs behave as
uniform field elements:

- Replacing, inserting, or dropping an item yields a colliding digest with
  probability about 2^-254.
- Reordering items (same multiset, different order) leaves `A` unchanged by
  construction and yields a colliding `B` with probability about 2^-127.

Against an adversary who chooses inputs, ordsum offers no collision
resistance: forging a stream with a target digest reduces to solving one
linear equation over the field.  If digests cross a trust boundary, they must
be protected the same way you would protect a setsum -- signed, MAC'd, or
stored somewhere the adversary cannot write.  (A keyed variant with per-key
item maps would give universal-hash forgery bounds; it is future work.)

## Costs

One SHA3-256 invocation plus two multiplications mod `2^127 - 1` per item;
push throughput is SHA3-bound.  `concat` is two multiplications and an
addition.  `inverse` is one 127-bit modular exponentiation (~microseconds).
State and digest are 32 bytes.

## Status

Property-tested (group laws, homomorphism over random split points, and
differential tests of the field arithmetic against num-bigint), but young.
The digest format may change before 1.0.
