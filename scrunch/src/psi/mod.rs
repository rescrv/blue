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
        B: crate::bit_vector::BitVector;
    fn len(&self) -> usize;
    fn lookup<T, B>(&self, sigma: &crate::Sigma<T, B>, idx: usize) -> usize
    where
        T: Copy + Clone + Eq + Hash + Ord,
        B: crate::bit_vector::BitVector;
    fn constrain<T, B>(
        &self,
        sigma: &crate::Sigma<T, B>,
        range: (usize, usize),
        into: (usize, usize),
    ) -> (usize, usize)
    where
        T: Copy + Clone + Eq + Hash + Ord,
        B: crate::bit_vector::BitVector;
}

pub struct ReferencePsi {
    psi: Vec<usize>,
}

impl Psi for ReferencePsi {
    fn new<T, B>(_sigma: &Sigma<T, B>, psi: &[usize]) -> Self
    where
        T: Copy + Clone + Eq + Hash + Ord,
        B: crate::bit_vector::BitVector,
    {
        Self { psi: psi.to_vec() }
    }

    fn len(&self) -> usize {
        self.psi.len()
    }

    fn lookup<T, B>(&self, _sigma: &Sigma<T, B>, idx: usize) -> usize
    where
        T: Copy + Clone + Eq + Hash + Ord,
        B: crate::bit_vector::BitVector,
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
        B: crate::bit_vector::BitVector,
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

#[cfg(test)]
mod tests {
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

    fn len<P>(new: fn(&Sigma<char, crate::bit_vector::ReferenceBitVector>, &[usize]) -> P)
    where
        P: Psi,
    {
        let sigma = Sigma::dirty_hack(&['i', 'm', 'p', 's'], &[2, 1, 4, 4, 1, 4, 4, 1, 3, 3, 1, 0]);
        let psi = new(&sigma, PSI);
        assert_eq!(12, psi.len());
    }

    fn lookup<P>(new: fn(&Sigma<char, crate::bit_vector::ReferenceBitVector>, &[usize]) -> P)
    where
        P: Psi,
    {
        let sigma = Sigma::dirty_hack(&['i', 'm', 'p', 's'], &[2, 1, 4, 4, 1, 4, 4, 1, 3, 3, 1, 0]);
        let psi = new(&sigma, PSI);
        for i in 0..PSI.len() {
            assert_eq!(PSI[i], psi.lookup(&sigma, i));
        }
    }

    fn constrain<P>(new: fn(&Sigma<char, crate::bit_vector::ReferenceBitVector>, &[usize]) -> P)
    where
        P: Psi,
    {
        let sigma = Sigma::dirty_hack(&['i', 'm', 'p', 's'], &[2, 1, 4, 4, 1, 4, 4, 1, 3, 3, 1, 0]);
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
        ($name:ident, $PSI:expr) => {
            mod $name {
                use super::*;

                #[test]
                fn len() {
                    super::len($PSI);
                }

                #[test]
                fn lookup() {
                    super::lookup($PSI);
                }

                #[test]
                fn constrain() {
                    super::constrain($PSI);
                }
            }
        };
    }

    test_Psi!(reference, ReferencePsi::new);

    use crate::bit_vector::ReferenceBitVector;
    use crate::dictionary::ReferenceDictionary;
    use crate::psi::wavelet_tree::WaveletTreePsi;
    use crate::wavelet_tree::ReferenceWaveletTree;
    test_Psi!(
        wavelet_tree,
        WaveletTreePsi::<
            ReferenceDictionary<ReferenceBitVector, (usize, usize)>,
            ReferenceWaveletTree,
        >::new
    );
}
