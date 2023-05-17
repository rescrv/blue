pub mod rrr;

///////////////////////////////////////////// BitVector ////////////////////////////////////////////

/// An [OldBitVector] is a sequence of 0 and 1 valued items.  It is called old because it is the
/// old interface.
pub trait OldBitVector {
    /// Create a new [BitVector] from the provided bits.  Can be assumed to be less than sparse.
    fn new(bv: &[bool]) -> Self;
    /// Create a new sparse bit vector from the indexed bits.
    fn sparse(bv: &[usize]) -> Self;

    /// The length of this [BitVector].  Always one more than the highest bit.
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool { self.len() == 0 }

    /// Computes `access[x]`, the value of the x'th bit.
    fn access(&self, x: usize) -> bool;
    /// Computes `rank[x]`, the number of bits set at i < x.
    fn rank(&self, x: usize) -> usize;
    /// Select the x'th bit from this set.  An index.
    fn select(&self, x: usize) -> usize;
}

//////////////////////////////////////// ReferenceBitVector ////////////////////////////////////////

/// A [ReferenceBitVector] provides an inefficient, but easy to understand and verify, bit vector.
pub struct ReferenceBitVector {
    bv: Vec<bool>,
    ranks: Vec<usize>,
    selects: Vec<usize>,
}

impl OldBitVector for ReferenceBitVector {
    fn new(bv: &[bool]) -> Self {
        let mut ranks = Vec::with_capacity(bv.len() + 1);
        let mut selects = Vec::with_capacity(bv.len() + 1);
        let mut rank: usize = 0;
        selects.push(0);
        for i in 0..bv.len() {
            ranks.push(rank);
            if bv[i] {
                rank += 1;
                selects.push(i + 1);
            }
        }
        ranks.push(rank);
        Self {
            bv: bv.to_vec(),
            ranks,
            selects,
        }
    }

    fn sparse(bv: &[usize]) -> Self {
        let mut bits: Vec<bool> = Vec::new();
        for &i in bv {
            if bits.len() <= i {
                bits.resize(i + 1, false);
            }
            bits[i] = true;
        }
        return Self::new(&bits);
    }

    fn len(&self) -> usize {
        self.bv.len()
    }

    fn access(&self, x: usize) -> bool {
        assert!(x < self.bv.len());
        self.bv[x]
    }

    fn rank(&self, x: usize) -> usize {
        assert!(x <= self.bv.len());
        self.ranks[x]
    }

    fn select(&self, x: usize) -> usize {
        assert!(x <= self.rank(self.bv.len()));
        self.selects[x]
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
pub mod tests {
    pub mod evens {
        use super::super::OldBitVector;

        pub const EVENS: &[bool] = &[false, true, false, true, false, true];

        pub fn access<BV: OldBitVector>(bv: BV) {
            assert_eq!(EVENS[0], bv.access(0));
            assert_eq!(EVENS[1], bv.access(1));
            assert_eq!(EVENS[2], bv.access(2));
            assert_eq!(EVENS[3], bv.access(3));
            assert_eq!(EVENS[4], bv.access(4));
            assert_eq!(EVENS[5], bv.access(5));
        }

        pub fn rank<BV: OldBitVector>(bv: BV) {
            assert_eq!(0, bv.rank(0));
            assert_eq!(0, bv.rank(1));
            assert_eq!(1, bv.rank(2));
            assert_eq!(1, bv.rank(3));
            assert_eq!(2, bv.rank(4));
            assert_eq!(2, bv.rank(5));
            assert_eq!(3, bv.rank(6));
        }

        pub fn select<BV: OldBitVector>(bv: BV) {
            assert_eq!(0, bv.select(0));
            assert_eq!(2, bv.select(1));
            assert_eq!(4, bv.select(2));
            assert_eq!(6, bv.select(3));
        }
    }

    pub mod odds {
        use super::super::OldBitVector;

        pub const ODDS: &[bool] = &[true, false, true, false, true, false];

        pub fn access<BV: OldBitVector>(bv: BV) {
            assert_eq!(ODDS[0], bv.access(0));
            assert_eq!(ODDS[1], bv.access(1));
            assert_eq!(ODDS[2], bv.access(2));
            assert_eq!(ODDS[3], bv.access(3));
            assert_eq!(ODDS[4], bv.access(4));
            assert_eq!(ODDS[5], bv.access(5));
        }

        pub fn rank<BV: OldBitVector>(bv: BV) {
            assert_eq!(0, bv.rank(0));
            assert_eq!(1, bv.rank(1));
            assert_eq!(1, bv.rank(2));
            assert_eq!(2, bv.rank(3));
            assert_eq!(2, bv.rank(4));
            assert_eq!(3, bv.rank(5));
            assert_eq!(3, bv.rank(6));
        }

        pub fn select<BV: OldBitVector>(bv: BV) {
            assert_eq!(0, bv.select(0));
            assert_eq!(1, bv.select(1));
            assert_eq!(3, bv.select(2));
            assert_eq!(5, bv.select(3));
        }
    }

    pub mod half_empty {
        use super::super::OldBitVector;

        pub const HALF_EMPTY: &[bool] = &[false, false, false, true, true, true];

        pub fn access<BV: OldBitVector>(bv: BV) {
            assert_eq!(HALF_EMPTY[0], bv.access(0));
            assert_eq!(HALF_EMPTY[1], bv.access(1));
            assert_eq!(HALF_EMPTY[2], bv.access(2));
            assert_eq!(HALF_EMPTY[3], bv.access(3));
            assert_eq!(HALF_EMPTY[4], bv.access(4));
            assert_eq!(HALF_EMPTY[5], bv.access(5));
        }

        pub fn rank<BV: OldBitVector>(bv: BV) {
            assert_eq!(0, bv.rank(0));
            assert_eq!(0, bv.rank(1));
            assert_eq!(0, bv.rank(2));
            assert_eq!(0, bv.rank(3));
            assert_eq!(1, bv.rank(4));
            assert_eq!(2, bv.rank(5));
            assert_eq!(3, bv.rank(6));
        }

        pub fn select<BV: OldBitVector>(bv: BV) {
            assert_eq!(0, bv.select(0));
            assert_eq!(4, bv.select(1));
            assert_eq!(5, bv.select(2));
            assert_eq!(6, bv.select(3));
        }
    }

    macro_rules! test_BitVector {
        ($name:ident, $BV:path) => {
            mod $name {
                mod evens {
                    use $crate::bit_vector::{OldBitVector, ReferenceBitVector};

                    #[test]
                    fn access() {
                        let bytes: &[u8] = &<$BV>::create_from_dense($crate::bit_vector::tests::evens::EVENS.iter().copied());
                        let bv: $BV = <$BV>::new(bytes).unwrap();
                        $crate::bit_vector::tests::evens::access(bv);
                    }

                    #[test]
                    fn rank() {
                        let bytes: &[u8] = &<$BV>::create_from_dense($crate::bit_vector::tests::evens::EVENS.iter().copied());
                        let bv: $BV = <$BV>::new(bytes).unwrap();
                        $crate::bit_vector::tests::evens::rank(bv);
                    }

                    #[test]
                    fn select() {
                        let bytes: &[u8] = &<$BV>::create_from_dense($crate::bit_vector::tests::evens::EVENS.iter().copied());
                        let bv: $BV = <$BV>::new(bytes).unwrap();
                        $crate::bit_vector::tests::evens::select(bv);
                    }
                }

                mod odds {
                    use $crate::bit_vector::OldBitVector;
                    use $crate::reference::*;

                    #[test]
                    fn access() {
                        let bytes: &[u8] = &<$BV>::create_from_dense($crate::bit_vector::tests::odds::ODDS.iter().copied());
                        let bv: $BV = <$BV>::new(bytes).unwrap();
                        $crate::bit_vector::tests::odds::access(bv);
                    }

                    #[test]
                    fn rank() {
                        let bytes: &[u8] = &<$BV>::create_from_dense($crate::bit_vector::tests::odds::ODDS.iter().copied());
                        let bv: $BV = <$BV>::new(bytes).unwrap();
                        $crate::bit_vector::tests::odds::rank(bv);
                    }

                    #[test]
                    fn select() {
                        let bytes: &[u8] = &<$BV>::create_from_dense($crate::bit_vector::tests::odds::ODDS.iter().copied());
                        let bv: $BV = <$BV>::new(bytes).unwrap();
                        $crate::bit_vector::tests::odds::select(bv);
                    }
                }

                mod half_empty {
                    use $crate::bit_vector::OldBitVector;
                    use $crate::reference::*;

                    #[test]
                    fn access() {
                        let bytes: &[u8] = &<$BV>::create_from_dense($crate::bit_vector::tests::half_empty::HALF_EMPTY.iter().copied());
                        let bv: $BV = <$BV>::new(bytes).unwrap();
                        $crate::bit_vector::tests::half_empty::access(bv);
                    }

                    #[test]
                    fn rank() {
                        let bytes: &[u8] = &<$BV>::create_from_dense($crate::bit_vector::tests::half_empty::HALF_EMPTY.iter().copied());
                        let bv: $BV = <$BV>::new(bytes).unwrap();
                        $crate::bit_vector::tests::half_empty::rank(bv);
                    }

                    #[test]
                    fn select() {
                        let bytes: &[u8] = &<$BV>::create_from_dense($crate::bit_vector::tests::half_empty::HALF_EMPTY.iter().copied());
                        let bv: $BV = <$BV>::new(bytes).unwrap();
                        $crate::bit_vector::tests::half_empty::select(bv);
                    }
                }
            }
        };
    }

    pub(crate) use test_BitVector;
}
