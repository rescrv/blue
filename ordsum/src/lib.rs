#![doc = include_str!("../README.md")]

use std::fmt::{Debug, Write};

use sha3::{Digest, Sha3_256};

/// The number of bytes in a ordsum digest: 16 bytes for each of the two field
/// elements (A, B) that represent the affine map t -> A*t + B.
pub const ORDSUM_BYTES: usize = 32;

/// The field modulus: the Mersenne prime 2^127 - 1.
pub const ORDSUM_PRIME: u128 = (1u128 << 127) - 1;

const MASK127: u128 = (1u128 << 127) - 1;
const MASK64: u128 = 0xffff_ffff_ffff_ffff;

////////////////////////////////////// field arithmetic /////////////////////////////////////////

/// Reduce a full 128-bit value into canonical range [0, p).
#[inline(always)]
fn reduce128(x: u128) -> u128 {
    // 2^127 == 1 (mod p), so fold the top bit down.
    let mut r = (x & MASK127) + (x >> 127);
    if r >= ORDSUM_PRIME {
        r -= ORDSUM_PRIME;
    }
    r
}

/// Full 256-bit product of two u128s, as (hi, lo).
#[inline(always)]
fn mul_wide(x: u128, y: u128) -> (u128, u128) {
    let x0 = x & MASK64;
    let x1 = x >> 64;
    let y0 = y & MASK64;
    let y1 = y >> 64;
    let ll = x0 * y0;
    let lh = x0 * y1;
    let hl = x1 * y0;
    let hh = x1 * y1;
    let (mid, mid_c) = lh.overflowing_add(hl);
    let (lo, lo_c) = ll.overflowing_add(mid << 64);
    let hi = hh + (mid >> 64) + ((mid_c as u128) << 64) + (lo_c as u128);
    (hi, lo)
}

/// Modular addition.  Inputs must be canonical.
#[inline(always)]
fn addmod(x: u128, y: u128) -> u128 {
    let mut s = x + y;
    if s >= ORDSUM_PRIME {
        s -= ORDSUM_PRIME;
    }
    s
}

/// Modular subtraction.  Inputs must be canonical.
#[inline(always)]
fn submod(x: u128, y: u128) -> u128 {
    let mut s = x + ORDSUM_PRIME - y;
    if s >= ORDSUM_PRIME {
        s -= ORDSUM_PRIME;
    }
    s
}

/// Modular multiplication.  Inputs must be canonical.
#[inline(always)]
fn mulmod(x: u128, y: u128) -> u128 {
    let (hi, lo) = mul_wide(x, y);
    // x, y < 2^127, so hi < 2^126 and hi << 1 cannot overflow.
    // value = hi * 2^128 + lo, and 2^128 == 2 (mod p).
    let sum = (lo & MASK127) + (lo >> 127) + (hi << 1);
    reduce128(sum)
}

/// Modular exponentiation by square-and-multiply.
#[inline]
fn powmod(mut base: u128, mut exp: u128) -> u128 {
    let mut acc = 1u128;
    while exp > 0 {
        if exp & 1 == 1 {
            acc = mulmod(acc, base);
        }
        base = mulmod(base, base);
        exp >>= 1;
    }
    acc
}

/// Modular inverse via Fermat's little theorem.  Input must be nonzero.
#[inline]
fn invmod(x: u128) -> u128 {
    debug_assert!(x != 0);
    powmod(x, ORDSUM_PRIME - 2)
}

////////////////////////////////////////// Ordsum ///////////////////////////////////////////////

/// An order-sensitive, concatenation-composable checksum.
///
/// Each item is hashed to an affine map t -> a*t + b over F_p (p = 2^127 - 1),
/// and a stream's checksum is the composition of its items' maps, earliest item
/// innermost.  Composition of affine maps is associative but not commutative,
/// so the checksum distinguishes order.  Every map is invertible, so prefixes
/// and suffixes can be subtracted.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ordsum {
    /// The multiplicative component: the product of every item's `a`.
    /// This component alone is order-independent.
    a: u128,
    /// The affine component; this is where order sensitivity lives.
    b: u128,
}

impl Ordsum {
    /// Hash one item to its affine map (a, b), with a != 0.
    #[inline]
    fn item_to_map(item: &[&[u8]]) -> (u128, u128) {
        let mut hasher = Sha3_256::default();
        for piece in item {
            hasher.update(piece);
        }
        let d: [u8; 32] = hasher.finalize().into();
        let a_raw = u128::from_le_bytes(d[..16].try_into().unwrap());
        let b_raw = u128::from_le_bytes(d[16..].try_into().unwrap());
        let mut a = reduce128(a_raw);
        if a == 0 {
            a = 1;
        }
        (a, reduce128(b_raw))
    }

    /// Append one item to the end of the stream this checksum represents.
    pub fn push(&mut self, item: &[u8]) {
        self.push_vectored(&[item]);
    }

    /// Append one item, provided as concatenated pieces, to the end of the
    /// stream this checksum represents.  Hashes as if the pieces were one
    /// contiguous buffer.
    pub fn push_vectored(&mut self, item: &[&[u8]]) {
        let (a, b) = Self::item_to_map(item);
        // new = item_map . self:  t -> a*(A*t + B) + b
        self.b = addmod(mulmod(a, self.b), b);
        self.a = mulmod(a, self.a);
    }

    /// The checksum of `self`'s stream followed by `other`'s stream.
    ///
    /// This is a homomorphism: pushing items [x, y, z] one-by-one equals
    /// concat of a checksum of [x] and a checksum of [y, z].
    #[must_use]
    pub fn concat(&self, other: &Ordsum) -> Ordsum {
        // other . self:  t -> A_o*(A_s*t + B_s) + B_o
        Ordsum {
            a: mulmod(other.a, self.a),
            b: addmod(mulmod(other.a, self.b), other.b),
        }
    }

    /// The inverse element in the affine group.  `x.concat(&x.inverse())` is
    /// the identity (the checksum of the empty stream).
    #[must_use]
    pub fn inverse(&self) -> Ordsum {
        let a_inv = invmod(self.a);
        Ordsum {
            a: a_inv,
            b: submod(0, mulmod(a_inv, self.b)),
        }
    }

    /// Given that `self` is the checksum of `prefix`'s stream followed by some
    /// rest, return the checksum of the rest.
    ///
    /// `prefix.concat(&self.remove_prefix(&prefix))` equals `self`.
    #[must_use]
    pub fn remove_prefix(&self, prefix: &Ordsum) -> Ordsum {
        prefix.inverse().concat(self)
    }

    /// Given that `self` is the checksum of some rest followed by `suffix`'s
    /// stream, return the checksum of the rest.
    ///
    /// `self.remove_suffix(&suffix).concat(&suffix)` equals `self`.
    #[must_use]
    pub fn remove_suffix(&self, suffix: &Ordsum) -> Ordsum {
        self.concat(&suffix.inverse())
    }

    /// The 32-byte digest: A as 16 little-endian bytes, then B.
    pub fn digest(&self) -> [u8; ORDSUM_BYTES] {
        let mut d = [0u8; ORDSUM_BYTES];
        d[..16].copy_from_slice(&self.a.to_le_bytes());
        d[16..].copy_from_slice(&self.b.to_le_bytes());
        d
    }

    /// Reconstruct a checksum from its digest.  Returns None unless both
    /// components are canonical field elements and A is nonzero (A = 0 is not
    /// an element of the affine group and cannot arise from any stream).
    pub fn from_digest(digest: [u8; ORDSUM_BYTES]) -> Option<Ordsum> {
        let a = u128::from_le_bytes(digest[..16].try_into().unwrap());
        let b = u128::from_le_bytes(digest[16..].try_into().unwrap());
        if a == 0 || a >= ORDSUM_PRIME || b >= ORDSUM_PRIME {
            return None;
        }
        Some(Ordsum { a, b })
    }

    /// The digest as a lowercase hex string.
    pub fn hexdigest(&self) -> String {
        let d = self.digest();
        let mut s = String::with_capacity(ORDSUM_BYTES * 2);
        for byte in d.iter() {
            write!(&mut s, "{:02x}", byte).expect("write to string should succeed");
        }
        s
    }

    /// Reconstruct a checksum from its hex digest.
    pub fn from_hexdigest(digest: &str) -> Option<Ordsum> {
        if digest.len() != ORDSUM_BYTES * 2 {
            return None;
        }
        let mut d = [0u8; ORDSUM_BYTES];
        for (i, byte) in d.iter_mut().enumerate() {
            *byte = u8::from_str_radix(digest.get(i * 2..i * 2 + 2)?, 16).ok()?;
        }
        Self::from_digest(d)
    }
}

/// The result of comparing two ordsums.
///
/// The arms make asymmetric claims.  `Divergent` is deterministic: equal
/// multisets always produce equal A components, so unequal A proves the
/// multisets differ.  `Reordered` is probabilistic: it means the multisets
/// agree unless the A components collided (~2^-127 for hash-derived streams).
/// `Divergent` does not rule out reordering on top of the multiset
/// difference; A is blind to order.
///
/// This enum classifies accidents, not attacks.  An adversary who can choose
/// items can force any arm; see the crate-level threat model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Diagnosis {
    /// Digests are identical: same items, same order (up to full collision).
    Equal,
    /// Same multiset of items, different order.
    Reordered,
    /// The multisets differ: at least one item changed, dropped, or
    /// duplicated.  Order may also differ; this checksum cannot tell.
    Divergent,
}

impl Ordsum {
    /// Compare two checksums and classify how their streams differ.
    ///
    /// Symmetric: `x.diagnose(&y) == y.diagnose(&x)`.
    pub fn diagnose(&self, other: &Ordsum) -> Diagnosis {
        // Match on the tuple so that a future arm forces this expression to
        // be revisited.  The wildcard on B when A differs is deliberate:
        // B-equality carries no information once the multisets diverge.
        match (self.a == other.a, self.b == other.b) {
            (true, true) => Diagnosis::Equal,
            (true, false) => Diagnosis::Reordered,
            (false, _) => Diagnosis::Divergent,
        }
    }
}

impl Default for Ordsum {
    /// The checksum of the empty stream: the identity map t -> t.
    fn default() -> Ordsum {
        Ordsum { a: 1, b: 0 }
    }
}

impl Debug for Ordsum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "ordsum:{}", self.hexdigest())
    }
}

/// `lhs * rhs` is the checksum of lhs's stream followed by rhs's stream.
/// Multiplication is associative and NOT commutative, matching concatenation.
impl std::ops::Mul<Ordsum> for Ordsum {
    type Output = Ordsum;

    fn mul(self, rhs: Ordsum) -> Ordsum {
        self.concat(&rhs)
    }
}

impl std::ops::MulAssign<Ordsum> for Ordsum {
    fn mul_assign(&mut self, rhs: Ordsum) {
        *self = self.concat(&rhs);
    }
}

/// `lhs / rhs` removes rhs's stream from the end of lhs's stream.
impl std::ops::Div<Ordsum> for Ordsum {
    type Output = Ordsum;

    fn div(self, rhs: Ordsum) -> Ordsum {
        self.remove_suffix(&rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_identity() {
        let e = Ordsum::default();
        assert_eq!(e, e.concat(&e));
        assert_eq!(e.hexdigest(), format!("01{}", "00".repeat(31)));
    }

    #[test]
    fn order_matters() {
        let mut xy = Ordsum::default();
        xy.push(b"x");
        xy.push(b"y");
        let mut yx = Ordsum::default();
        yx.push(b"y");
        yx.push(b"x");
        assert_ne!(xy, yx);
    }

    #[test]
    fn push_vectored_matches_push() {
        let mut whole = Ordsum::default();
        whole.push(b"hello world");
        let mut pieces = Ordsum::default();
        pieces.push_vectored(&[b"hello", b" ", b"world"]);
        assert_eq!(whole, pieces);
    }

    #[test]
    fn concat_matches_sequential_push() {
        let mut whole = Ordsum::default();
        for item in [b"a".as_ref(), b"b".as_ref(), b"c".as_ref()] {
            whole.push(item);
        }
        let mut left = Ordsum::default();
        left.push(b"a");
        let mut right = Ordsum::default();
        right.push(b"b");
        right.push(b"c");
        assert_eq!(whole, left.concat(&right));
        assert_eq!(whole, left * right);
    }

    #[test]
    fn prefix_suffix_roundtrip() {
        let mut left = Ordsum::default();
        left.push(b"wal fragment 1");
        let mut right = Ordsum::default();
        right.push(b"wal fragment 2");
        let whole = left.concat(&right);
        assert_eq!(right, whole.remove_prefix(&left));
        assert_eq!(left, whole.remove_suffix(&right));
        assert_eq!(left, whole / right);
    }

    #[test]
    fn digest_roundtrip() {
        let mut c = Ordsum::default();
        c.push(b"round");
        c.push(b"trip");
        assert_eq!(Some(c), Ordsum::from_digest(c.digest()));
        assert_eq!(Some(c), Ordsum::from_hexdigest(&c.hexdigest()));
    }

    #[test]
    fn from_digest_rejects_noncanonical() {
        // A = 0 is not in the group.
        let mut d = [0u8; ORDSUM_BYTES];
        assert_eq!(None, Ordsum::from_digest(d));
        // A = p is not canonical.
        d[..16].copy_from_slice(&ORDSUM_PRIME.to_le_bytes());
        assert_eq!(None, Ordsum::from_digest(d));
        // A = p + 1 is not canonical either, and aliases A = 2 additively.
        d[..16].copy_from_slice(&(ORDSUM_PRIME + 1).to_le_bytes());
        assert_eq!(None, Ordsum::from_digest(d));
    }
}
