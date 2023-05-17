use buffertk::{Buffer, Unpackable};

use super::bit_array::BitArray;
use super::Error;

pub mod rrr;

///////////////////////////////////////////// BitVector ////////////////////////////////////////////

pub trait BitVector<'a>: Unpackable<'a> {
    /// The length of this [BitVector].  Always one more than the highest bit.
    fn len(&self) -> usize;
    /// A [BitVector] `is_empty` when it has zero bits.
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
    bits: Vec<bool>,
    ranks: Vec<usize>,
    selects: Vec<usize>,
}

impl<'a> Unpackable<'a> for ReferenceBitVector {
    type Error = Error;
    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Self::Error> {
        let mut bits = Vec::with_capacity(buf.len() * 8);
        for byte in buf {
            for bit in 0..8 {
                bits.push(byte & (1 << bit) != 0)
            }
        }
        let mut ranks = Vec::with_capacity(bits.len() + 1);
        let mut selects = Vec::with_capacity(bits.len() + 1);
        let mut rank: usize = 0;
        selects.push(0);
        for i in 0..bits.len() {
            ranks.push(rank);
            if bits[i] {
                rank += 1;
                selects.push(i + 1);
            }
        }
        ranks.push(rank);
        Ok((Self {
            bits,
            ranks,
            selects,
        }, &[]))
    }
}

impl<'a> BitVector<'a> for ReferenceBitVector {
    fn len(&self) -> usize {
        self.bits.len()
    }

    fn access(&self, x: usize) -> bool {
        assert!(x < self.bits.len());
        self.bits[x]
    }

    fn rank(&self, x: usize) -> usize {
        assert!(x <= self.bits.len());
        self.ranks[x]
    }

    fn select(&self, x: usize) -> usize {
        assert!(x <= self.rank(self.bits.len()));
        self.selects[x]
    }
}

/////////////////////////////////////////// OldBitVector ///////////////////////////////////////////

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

/////////////////////////////////////// ReferenceOldBitVector //////////////////////////////////////

/// A [ReferenceOldBitVector] provides an inefficient, but easy to understand and verify, bit vector.
pub struct ReferenceOldBitVector {
    bv: Vec<bool>,
    ranks: Vec<usize>,
    selects: Vec<usize>,
}

impl OldBitVector for ReferenceOldBitVector {
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
    use buffertk::Buffer;

    use super::super::bit_array::BitArray;
    use super::{BitVector, ReferenceBitVector};

    trait TestBitVector<'a>: BitVector<'a> {
        fn construct(case: &[bool]) -> Buffer;
    }

    impl<'a> TestBitVector<'a> for ReferenceBitVector {
        fn construct(case: &[bool]) -> Buffer {
            BitArray::construct(case.iter().copied())
        }
    }

    pub mod evens {
        use super::{BitVector, TestBitVector};

        pub const EVENS: &[bool] = &[false, true, false, true, false, true];

        pub fn access<'a, BV: BitVector<'a>>(bv: BV) {
            assert_eq!(EVENS[0], bv.access(0));
            assert_eq!(EVENS[1], bv.access(1));
            assert_eq!(EVENS[2], bv.access(2));
            assert_eq!(EVENS[3], bv.access(3));
            assert_eq!(EVENS[4], bv.access(4));
            assert_eq!(EVENS[5], bv.access(5));
        }

        pub fn rank<'a, BV: BitVector<'a>>(bv: BV) {
            assert_eq!(0, bv.rank(0));
            assert_eq!(0, bv.rank(1));
            assert_eq!(1, bv.rank(2));
            assert_eq!(1, bv.rank(3));
            assert_eq!(2, bv.rank(4));
            assert_eq!(2, bv.rank(5));
            assert_eq!(3, bv.rank(6));
        }

        pub fn select<'a, BV: BitVector<'a>>(bv: BV) {
            assert_eq!(0, bv.select(0));
            assert_eq!(2, bv.select(1));
            assert_eq!(4, bv.select(2));
            assert_eq!(6, bv.select(3));
        }
    }

    pub mod odds {
        use super::{BitVector, TestBitVector};

        pub const ODDS: &[bool] = &[true, false, true, false, true, false];

        pub fn access<'a, BV: BitVector<'a>>(bv: BV) {
            assert_eq!(ODDS[0], bv.access(0));
            assert_eq!(ODDS[1], bv.access(1));
            assert_eq!(ODDS[2], bv.access(2));
            assert_eq!(ODDS[3], bv.access(3));
            assert_eq!(ODDS[4], bv.access(4));
            assert_eq!(ODDS[5], bv.access(5));
        }

        pub fn rank<'a, BV: BitVector<'a>>(bv: BV) {
            assert_eq!(0, bv.rank(0));
            assert_eq!(1, bv.rank(1));
            assert_eq!(1, bv.rank(2));
            assert_eq!(2, bv.rank(3));
            assert_eq!(2, bv.rank(4));
            assert_eq!(3, bv.rank(5));
            assert_eq!(3, bv.rank(6));
        }

        pub fn select<'a, BV: BitVector<'a>>(bv: BV) {
            assert_eq!(0, bv.select(0));
            assert_eq!(1, bv.select(1));
            assert_eq!(3, bv.select(2));
            assert_eq!(5, bv.select(3));
        }
    }

    pub mod half_empty {
        use super::{BitVector, TestBitVector};

        pub const HALF_EMPTY: &[bool] = &[false, false, false, true, true, true];

        pub fn access<'a, BV: BitVector<'a>>(bv: BV) {
            assert_eq!(HALF_EMPTY[0], bv.access(0));
            assert_eq!(HALF_EMPTY[1], bv.access(1));
            assert_eq!(HALF_EMPTY[2], bv.access(2));
            assert_eq!(HALF_EMPTY[3], bv.access(3));
            assert_eq!(HALF_EMPTY[4], bv.access(4));
            assert_eq!(HALF_EMPTY[5], bv.access(5));
        }

        pub fn rank<'a, BV: BitVector<'a>>(bv: BV) {
            assert_eq!(0, bv.rank(0));
            assert_eq!(0, bv.rank(1));
            assert_eq!(0, bv.rank(2));
            assert_eq!(0, bv.rank(3));
            assert_eq!(1, bv.rank(4));
            assert_eq!(2, bv.rank(5));
            assert_eq!(3, bv.rank(6));
        }

        pub fn select<'a, BV: BitVector<'a>>(bv: BV) {
            assert_eq!(0, bv.select(0));
            assert_eq!(4, bv.select(1));
            assert_eq!(5, bv.select(2));
            assert_eq!(6, bv.select(3));
        }
    }

    macro_rules! test_BitVector {
        ($name:ident, $BV:ident) => {
            mod $name {
                use super::{BitVector, TestBitVector};
                use super::$BV;

                mod evens {
                    use buffertk::{Buffer, Unpackable};
                    use super::{BitVector, TestBitVector};
                    use super::$BV;

                    fn bitvector() -> $BV {
                        let case = $crate::bit_vector::tests::evens::EVENS;
                        let buf: Buffer = <$BV as TestBitVector>::construct(case);
                        let bytes: &[u8] = buf.as_bytes();
                        <$BV as Unpackable>::unpack(bytes).unwrap().0
                    }

                    #[test]
                    fn access() {
                        $crate::bit_vector::tests::evens::access(bitvector());
                    }

                    #[test]
                    fn rank() {
                        $crate::bit_vector::tests::evens::rank(bitvector());
                    }

                    #[test]
                    fn select() {
                        $crate::bit_vector::tests::evens::select(bitvector());
                    }
                }

                mod odds {
                    use buffertk::{Buffer, Unpackable};
                    use super::{BitVector, TestBitVector};
                    use super::$BV;

                    fn bitvector() -> $BV {
                        let case = $crate::bit_vector::tests::odds::ODDS;
                        let buf: Buffer = <$BV as TestBitVector>::construct(case);
                        let bytes: &[u8] = buf.as_bytes();
                        <$BV as Unpackable>::unpack(bytes).unwrap().0
                    }

                    #[test]
                    fn access() {
                        $crate::bit_vector::tests::odds::access(bitvector());
                    }

                    #[test]
                    fn rank() {
                        $crate::bit_vector::tests::odds::rank(bitvector());
                    }

                    #[test]
                    fn select() {
                        $crate::bit_vector::tests::odds::select(bitvector());
                    }
                }

                mod half_empty {
                    use buffertk::{Buffer, Unpackable};
                    use super::{BitVector, TestBitVector};
                    use super::$BV;

                    fn bitvector() -> $BV {
                        let case = $crate::bit_vector::tests::half_empty::HALF_EMPTY;
                        let buf: Buffer = <$BV as TestBitVector>::construct(case);
                        let bytes: &[u8] = buf.as_bytes();
                        <$BV as Unpackable>::unpack(bytes).unwrap().0
                    }

                    #[test]
                    fn access() {
                        $crate::bit_vector::tests::half_empty::access(bitvector());
                    }

                    #[test]
                    fn rank() {
                        $crate::bit_vector::tests::half_empty::rank(bitvector());
                    }

                    #[test]
                    fn select() {
                        $crate::bit_vector::tests::half_empty::select(bitvector());
                    }
                }
            }
        };
    }

    pub(crate) use test_BitVector;

    test_BitVector!(reference, ReferenceBitVector);
}
