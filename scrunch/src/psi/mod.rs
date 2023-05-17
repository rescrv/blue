use std::hash::Hash;

use crate::Sigma;

pub mod wavelet_tree;

pub fn compute(isa: &[usize]) -> Vec<usize> {
    let mut psi: Vec<usize> = Vec::with_capacity(isa.len());
    psi.resize(isa.len(), 0);
    psi[isa[isa.len() - 1]] = isa[0];
    for i in 1..isa.len() {
        psi[isa[i - 1]] = isa[i];
    }
    psi
}

pub trait Psi {
    fn new<T, B>(sigma: &Sigma<T, B>, psi: &[usize]) -> Self
    where
        T: Copy + Clone + Eq + Hash + Ord,
        B: crate::bit_vector::OldBitVector;
    fn len(&self) -> usize;
    fn lookup<T, B>(&self, sigma: &crate::Sigma<T, B>, idx: usize) -> usize
    where
        T: Copy + Clone + Eq + Hash + Ord,
        B: crate::bit_vector::OldBitVector;
    fn constrain<T, B>(
        &self,
        sigma: &crate::Sigma<T, B>,
        range: (usize, usize),
        into: (usize, usize),
    ) -> (usize, usize)
    where
        T: Copy + Clone + Eq + Hash + Ord,
        B: crate::bit_vector::OldBitVector;
}

pub struct ReferencePsi {
    psi: Vec<usize>,
}

impl Psi for ReferencePsi {
    fn new<T, B>(_sigma: &Sigma<T, B>, psi: &[usize]) -> Self
    where
        T: Copy + Clone + Eq + Hash + Ord,
        B: crate::bit_vector::OldBitVector,
    {
        Self { psi: psi.to_vec() }
    }

    fn len(&self) -> usize {
        self.psi.len()
    }

    fn lookup<T, B>(&self, _sigma: &Sigma<T, B>, idx: usize) -> usize
    where
        T: Copy + Clone + Eq + Hash + Ord,
        B: crate::bit_vector::OldBitVector,
    {
        self.psi[idx]
    }

    fn constrain<T, B>(
        &self,
        _sigma: &crate::Sigma<T, B>,
        range: (usize, usize),
        into: (usize, usize),
    ) -> (usize, usize)
    where
        T: Copy + Clone + Eq + Hash + Ord,
        B: crate::bit_vector::OldBitVector,
    {
        let start = match self.psi[range.0..range.1].binary_search_by(|probe| probe.cmp(&into.0)) {
            Ok(x) => x,
            Err(x) => x,
        } + range.0;
        let limit = match self.psi[range.0..range.1].binary_search_by(|probe| probe.cmp(&into.1)) {
            Ok(x) => x,
            Err(x) => x,
        } + range.0;
        (start, limit)
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
pub mod tests {
    use super::super::psi::wavelet_tree::WaveletTreePsi;
    use super::super::dictionary::ReferenceDictionary;
    use super::super::bit_vector::ReferenceOldBitVector;
    use super::super::wavelet_tree::ReferenceWaveletTree;
    use super::super::psi::ReferencePsi;
    use super::*;

    // this is the isa for mississippi
    pub const ISA: &[usize] = &[5, 4, 11, 9, 3, 10, 8, 2, 7, 6, 1, 0];
    pub const PSI: &[usize] = &[5, 0, 7, 10, 11, 4, 1, 6, 2, 3, 8, 9];

    // ((expect), (range), (into))
    pub const CONSTRAIN: &[((usize, usize), (usize, usize), (usize, usize))] = &[
        ((3, 5), (1, 5), (8, 12)),
        ((1, 2), (1, 5), (0, 1)),
        ((12, 12), (8, 12), (5, 5)),
        ((12, 12), (8, 12), (6, 8)),
    ];

    #[test]
    fn compute() {
        let returned: &[usize] = &super::compute(ISA);
        assert_eq!(PSI, returned);
    }

    pub fn len<P>(new: fn(&Sigma<char, ReferenceOldBitVector>, &[usize]) -> P)
    where
        P: Psi,
    {
        let sigma = Sigma::test_hack(&['i', 'm', 'p', 's'], &[2, 1, 4, 4, 1, 4, 4, 1, 3, 3, 1, 0]);
        let psi = new(&sigma, PSI);
        assert_eq!(12, psi.len());
    }

    pub fn lookup<P>(new: fn(&Sigma<char, ReferenceOldBitVector>, &[usize]) -> P)
    where
        P: Psi,
    {
        let sigma = Sigma::test_hack(&['i', 'm', 'p', 's'], &[2, 1, 4, 4, 1, 4, 4, 1, 3, 3, 1, 0]);
        let psi = new(&sigma, PSI);
        for i in 0..PSI.len() {
            assert_eq!(PSI[i], psi.lookup(&sigma, i));
        }
    }

    pub fn constrain<P>(new: fn(&Sigma<char, ReferenceOldBitVector>, &[usize]) -> P)
    where
        P: Psi,
    {
        let sigma = Sigma::test_hack(&['i', 'm', 'p', 's'], &[2, 1, 4, 4, 1, 4, 4, 1, 3, 3, 1, 0]);
        let psi = new(&sigma, PSI);
        for (expect, range, into) in CONSTRAIN {
            let result = psi.constrain(&sigma, *range, *into);
            if expect.0 == expect.1 {
                assert_eq!(result.0, result.1);
            } else {
                assert_eq!(*expect, result);
            }
        }
    }

    // TODO(rescrv): bad cases, like OOB and multi-column ranges

    macro_rules! test_Psi {
        ($name:ident, $PSI:path) => {
            mod $name {
                use $crate::psi::Psi;
                use $crate::psi::ReferencePsi;

                #[test]
                fn len() {
                    $crate::psi::tests::len(<$PSI>::new);
                }

                #[test]
                fn lookup() {
                    $crate::psi::tests::lookup(<$PSI>::new);
                }

                #[test]
                fn constrain() {
                    $crate::psi::tests::constrain(<$PSI>::new);
                }
            }
        };
    }

    pub(crate) use test_Psi;
}
