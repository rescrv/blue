//! # provsum
//!
//! Fixed-size digests of provenance polynomials.
//!
//! Every error-origin site in a codebase gets a 16-byte GUID.  Each GUID is
//! hashed to a nonzero point in `F_p` (one per lane, `p = 2^61 - 1`).  The
//! digest of an error is its `N[X]` provenance polynomial evaluated at those
//! points:
//!
//! * `origin(site)`             → `r_site`
//! * `wrap(site, d)`            → `r_site · d`
//! * `merge(a, b)`              → `a + b`
//! * `pick(site, [d_0, d_1,…])` → `Σ r_{site,slot_i} · d_i`
//!
//! `pick` is how order enters a commutative semiring: the site contributes a
//! distinct variable per *slot*, so returning `a` and discarding `b` yields a
//! different polynomial from the reverse.  Slot 0 is the returned error.
//!
//! Two digests are equal iff the two polynomials are equal, except with
//! probability at most `(d/(p-1))^LANES` where `d` is the max wrap depth
//! (Schwartz–Zippel, lanes independent).  The construction is not designed
//! against an adversary who controls GUIDs or points; it is meant for
//! comparing two implementations of the same code.
//!
//! The point derivation is pinned (splitmix64 finalizer over the GUID bytes)
//! so that a second implementation (see `python/provsum.py`) reproduces the
//! same digests byte-for-byte.

#![forbid(unsafe_code)]

pub mod poly;

/// Number of independent lanes.  Each lane is an independent evaluation point.
pub const LANES: usize = 2;

/// The Mersenne prime 2^61 - 1.
pub const P: u64 = (1u64 << 61) - 1;

/// Serialized digest size in bytes: `LANES * 8` for the lanes, plus 2 for depth.
pub const DIGEST_BYTES: usize = LANES * 8 + 2;

/// A 16-byte site identifier.  Assign one per error-origin / wrap / pick site
/// in the source, and carry the same value across implementations.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Site(pub [u8; 16]);

impl Site {
    /// Convenience for tests and examples: a site from a short label.  Do not
    /// use in production—assign real GUIDs so sites survive renames.
    pub fn from_label(label: &str) -> Site {
        let mut b = [0u8; 16];
        let mut s = mix(0x6c61_6265_6c5f_7369 /* "label_si" */);
        for (i, chunk) in label.as_bytes().chunks(8).enumerate() {
            let mut w = [0u8; 8];
            w[..chunk.len()].copy_from_slice(chunk);
            s = mix(s ^ u64::from_le_bytes(w) ^ (i as u64));
        }
        b[..8].copy_from_slice(&s.to_le_bytes());
        b[8..].copy_from_slice(&mix(s).to_le_bytes());
        Site(b)
    }
}

// ---------------------------------------------------------------------------
// Field arithmetic over F_p, p = 2^61 - 1.
// ---------------------------------------------------------------------------

#[inline]
pub(crate) fn reduce128(x: u128) -> u64 {
    // x < 2^122.  Split at 61 bits twice.
    let lo = (x as u64) & P;
    let hi = (x >> 61) as u64; // < 2^61
    let s = lo + hi; // < 2^62
    let s = (s & P) + (s >> 61);
    if s >= P {
        s - P
    } else {
        s
    }
}

#[inline]
pub(crate) fn fadd(a: u64, b: u64) -> u64 {
    let s = a + b; // < 2^62
    let s = (s & P) + (s >> 61);
    if s >= P {
        s - P
    } else {
        s
    }
}

#[inline]
pub(crate) fn fmul(a: u64, b: u64) -> u64 {
    reduce128((a as u128) * (b as u128))
}

// ---------------------------------------------------------------------------
// Point derivation.  Pinned; mirrored in python/provsum.py.
// ---------------------------------------------------------------------------

/// splitmix64 finalizer.
#[inline]
pub(crate) fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

const SEED: u64 = 0x7072_6f76_7375_6d31; // "provsum1"

/// Slot value used by `origin`/`wrap` (no positional slot).
pub const NO_SLOT: u32 = u32::MAX;

/// Derive the evaluation point for `(site, slot, lane)`.  `pick` uses slots
/// `0..n`; `origin`/`wrap` use `NO_SLOT`.
pub fn point(site: &Site, slot: u32, lane: u32) -> u64 {
    let mut s = mix(SEED ^ ((lane as u64) << 32) ^ (slot as u64));
    for chunk in site.0.chunks(8) {
        let mut w = [0u8; 8];
        w.copy_from_slice(chunk);
        s = mix(s ^ u64::from_le_bytes(w));
    }
    loop {
        let r = s % P;
        if r != 0 {
            return r;
        }
        s = mix(s);
    }
}

// ---------------------------------------------------------------------------
// Digest.
// ---------------------------------------------------------------------------

/// A provenance digest: the polynomial's value in each lane, plus an upper
/// bound on its total degree (used only for the collision bound; equality
/// compares lanes only).
#[derive(Clone, Copy, Debug)]
pub struct Digest {
    lanes: [u64; LANES],
    depth: u16,
}

impl PartialEq for Digest {
    fn eq(&self, other: &Self) -> bool {
        self.lanes == other.lanes
    }
}
impl Eq for Digest {}

impl core::hash::Hash for Digest {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.lanes.hash(state)
    }
}

impl Digest {
    /// The zero polynomial: identity for `merge`, annihilator for `wrap`.
    pub const ZERO: Digest = Digest {
        lanes: [0; LANES],
        depth: 0,
    };

    /// The constant polynomial 1: identity for `wrap`.  Rarely needed.
    pub const ONE: Digest = Digest {
        lanes: [1; LANES],
        depth: 0,
    };

    fn points(site: &Site, slot: u32) -> [u64; LANES] {
        let mut r = [0u64; LANES];
        for (lane, out) in r.iter_mut().enumerate() {
            *out = point(site, slot, lane as u32);
        }
        r
    }

    /// An error originating at `site`.  Polynomial: `x_site`.
    pub fn origin(site: &Site) -> Digest {
        Digest {
            lanes: Self::points(site, NO_SLOT),
            depth: 1,
        }
    }

    /// Wrap `self` at `site`.  Polynomial: `x_site · self`.
    #[must_use]
    pub fn wrap(&self, site: &Site) -> Digest {
        let r = Self::points(site, NO_SLOT);
        let mut lanes = [0u64; LANES];
        for i in 0..LANES {
            lanes[i] = fmul(r[i], self.lanes[i]);
        }
        Digest {
            lanes,
            depth: self.depth.saturating_add(1),
        }
    }

    /// Alternative derivations.  Polynomial: `self + other`.  Commutative.
    #[must_use]
    pub fn merge(&self, other: &Digest) -> Digest {
        let mut lanes = [0u64; LANES];
        for i in 0..LANES {
            lanes[i] = fadd(self.lanes[i], other.lanes[i]);
        }
        Digest {
            lanes,
            depth: self.depth.max(other.depth),
        }
    }

    /// Ordered choice at `site`.  `ranked[0]` is the error that was returned;
    /// the rest were considered and discarded, in priority order.
    /// Polynomial: `Σ_i x_{site,i} · ranked[i]`.  Not commutative in `ranked`.
    pub fn pick(site: &Site, ranked: &[Digest]) -> Digest {
        let mut acc = Digest::ZERO;
        for (slot, d) in ranked.iter().enumerate() {
            let r = Self::points(site, slot as u32);
            let mut lanes = [0u64; LANES];
            for i in 0..LANES {
                lanes[i] = fmul(r[i], d.lanes[i]);
            }
            let term = Digest {
                lanes,
                depth: d.depth.saturating_add(1),
            };
            acc = acc.merge(&term);
        }
        acc
    }

    /// Binary convenience for `pick`.
    pub fn pick2(site: &Site, returned: &Digest, discarded: &Digest) -> Digest {
        Self::pick(site, &[*returned, *discarded])
    }

    /// Upper bound on the total degree of the polynomial this digest evaluates.
    pub fn depth(&self) -> u16 {
        self.depth
    }

    /// Raw lane values.
    pub fn lanes(&self) -> [u64; LANES] {
        self.lanes
    }

    /// `log2` of the Schwartz–Zippel false-equality bound for comparing this
    /// digest against another of at most the same depth: `LANES · log2(d/(p-1))`.
    pub fn log2_collision_bound(&self) -> f64 {
        let d = self.depth.max(1) as f64;
        (LANES as f64) * (d.log2() - ((P - 1) as f64).log2())
    }

    pub fn to_bytes(&self) -> [u8; DIGEST_BYTES] {
        let mut b = [0u8; DIGEST_BYTES];
        for (i, l) in self.lanes.iter().enumerate() {
            b[i * 8..i * 8 + 8].copy_from_slice(&l.to_le_bytes());
        }
        b[LANES * 8..].copy_from_slice(&self.depth.to_le_bytes());
        b
    }

    pub fn from_bytes(b: &[u8; DIGEST_BYTES]) -> Option<Digest> {
        let mut lanes = [0u64; LANES];
        for (i, l) in lanes.iter_mut().enumerate() {
            let mut w = [0u8; 8];
            w.copy_from_slice(&b[i * 8..i * 8 + 8]);
            *l = u64::from_le_bytes(w);
            if *l >= P {
                return None;
            }
        }
        let depth = u16::from_le_bytes([b[LANES * 8], b[LANES * 8 + 1]]);
        Some(Digest { lanes, depth })
    }

    /// Hex of the lanes only (what equality compares).
    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(LANES * 16);
        for l in self.lanes {
            s.push_str(&format!("{:016x}", l));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(l: &str) -> Site {
        Site::from_label(l)
    }

    #[test]
    fn field_laws() {
        let a = point(&s("a"), NO_SLOT, 0);
        let b = point(&s("b"), NO_SLOT, 0);
        let c = point(&s("c"), NO_SLOT, 0);
        assert_eq!(fmul(a, fmul(b, c)), fmul(fmul(a, b), c));
        assert_eq!(fmul(a, fadd(b, c)), fadd(fmul(a, b), fmul(a, c)));
        assert_eq!(fadd(a, P - a), 0);
        assert_eq!(fmul(P - 1, P - 1), 1);
    }

    #[test]
    fn points_nonzero_and_distinct_per_lane_and_slot() {
        let site = s("x");
        let p0 = point(&site, NO_SLOT, 0);
        let p1 = point(&site, NO_SLOT, 1);
        let q0 = point(&site, 0, 0);
        let q1 = point(&site, 1, 0);
        assert!(p0 != 0 && p1 != 0 && q0 != 0 && q1 != 0);
        assert_ne!(p0, p1);
        assert_ne!(q0, q1);
        assert_ne!(p0, q0);
    }

    #[test]
    fn semiring_laws_on_digests() {
        let a = Digest::origin(&s("a"));
        let b = Digest::origin(&s("b"));
        let w = s("w");
        // distributivity: w·(a+b) == w·a + w·b
        assert_eq!(a.merge(&b).wrap(&w), a.wrap(&w).merge(&b.wrap(&w)));
        // commutativity of merge
        assert_eq!(a.merge(&b), b.merge(&a));
        // wrap order is invisible (commutative multiplication) — by design
        assert_eq!(a.wrap(&s("p")).wrap(&s("q")), a.wrap(&s("q")).wrap(&s("p")));
        // identities
        assert_eq!(a.merge(&Digest::ZERO), a);
        assert_eq!(Digest::ZERO.wrap(&w), Digest::ZERO);
    }

    #[test]
    fn pick_is_order_sensitive() {
        let a = Digest::origin(&s("a"));
        let b = Digest::origin(&s("b"));
        let site = s("choose");
        let ab = Digest::pick2(&site, &a, &b);
        let ba = Digest::pick2(&site, &b, &a);
        assert_ne!(ab, ba);
        // and still distinguishes from a plain merge at that site
        assert_ne!(ab, a.merge(&b).wrap(&site));
        // n-ary: permutation matters
        let c = Digest::origin(&s("c"));
        assert_ne!(
            Digest::pick(&site, &[a, b, c]),
            Digest::pick(&site, &[a, c, b])
        );
    }

    #[test]
    fn missing_wrap_is_detected() {
        let a = Digest::origin(&s("a"));
        let full = a.wrap(&s("io")).wrap(&s("handler"));
        let short = a.wrap(&s("handler"));
        assert_ne!(full, short);
        assert_ne!(full, full.wrap(&s("io"))); // double wrap
    }

    #[test]
    fn bytes_roundtrip() {
        let d = Digest::origin(&s("a")).wrap(&s("b"));
        let b = d.to_bytes();
        let back = Digest::from_bytes(&b).unwrap();
        assert_eq!(d, back);
        assert_eq!(d.depth(), back.depth());
    }

    #[test]
    fn depth_bound() {
        let mut d = Digest::origin(&s("a"));
        for i in 0..9 {
            d = d.wrap(&s(&format!("w{i}")));
        }
        assert_eq!(d.depth(), 10);
        // two lanes at depth 10: about 2 * (log2 10 - 61) ≈ -115
        assert!(d.log2_collision_bound() < -114.0);
    }
}
