//! Expanded `N[X]` provenance for diagnostics.
//!
//! `Poly` is the literal polynomial the digest evaluates.  It costs space
//! proportional to the number of monomials, which is exponential in merge
//! depth in the worst case, but for a few thousand sites and shallow wrap
//! depth it is cheap enough to keep behind a debug flag.  `Poly::digest()`
//! evaluates it at the same points the streaming digest uses, so
//! `poly.digest() == streaming_digest` is a checkable invariant, and
//! `Poly::diff` localizes divergence between two implementations.

use crate::{fadd, fmul, point, Digest, Site, LANES, NO_SLOT};
use std::collections::BTreeMap;

/// A variable: a site, optionally with a positional slot.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Var {
    pub site: Site,
    pub slot: u32,
}

impl Var {
    pub fn plain(site: Site) -> Var {
        Var {
            site,
            slot: NO_SLOT,
        }
    }
}

/// A monomial: variable → exponent.
pub type Monomial = BTreeMap<Var, u32>;

/// A polynomial over `N`: monomial → coefficient.  Zero coefficients are never
/// stored.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Poly(pub BTreeMap<Monomial, u64>);

impl Poly {
    pub fn zero() -> Poly {
        Poly::default()
    }

    pub fn origin(site: &Site) -> Poly {
        let mut m = Monomial::new();
        m.insert(Var::plain(*site), 1);
        let mut p = BTreeMap::new();
        p.insert(m, 1);
        Poly(p)
    }

    fn mul_var(&self, v: Var) -> Poly {
        let mut out = BTreeMap::new();
        for (m, c) in &self.0 {
            let mut m2 = m.clone();
            *m2.entry(v).or_insert(0) += 1;
            *out.entry(m2).or_insert(0) += c;
        }
        Poly(out)
    }

    pub fn wrap(&self, site: &Site) -> Poly {
        self.mul_var(Var::plain(*site))
    }

    pub fn merge(&self, other: &Poly) -> Poly {
        let mut out = self.0.clone();
        for (m, c) in &other.0 {
            *out.entry(m.clone()).or_insert(0) += c;
        }
        Poly(out)
    }

    pub fn pick(site: &Site, ranked: &[Poly]) -> Poly {
        let mut acc = Poly::zero();
        for (slot, p) in ranked.iter().enumerate() {
            acc = acc.merge(&p.mul_var(Var {
                site: *site,
                slot: slot as u32,
            }));
        }
        acc
    }

    pub fn pick2(site: &Site, returned: &Poly, discarded: &Poly) -> Poly {
        Poly::pick(site, &[returned.clone(), discarded.clone()])
    }

    /// Total degree.
    pub fn degree(&self) -> u32 {
        self.0
            .keys()
            .map(|m| m.values().sum::<u32>())
            .max()
            .unwrap_or(0)
    }

    /// Evaluate at the crate's pinned points.  Must equal the streaming digest.
    pub fn digest(&self) -> Digest {
        let mut lanes = [0u64; LANES];
        for (m, c) in &self.0 {
            let c = c % crate::P;
            for lane in 0..LANES {
                let mut term = c;
                for (v, e) in m {
                    let r = point(&v.site, v.slot, lane as u32);
                    for _ in 0..*e {
                        term = fmul(term, r);
                    }
                }
                lanes[lane] = fadd(lanes[lane], term);
            }
        }
        // Reconstruct through the public API so `depth` is populated
        // consistently: degree is an exact bound here.
        let mut b = [0u8; crate::DIGEST_BYTES];
        for (i, l) in lanes.iter().enumerate() {
            b[i * 8..i * 8 + 8].copy_from_slice(&l.to_le_bytes());
        }
        let d = self.degree().min(u16::MAX as u32) as u16;
        b[LANES * 8..].copy_from_slice(&d.to_le_bytes());
        Digest::from_bytes(&b).expect("lanes are reduced")
    }

    /// Monomials whose coefficients differ: `(monomial, coeff_in_self, coeff_in_other)`.
    pub fn diff(&self, other: &Poly) -> Vec<(Monomial, u64, u64)> {
        let mut keys: Vec<&Monomial> = self.0.keys().chain(other.0.keys()).collect();
        keys.sort();
        keys.dedup();
        keys.into_iter()
            .filter_map(|m| {
                let a = *self.0.get(m).unwrap_or(&0);
                let b = *other.0.get(m).unwrap_or(&0);
                (a != b).then(|| (m.clone(), a, b))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(l: &str) -> Site {
        Site::from_label(l)
    }

    /// Build the same expression both ways and check the digests agree.
    #[test]
    fn expanded_matches_streaming() {
        let (a, b, c) = (s("a"), s("b"), s("c"));
        let (w1, w2, ch) = (s("w1"), s("w2"), s("choose"));

        let d = Digest::pick2(
            &ch,
            &Digest::origin(&a).wrap(&w1).merge(&Digest::origin(&b).wrap(&w1)),
            &Digest::origin(&c).wrap(&w2),
        )
        .wrap(&w2);

        let p = Poly::pick2(
            &ch,
            &Poly::origin(&a).wrap(&w1).merge(&Poly::origin(&b).wrap(&w1)),
            &Poly::origin(&c).wrap(&w2),
        )
        .wrap(&w2);

        assert_eq!(p.digest(), d);
        assert_eq!(p.degree() as u16, d.depth());
    }

    #[test]
    fn diff_localizes_missing_wrap() {
        let (a, io, h) = (s("a"), s("io"), s("handler"));
        let full = Poly::origin(&a).wrap(&io).wrap(&h);
        let short = Poly::origin(&a).wrap(&h);
        let diff = full.diff(&short);
        assert_eq!(diff.len(), 2);
        assert_ne!(full.digest(), short.digest());
    }

    /// Randomized: distinct polynomials get distinct digests; equal ones agree.
    #[test]
    fn random_expressions() {
        let mut seed = 0xdead_beefu64;
        let mut rng = move || {
            seed = crate::mix(seed);
            seed
        };
        let sites: Vec<Site> = (0..32).map(|i| s(&format!("s{i}"))).collect();
        let mut seen: Vec<(Poly, Digest)> = Vec::new();
        for _ in 0..300 {
            let (p, d) = gen(&mut rng, &sites, 4);
            assert_eq!(p.digest(), d, "expanded != streaming");
            for (q, e) in &seen {
                assert_eq!(&p == q, &d == e, "digest equality disagrees with polynomial equality");
            }
            seen.push((p, d));
        }
    }

    fn gen(rng: &mut impl FnMut() -> u64, sites: &[Site], depth: u32) -> (Poly, Digest) {
        let site = sites[(rng() % sites.len() as u64) as usize];
        if depth == 0 || rng() % 4 == 0 {
            return (Poly::origin(&site), Digest::origin(&site));
        }
        match rng() % 3 {
            0 => {
                let (p, d) = gen(rng, sites, depth - 1);
                (p.wrap(&site), d.wrap(&site))
            }
            1 => {
                let (p1, d1) = gen(rng, sites, depth - 1);
                let (p2, d2) = gen(rng, sites, depth - 1);
                (p1.merge(&p2), d1.merge(&d2))
            }
            _ => {
                let (p1, d1) = gen(rng, sites, depth - 1);
                let (p2, d2) = gen(rng, sites, depth - 1);
                (Poly::pick2(&site, &p1, &p2), Digest::pick2(&site, &d1, &d2))
            }
        }
    }
}
