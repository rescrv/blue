use buffertk::Unpackable;

use super::bit_array::BitArray;
use super::bit_array::Builder as BitArrayBuilder;
use super::Error;
use crate::binary_search::partition_by;
use crate::builder::{Builder, Helper};

pub mod rrr;
pub mod sparse;

///////////////////////////////////////////// BitVector ////////////////////////////////////////////

pub trait BitVector {
    type Output<'a>: BitVector;

    /// Append the byte-representation of the bit vector to buf.
    fn construct<H: Helper>(bits: &[bool], builder: &mut Builder<'_, H>) -> Result<(), Error>;
    /// Parse the byte-representation of the buffer.
    fn parse<'a, 'b: 'a>(buf: &'b [u8]) -> Result<(Self::Output<'a>, &'b [u8]), Error>;

    /// The length of this [BitVector].  Always one more than the highest bit.
    fn len(&self) -> usize;
    /// A [BitVector] `is_empty` when it has zero bits.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Computes `access[x]`, the value of the x'th bit.
    fn access(&self, x: usize) -> Option<bool>;
    /// Computes `rank[x]`, the number of bits set at i < x.
    fn rank(&self, x: usize) -> Option<usize>;
    /// Select the x'th bit from this set.  An index.
    ///
    /// A default implementation is provided that uses binary search over rank.
    fn select(&self, x: usize) -> Option<usize> {
        // SAFETY(rescrv): rank is defined for [0, self.len()]
        let left = partition_by(0, self.len(), |mid| self.rank(mid).unwrap() < x);
        if self.rank(left) == Some(x) {
            Some(left)
        } else {
            None
        }
    }

    /// Computes `rank_0[x]`, the number of bits unset at i < x.
    ///
    /// Note that bit vectors may not implement this efficiently, so a default implementation that
    /// uses x - rank[x] is provided.
    fn rank0(&self, x: usize) -> Option<usize> {
        Some(x - self.rank(x)?)
    }

    /// Select the x'th 0-valued bit from this set.  An index.
    ///
    /// Note that bit vectors may not implement this efficiently, so a default implementation that
    /// uses binary-search by rank0 is provided.
    fn select0(&self, x: usize) -> Option<usize> {
        // SAFETY(rescrv): rank is defined for [0, self.len()]
        let left = partition_by(0, self.len(), |mid| self.rank0(mid).unwrap() < x);
        if self.rank0(left) == Some(x) {
            Some(left)
        } else {
            None
        }
    }
}

////////////////////////////////////// ReferenceBitVectorStub //////////////////////////////////////

#[derive(Clone, Debug, Default, prototk_derive::Message)]
struct ReferenceBitVectorStub<'a> {
    #[prototk(1, uint64)]
    length: u64,
    #[prototk(2, bytes)]
    bits: &'a [u8],
}

//////////////////////////////////////// ReferenceBitVector ////////////////////////////////////////

/// A [ReferenceBitVector] provides an inefficient, but easy to understand and verify, bit vector.
pub struct ReferenceBitVector<'a> {
    length: usize,
    bits: BitArray<'a>,
    ranks: Vec<usize>,
    selects: Vec<usize>,
    selects0: Vec<usize>,
}

impl<'a> BitVector for ReferenceBitVector<'a> {
    type Output<'b> = ReferenceBitVector<'b>;

    fn construct<H: Helper>(bits: &[bool], builder: &mut Builder<'_, H>) -> Result<(), Error> {
        let length: u64 = bits.len() as u64;
        let mut bit_builder = BitArrayBuilder::with_capacity(bits.len());
        for bit in bits {
            bit_builder.push(*bit);
        }
        let bits = bit_builder.seal();
        builder.append_raw_packable(&ReferenceBitVectorStub {
            length,
            bits: bits.as_slice(),
        });
        Ok(())
    }

    fn parse<'b, 'c: 'b>(buf: &'c [u8]) -> Result<(Self::Output<'b>, &'c [u8]), Error> {
        let (stub, buf) =
            ReferenceBitVectorStub::unpack(buf).map_err(|_| Error::InvalidBitVector)?;
        let length: usize = stub.length.try_into()?;
        let bits: BitArray<'b> = BitArray::new(stub.bits);
        let mut ranks = Vec::with_capacity(length + 1);
        let mut selects = Vec::with_capacity(length + 1);
        let mut selects0 = Vec::with_capacity(length + 1);
        let mut rank: usize = 0;
        selects.push(0);
        selects0.push(0);
        for i in 0..length {
            ranks.push(rank);
            let err = Error::BadRank(i);
            if bits.get(i).ok_or(err)? {
                rank += 1;
                selects.push(i + 1);
            } else {
                selects0.push(i + 1);
            }
        }
        ranks.push(rank);
        Ok((
            Self::Output {
                length,
                bits,
                ranks,
                selects,
                selects0,
            },
            buf,
        ))
    }

    fn len(&self) -> usize {
        self.length
    }

    fn access(&self, x: usize) -> Option<bool> {
        if x < self.len() {
            self.bits.get(x)
        } else {
            None
        }
    }

    fn rank(&self, x: usize) -> Option<usize> {
        self.ranks.get(x).copied()
    }

    fn select(&self, x: usize) -> Option<usize> {
        self.selects.get(x).copied()
    }

    fn select0(&self, x: usize) -> Option<usize> {
        self.selects0.get(x).copied()
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
pub mod tests {
    use super::BitVector;

    struct TestCase {
        bits: &'static [bool],
        ranks: &'static [usize],
        ranks0: &'static [usize],
        selects: &'static [usize],
        selects0: &'static [usize],
    }

    impl TestCase {
        fn access<BV: BitVector>(&self, bv: &BV) {
            for (idx, bit) in self.bits.iter().enumerate() {
                assert_eq!(Some(*bit), bv.access(idx));
            }
            assert_eq!(None, bv.access(self.bits.len()));
        }

        fn rank<BV: BitVector>(&self, bv: &BV) {
            for (idx, rank) in self.ranks.iter().enumerate() {
                assert_eq!(Some(*rank), bv.rank(idx));
            }
            assert_eq!(None, bv.rank(self.ranks.len()));
        }

        fn rank0<BV: BitVector>(&self, bv: &BV) {
            for (idx, rank) in self.ranks0.iter().enumerate() {
                assert_eq!(Some(*rank), bv.rank0(idx));
            }
            assert_eq!(None, bv.rank0(self.ranks.len()));
        }

        fn select<BV: BitVector>(&self, bv: &BV) {
            for (idx, select) in self.selects.iter().enumerate() {
                assert_eq!(Some(*select), bv.select(idx));
            }
            assert_eq!(None, bv.select(self.selects.len()));
        }

        fn select0<BV: BitVector>(&self, bv: &BV) {
            for (idx, select) in self.selects0.iter().enumerate() {
                assert_eq!(Some(*select), bv.select0(idx));
            }
            assert_eq!(None, bv.select0(self.selects0.len()));
        }
    }

    const EMPTY: &TestCase = &TestCase {
        bits: &[],
        ranks: &[0],
        ranks0: &[0],
        selects: &[0],
        selects0: &[0],
    };

    const ONE_BIT_FALSE: &TestCase = &TestCase {
        bits: &[false],
        ranks: &[0, 0],
        ranks0: &[0, 1],
        selects: &[0],
        selects0: &[0, 1],
    };

    const ONE_BIT_TRUE: &TestCase = &TestCase {
        bits: &[true],
        ranks: &[0, 1],
        ranks0: &[0, 0],
        selects: &[0, 1],
        selects0: &[0],
    };

    const EVENS: &TestCase = &TestCase {
        bits: &[false, true, false, true, false, true],
        ranks: &[0, 0, 1, 1, 2, 2, 3],
        ranks0: &[0, 1, 1, 2, 2, 3, 3],
        selects: &[0, 2, 4, 6],
        selects0: &[0, 1, 3, 5],
    };

    const ODDS: &TestCase = &TestCase {
        bits: &[true, false, true, false, true, false],
        ranks: &[0, 1, 1, 2, 2, 3, 3],
        ranks0: &[0, 0, 1, 1, 2, 2, 3],
        selects: &[0, 1, 3, 5],
        selects0: &[0, 2, 4, 6],
    };

    const HALF_EMPTY: &TestCase = &TestCase {
        bits: &[false, false, false, true, true, true],
        ranks: &[0, 0, 0, 0, 1, 2, 3],
        ranks0: &[0, 1, 2, 3, 3, 3, 3],
        selects: &[0, 4, 5, 6],
        selects0: &[0, 1, 2, 3],
    };

    const BAD_TREE_NODE_1: &TestCase = &TestCase {
        bits: &[
            true, true, true, true, true, true, true, true, true, false, true, false, true, true,
            true, false, false, true, true, false, false, false, false, false, true, true, false,
            true, true, true, true, false, false, true, true, true, true, true, true, true, false,
            true, true, true, true, true, true, true, true, true, true, true, true, false, true,
            false, true, false, true, true, false, false, false, false, true, true, true, false,
            false, true, true, false, true, true, true, true, false, false, false, true, false,
            false, false, false, false, false, false, true, true, false, true, false, true, true,
            false, false, true, true, false, false, true, false, false, false, false, false, false,
            false, false, false, false, false, false, false, true, false, false, false, true, true,
            true, false, true, false, false, false, false, false, false, false, false, true, false,
            false, false, false, false, false, false, false, true, false, true, false, false,
            false, false, false, false, false, false, false, true, false, false, false, true, true,
            false, false, true, false, false, false, false, false, false, false, false, true,
            false, false, false, true, false, true, true, false, false, true, true, false, true,
            false, false, false, true, true, false, false, false, false, true, true, false, false,
            false, false, false, true, true, true, false, true, true, false, true, true, true,
            true, true, true, true, true, false, true, true, false, false, true, true, true, false,
            false, true, false, false, false, true, true, true, true, true, false, true, true,
            true, true, false, true, true, true, false, false, true, false, true, false, true,
            false, true, true, true, true, true, true, true, true, false, true, false, false,
            false, false, false, false, true, true, true, false, true, true, false, false, true,
            false, false, true, false, true, true, true, true, false, true, true, true, true, true,
            true, true, true, true, true, true, false, false, true, false, false, false, false,
            false, false, true, true, false, true, true, false, true, true, true, true, false,
            false, true, true, false, true, false, false, true, true, true, true, true, true, true,
            false, false, true, true, false, true, true, true, true, true, true, true, false, true,
            false, true, true, false, false, true, false, true, true, false, false, true, true,
            true, false, true, false, true, true, false, true, true, true, true, true, false, true,
            true, true, true, true, true, false, true, true, false, false, false, true, true,
            false, true, true, true, true, true, true, true, true, false, true, true, true, true,
            true, false, false, true, false, true, false, false, false, false, true, true, true,
            false, true, true, true, true, true, true, true, false, false, false, false, true,
            true, false, false, false, false, false, true, false, true, false, false, true, false,
            false, true, true, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, true, true, true, true, false, false, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, false, true,
            true, true, true, false, false, true, true, true, false, true, true, false, false,
            false, true, false, false, true, true, true, true, true, false, false, true, true,
            true, true, false, true, true, false, true, true, true, true, true, true, true, true,
            true, true, true, true, false, true, false, true, false, true, true, false, true, true,
            true, false, false, true, false, true, false, false, true, true, true, true, true,
            true, true, true, false, true, true, true, false, true, true, false, true, true, true,
            true, false, true, true, true, true, true, true, true, true, true, true, true, true,
            false, false, false, false, true, false, false, false, true, false, false, true, false,
            true, false, false, false, false, false, true, false, false, false, false, false, true,
            true, true, true, false, true, true, false, false, true, true, false, false, true,
            false, false, true, false, false, false, true, true, true, true, true, true, true,
            true, true, false, true, false, false, false, false, false, false, false, false, true,
            true, true, true, true, true, true, true, true, false, true, false, false, true, true,
            false, true, false, true, false, true, true, true, false, true, true, false, false,
            true, false, true, false, false, true, false, true, true, false, true, false, false,
            false, true, true, true, false, false, true, false, false, false, false, false, false,
            false, false, false, false, false, false, true, false, false, false, false, false,
            true, true, false, true, true, true, true, true, true, true, true, true, true, false,
            true, true, false, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, false, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, false, false, false, false, true, false, false, false,
            false, false, true, true, false, false, true, true, false, false, false, false, false,
            false, false, false, false, true, true, false, false, true, true, false, true, false,
            true, true, true, true, false, true, true, false, true, true, false, true, false,
            false, false, true, true, true, true, true, false, false, true, true, true, true,
            false, false, false, false, false, false, false, false, false, true, true, true, false,
            false, true, false, false, true, false, true, false, true, false, false, true, true,
            false, false, false, false, true, true, true, true, true, true, true, false, true,
            false, true, true, true, true, true, true, false, true, true, true, true, false, true,
            false, false, true, true, true, true, true, false, true, false, true, false, true,
            true, false, false, true, false, true, false, false, false, false, true, true, false,
            true, true, true, true, true, false, false, false, true, false, true, true, false,
            false, true, true, true, true, true, true, false, false, true, true, true, false,
            false, false, false, false, false, false, true, true, false, false, true, true, true,
            true, false, true, false, true, true, true, true, true, false, false, true, false,
            true, true, false, false, true, true, false, true, true, true, true, true, false,
            false, true, true, true, true, false, true, false, false, true, true, true, true, true,
            false, false, false, true, true, false, false, true, true, false, true, true, true,
            true, false, false, false, true, true, false, true, true, true, false, true, false,
            true, true, true, false, false, false, true, false, true, true, false, true, true,
            false, false, true, false, false, false, false, true, false, false, false, true, false,
            false, false, false, false, true, true, false, false, false, false, true, true, true,
            true, false, false, true, true, true, true, false, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, false, true,
            true, true, true, true, true, true, true, true, true, false, true, true, true, true,
            false, true, true, true, true, true, true, false, true, true, false, true, true, false,
            false, true, false, true, true, false, false, false, false, true, true, true, true,
            true, true, false, true, true, true, true, true, true, true, true, true, false, false,
            false, false, false, false, false, false, false, false, true, true, true, false, true,
            false, true, false, false, false, false, false, false, false, true, false, false, true,
            true, true, false, false, false, true, true, true, true, true, true, false, false,
            true, true, true, false, false, true, false, false, false, false, false, true, false,
            false, false, true, false, false, true, true, true, true, true, true, true, true, true,
            false, true, true, true, true, false, true, false, false, false, true, false, false,
            true, false, false, true, false, false, true, false, true, true, false, false, true,
            false, true, true, true, false, false, true, true, true, false, false, true, false,
            false, true, true, false, true, true, true, true, true, false, true, true, false, true,
            true, false, false, true, false, false, false, true, true, false, true, true, false,
            true, false, true, false, false, true, true, true, false, true, true, true, true, true,
            true, false, false, true, true, false, false, true, true, true, true, true, true, true,
            true, true, true, true, true, false, false, true, true, true, true, false, true, true,
            false, true, false, false, false, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, false, true, true,
            true, true, false, true, true, true, true, true, true, false, true, false, false,
            false, false, false, false, true, true, true, false, false, false, false, false, false,
            false, false, true, true, true, false, false, false, false, false, true, false, false,
            false, false, true, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, true, false, false, false,
            false, false, false, true, false, false, true, false, true, false, true, false, false,
            true, true, false, true, false, true, false, true, true, true, true, true, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, true, true, true, true, true, true,
            false, false, false, true, true, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, false, true, true, true, true, true, true, true, true, false, false, false,
            false, false, false, true, false, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, false, true, true, true, false, false, false,
            false, true, false, true, true, true, false, false, false, true, false, true, false,
            true, false, true, false, true, false, false, false, false, false, true, false, false,
            false, false, false, false, false, false, false, false, false, true, false, false,
            false, false, true, true, true, true, true, false, false, false, true, false, false,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, false, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, false,
            false, true, false, false, true, false, true, false, false, false, false, false, true,
            true, true, false, false, true, true, true, false, false, true, true, true, true, true,
            true, true, true, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            true, true, false, false, false, false, false, false, false, false, false, false,
            false, false, false, true, true, true, false, false, false, false, false, false, false,
            false, false, false, false, false, true, true, true, true, false, false, false, true,
            false, false, false, false, true, false, true, false, true, true, false, true,
        ],
        ranks: &[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 9, 10, 10, 11, 12, 13, 13, 13, 14, 15, 15, 15, 15, 15,
            15, 16, 17, 17, 18, 19, 20, 21, 21, 21, 22, 23, 24, 25, 26, 27, 28, 28, 29, 30, 31, 32,
            33, 34, 35, 36, 37, 38, 39, 40, 40, 41, 41, 42, 42, 43, 44, 44, 44, 44, 44, 45, 46, 47,
            47, 47, 48, 49, 49, 50, 51, 52, 53, 53, 53, 53, 54, 54, 54, 54, 54, 54, 54, 54, 55, 56,
            56, 57, 57, 58, 59, 59, 59, 60, 61, 61, 61, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62,
            62, 62, 62, 63, 63, 63, 63, 64, 65, 66, 66, 67, 67, 67, 67, 67, 67, 67, 67, 67, 68, 68,
            68, 68, 68, 68, 68, 68, 68, 69, 69, 70, 70, 70, 70, 70, 70, 70, 70, 70, 70, 71, 71, 71,
            71, 72, 73, 73, 73, 74, 74, 74, 74, 74, 74, 74, 74, 74, 75, 75, 75, 75, 76, 76, 77, 78,
            78, 78, 79, 80, 80, 81, 81, 81, 81, 82, 83, 83, 83, 83, 83, 84, 85, 85, 85, 85, 85, 85,
            86, 87, 88, 88, 89, 90, 90, 91, 92, 93, 94, 95, 96, 97, 98, 98, 99, 100, 100, 100, 101,
            102, 103, 103, 103, 104, 104, 104, 104, 105, 106, 107, 108, 109, 109, 110, 111, 112,
            113, 113, 114, 115, 116, 116, 116, 117, 117, 118, 118, 119, 119, 120, 121, 122, 123,
            124, 125, 126, 127, 127, 128, 128, 128, 128, 128, 128, 128, 129, 130, 131, 131, 132,
            133, 133, 133, 134, 134, 134, 135, 135, 136, 137, 138, 139, 139, 140, 141, 142, 143,
            144, 145, 146, 147, 148, 149, 150, 150, 150, 151, 151, 151, 151, 151, 151, 151, 152,
            153, 153, 154, 155, 155, 156, 157, 158, 159, 159, 159, 160, 161, 161, 162, 162, 162,
            163, 164, 165, 166, 167, 168, 169, 169, 169, 170, 171, 171, 172, 173, 174, 175, 176,
            177, 178, 178, 179, 179, 180, 181, 181, 181, 182, 182, 183, 184, 184, 184, 185, 186,
            187, 187, 188, 188, 189, 190, 190, 191, 192, 193, 194, 195, 195, 196, 197, 198, 199,
            200, 201, 201, 202, 203, 203, 203, 203, 204, 205, 205, 206, 207, 208, 209, 210, 211,
            212, 213, 213, 214, 215, 216, 217, 218, 218, 218, 219, 219, 220, 220, 220, 220, 220,
            221, 222, 223, 223, 224, 225, 226, 227, 228, 229, 230, 230, 230, 230, 230, 231, 232,
            232, 232, 232, 232, 232, 233, 233, 234, 234, 234, 235, 235, 235, 236, 237, 237, 237,
            237, 237, 237, 237, 237, 237, 237, 237, 237, 237, 237, 237, 238, 239, 240, 241, 241,
            241, 242, 243, 244, 245, 246, 247, 248, 249, 250, 251, 252, 253, 254, 255, 255, 256,
            257, 258, 259, 259, 259, 260, 261, 262, 262, 263, 264, 264, 264, 264, 265, 265, 265,
            266, 267, 268, 269, 270, 270, 270, 271, 272, 273, 274, 274, 275, 276, 276, 277, 278,
            279, 280, 281, 282, 283, 284, 285, 286, 287, 288, 288, 289, 289, 290, 290, 291, 292,
            292, 293, 294, 295, 295, 295, 296, 296, 297, 297, 297, 298, 299, 300, 301, 302, 303,
            304, 305, 305, 306, 307, 308, 308, 309, 310, 310, 311, 312, 313, 314, 314, 315, 316,
            317, 318, 319, 320, 321, 322, 323, 324, 325, 326, 326, 326, 326, 326, 327, 327, 327,
            327, 328, 328, 328, 329, 329, 330, 330, 330, 330, 330, 330, 331, 331, 331, 331, 331,
            331, 332, 333, 334, 335, 335, 336, 337, 337, 337, 338, 339, 339, 339, 340, 340, 340,
            341, 341, 341, 341, 342, 343, 344, 345, 346, 347, 348, 349, 350, 350, 351, 351, 351,
            351, 351, 351, 351, 351, 351, 352, 353, 354, 355, 356, 357, 358, 359, 360, 360, 361,
            361, 361, 362, 363, 363, 364, 364, 365, 365, 366, 367, 368, 368, 369, 370, 370, 370,
            371, 371, 372, 372, 372, 373, 373, 374, 375, 375, 376, 376, 376, 376, 377, 378, 379,
            379, 379, 380, 380, 380, 380, 380, 380, 380, 380, 380, 380, 380, 380, 380, 381, 381,
            381, 381, 381, 381, 382, 383, 383, 384, 385, 386, 387, 388, 389, 390, 391, 392, 393,
            393, 394, 395, 395, 396, 397, 398, 399, 400, 401, 402, 403, 404, 405, 406, 407, 408,
            409, 410, 410, 411, 412, 413, 414, 415, 416, 417, 418, 419, 420, 421, 422, 423, 424,
            425, 426, 427, 428, 429, 430, 431, 432, 433, 434, 435, 436, 437, 438, 439, 440, 441,
            442, 443, 444, 445, 446, 447, 448, 449, 450, 451, 452, 453, 454, 455, 456, 457, 458,
            459, 460, 461, 462, 463, 464, 465, 466, 466, 466, 466, 466, 467, 467, 467, 467, 467,
            467, 468, 469, 469, 469, 470, 471, 471, 471, 471, 471, 471, 471, 471, 471, 471, 472,
            473, 473, 473, 474, 475, 475, 476, 476, 477, 478, 479, 480, 480, 481, 482, 482, 483,
            484, 484, 485, 485, 485, 485, 486, 487, 488, 489, 490, 490, 490, 491, 492, 493, 494,
            494, 494, 494, 494, 494, 494, 494, 494, 494, 495, 496, 497, 497, 497, 498, 498, 498,
            499, 499, 500, 500, 501, 501, 501, 502, 503, 503, 503, 503, 503, 504, 505, 506, 507,
            508, 509, 510, 510, 511, 511, 512, 513, 514, 515, 516, 517, 517, 518, 519, 520, 521,
            521, 522, 522, 522, 523, 524, 525, 526, 527, 527, 528, 528, 529, 529, 530, 531, 531,
            531, 532, 532, 533, 533, 533, 533, 533, 534, 535, 535, 536, 537, 538, 539, 540, 540,
            540, 540, 541, 541, 542, 543, 543, 543, 544, 545, 546, 547, 548, 549, 549, 549, 550,
            551, 552, 552, 552, 552, 552, 552, 552, 552, 553, 554, 554, 554, 555, 556, 557, 558,
            558, 559, 559, 560, 561, 562, 563, 564, 564, 564, 565, 565, 566, 567, 567, 567, 568,
            569, 569, 570, 571, 572, 573, 574, 574, 574, 575, 576, 577, 578, 578, 579, 579, 579,
            580, 581, 582, 583, 584, 584, 584, 584, 585, 586, 586, 586, 587, 588, 588, 589, 590,
            591, 592, 592, 592, 592, 593, 594, 594, 595, 596, 597, 597, 598, 598, 599, 600, 601,
            601, 601, 601, 602, 602, 603, 604, 604, 605, 606, 606, 606, 607, 607, 607, 607, 607,
            608, 608, 608, 608, 609, 609, 609, 609, 609, 609, 610, 611, 611, 611, 611, 611, 612,
            613, 614, 615, 615, 615, 616, 617, 618, 619, 619, 620, 621, 622, 623, 624, 625, 626,
            627, 628, 629, 630, 631, 632, 633, 634, 635, 636, 637, 637, 638, 639, 640, 641, 642,
            643, 644, 645, 646, 647, 647, 648, 649, 650, 651, 651, 652, 653, 654, 655, 656, 657,
            657, 658, 659, 659, 660, 661, 661, 661, 662, 662, 663, 664, 664, 664, 664, 664, 665,
            666, 667, 668, 669, 670, 670, 671, 672, 673, 674, 675, 676, 677, 678, 679, 679, 679,
            679, 679, 679, 679, 679, 679, 679, 679, 680, 681, 682, 682, 683, 683, 684, 684, 684,
            684, 684, 684, 684, 684, 685, 685, 685, 686, 687, 688, 688, 688, 688, 689, 690, 691,
            692, 693, 694, 694, 694, 695, 696, 697, 697, 697, 698, 698, 698, 698, 698, 698, 699,
            699, 699, 699, 700, 700, 700, 701, 702, 703, 704, 705, 706, 707, 708, 709, 709, 710,
            711, 712, 713, 713, 714, 714, 714, 714, 715, 715, 715, 716, 716, 716, 717, 717, 717,
            718, 718, 719, 720, 720, 720, 721, 721, 722, 723, 724, 724, 724, 725, 726, 727, 727,
            727, 728, 728, 728, 729, 730, 730, 731, 732, 733, 734, 735, 735, 736, 737, 737, 738,
            739, 739, 739, 740, 740, 740, 740, 741, 742, 742, 743, 744, 744, 745, 745, 746, 746,
            746, 747, 748, 749, 749, 750, 751, 752, 753, 754, 755, 755, 755, 756, 757, 757, 757,
            758, 759, 760, 761, 762, 763, 764, 765, 766, 767, 768, 769, 769, 769, 770, 771, 772,
            773, 773, 774, 775, 775, 776, 776, 776, 776, 777, 778, 779, 780, 781, 782, 783, 784,
            785, 786, 787, 788, 789, 790, 791, 792, 793, 794, 795, 796, 796, 797, 798, 799, 800,
            800, 801, 802, 803, 804, 805, 806, 806, 807, 807, 807, 807, 807, 807, 807, 808, 809,
            810, 810, 810, 810, 810, 810, 810, 810, 810, 811, 812, 813, 813, 813, 813, 813, 813,
            814, 814, 814, 814, 814, 815, 815, 815, 815, 815, 815, 815, 815, 815, 815, 815, 815,
            815, 815, 815, 815, 815, 815, 816, 816, 816, 816, 816, 816, 816, 817, 817, 817, 818,
            818, 819, 819, 820, 820, 820, 821, 822, 822, 823, 823, 824, 824, 825, 826, 827, 828,
            829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829,
            829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829,
            829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829,
            829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829,
            829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829,
            829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829,
            829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 830, 831,
            832, 833, 834, 835, 835, 835, 835, 836, 837, 837, 837, 837, 837, 837, 837, 837, 837,
            837, 837, 837, 837, 837, 837, 837, 837, 837, 837, 837, 837, 837, 837, 837, 837, 837,
            837, 837, 837, 837, 837, 837, 837, 837, 837, 837, 837, 837, 837, 837, 837, 837, 837,
            837, 837, 837, 837, 837, 837, 837, 838, 839, 840, 841, 842, 843, 844, 845, 846, 847,
            848, 849, 850, 851, 852, 853, 854, 855, 856, 857, 858, 859, 860, 861, 862, 863, 864,
            865, 866, 867, 868, 869, 870, 871, 872, 873, 874, 875, 876, 877, 878, 879, 880, 881,
            882, 883, 884, 885, 886, 887, 888, 889, 890, 891, 892, 893, 894, 895, 896, 897, 898,
            899, 900, 901, 901, 902, 903, 904, 905, 906, 907, 908, 909, 909, 909, 909, 909, 909,
            909, 910, 910, 911, 912, 913, 914, 915, 916, 917, 918, 919, 920, 921, 922, 923, 924,
            925, 925, 926, 927, 928, 928, 928, 928, 928, 929, 929, 930, 931, 932, 932, 932, 932,
            933, 933, 934, 934, 935, 935, 936, 936, 937, 937, 937, 937, 937, 937, 938, 938, 938,
            938, 938, 938, 938, 938, 938, 938, 938, 938, 939, 939, 939, 939, 939, 940, 941, 942,
            943, 944, 944, 944, 944, 945, 945, 945, 946, 947, 948, 949, 950, 951, 952, 953, 954,
            955, 956, 957, 958, 959, 960, 961, 962, 963, 964, 965, 965, 966, 967, 968, 969, 970,
            971, 972, 973, 974, 975, 976, 977, 978, 979, 980, 981, 982, 983, 984, 985, 986, 987,
            988, 989, 990, 991, 992, 993, 994, 995, 996, 997, 998, 999, 999, 999, 1000, 1000, 1000,
            1001, 1001, 1002, 1002, 1002, 1002, 1002, 1002, 1003, 1004, 1005, 1005, 1005, 1006,
            1007, 1008, 1008, 1008, 1009, 1010, 1011, 1012, 1013, 1014, 1015, 1016, 1016, 1016,
            1016, 1016, 1016, 1016, 1016, 1016, 1016, 1016, 1016, 1016, 1016, 1016, 1016, 1016,
            1016, 1016, 1016, 1016, 1016, 1016, 1017, 1018, 1018, 1018, 1018, 1018, 1018, 1018,
            1018, 1018, 1018, 1018, 1018, 1018, 1018, 1019, 1020, 1021, 1021, 1021, 1021, 1021,
            1021, 1021, 1021, 1021, 1021, 1021, 1021, 1021, 1022, 1023, 1024, 1025, 1025, 1025,
            1025, 1026, 1026, 1026, 1026, 1026, 1027, 1027, 1028, 1028, 1029, 1030, 1030, 1031,
        ],
        ranks0: &[
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 2, 2, 2, 2, 3, 4, 4, 4, 5, 6, 7, 8, 9, 9, 9, 10,
            10, 10, 10, 10, 11, 12, 12, 12, 12, 12, 12, 12, 12, 13, 13, 13, 13, 13, 13, 13, 13, 13,
            13, 13, 13, 13, 14, 14, 15, 15, 16, 16, 16, 17, 18, 19, 20, 20, 20, 20, 21, 22, 22, 22,
            23, 23, 23, 23, 23, 24, 25, 26, 26, 27, 28, 29, 30, 31, 32, 33, 33, 33, 34, 34, 35, 35,
            35, 36, 37, 37, 37, 38, 39, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 52,
            53, 54, 55, 55, 55, 55, 56, 56, 57, 58, 59, 60, 61, 62, 63, 64, 64, 65, 66, 67, 68, 69,
            70, 71, 72, 72, 73, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 82, 83, 84, 85, 85, 85, 86,
            87, 87, 88, 89, 90, 91, 92, 93, 94, 95, 95, 96, 97, 98, 98, 99, 99, 99, 100, 101, 101,
            101, 102, 102, 103, 104, 105, 105, 105, 106, 107, 108, 109, 109, 109, 110, 111, 112,
            113, 114, 114, 114, 114, 115, 115, 115, 116, 116, 116, 116, 116, 116, 116, 116, 116,
            117, 117, 117, 118, 119, 119, 119, 119, 120, 121, 121, 122, 123, 124, 124, 124, 124,
            124, 124, 125, 125, 125, 125, 125, 126, 126, 126, 126, 127, 128, 128, 129, 129, 130,
            130, 131, 131, 131, 131, 131, 131, 131, 131, 131, 132, 132, 133, 134, 135, 136, 137,
            138, 138, 138, 138, 139, 139, 139, 140, 141, 141, 142, 143, 143, 144, 144, 144, 144,
            144, 145, 145, 145, 145, 145, 145, 145, 145, 145, 145, 145, 145, 146, 147, 147, 148,
            149, 150, 151, 152, 153, 153, 153, 154, 154, 154, 155, 155, 155, 155, 155, 156, 157,
            157, 157, 158, 158, 159, 160, 160, 160, 160, 160, 160, 160, 160, 161, 162, 162, 162,
            163, 163, 163, 163, 163, 163, 163, 163, 164, 164, 165, 165, 165, 166, 167, 167, 168,
            168, 168, 169, 170, 170, 170, 170, 171, 171, 172, 172, 172, 173, 173, 173, 173, 173,
            173, 174, 174, 174, 174, 174, 174, 174, 175, 175, 175, 176, 177, 178, 178, 178, 179,
            179, 179, 179, 179, 179, 179, 179, 179, 180, 180, 180, 180, 180, 180, 181, 182, 182,
            183, 183, 184, 185, 186, 187, 187, 187, 187, 188, 188, 188, 188, 188, 188, 188, 188,
            189, 190, 191, 192, 192, 192, 193, 194, 195, 196, 197, 197, 198, 198, 199, 200, 200,
            201, 202, 202, 202, 203, 204, 205, 206, 207, 208, 209, 210, 211, 212, 213, 214, 215,
            216, 216, 216, 216, 216, 217, 218, 218, 218, 218, 218, 218, 218, 218, 218, 218, 218,
            218, 218, 218, 218, 219, 219, 219, 219, 219, 220, 221, 221, 221, 221, 222, 222, 222,
            223, 224, 225, 225, 226, 227, 227, 227, 227, 227, 227, 228, 229, 229, 229, 229, 229,
            230, 230, 230, 231, 231, 231, 231, 231, 231, 231, 231, 231, 231, 231, 231, 231, 232,
            232, 233, 233, 234, 234, 234, 235, 235, 235, 235, 236, 237, 237, 238, 238, 239, 240,
            240, 240, 240, 240, 240, 240, 240, 240, 241, 241, 241, 241, 242, 242, 242, 243, 243,
            243, 243, 243, 244, 244, 244, 244, 244, 244, 244, 244, 244, 244, 244, 244, 244, 245,
            246, 247, 248, 248, 249, 250, 251, 251, 252, 253, 253, 254, 254, 255, 256, 257, 258,
            259, 259, 260, 261, 262, 263, 264, 264, 264, 264, 264, 265, 265, 265, 266, 267, 267,
            267, 268, 269, 269, 270, 271, 271, 272, 273, 274, 274, 274, 274, 274, 274, 274, 274,
            274, 274, 275, 275, 276, 277, 278, 279, 280, 281, 282, 283, 283, 283, 283, 283, 283,
            283, 283, 283, 283, 284, 284, 285, 286, 286, 286, 287, 287, 288, 288, 289, 289, 289,
            289, 290, 290, 290, 291, 292, 292, 293, 293, 294, 295, 295, 296, 296, 296, 297, 297,
            298, 299, 300, 300, 300, 300, 301, 302, 302, 303, 304, 305, 306, 307, 308, 309, 310,
            311, 312, 313, 314, 314, 315, 316, 317, 318, 319, 319, 319, 320, 320, 320, 320, 320,
            320, 320, 320, 320, 320, 320, 321, 321, 321, 322, 322, 322, 322, 322, 322, 322, 322,
            322, 322, 322, 322, 322, 322, 322, 322, 323, 323, 323, 323, 323, 323, 323, 323, 323,
            323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323,
            323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323,
            323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 324, 325, 326,
            327, 327, 328, 329, 330, 331, 332, 332, 332, 333, 334, 334, 334, 335, 336, 337, 338,
            339, 340, 341, 342, 343, 343, 343, 344, 345, 345, 345, 346, 346, 347, 347, 347, 347,
            347, 348, 348, 348, 349, 349, 349, 350, 350, 351, 352, 353, 353, 353, 353, 353, 353,
            354, 355, 355, 355, 355, 355, 356, 357, 358, 359, 360, 361, 362, 363, 364, 364, 364,
            364, 365, 366, 366, 367, 368, 368, 369, 369, 370, 370, 371, 372, 372, 372, 373, 374,
            375, 376, 376, 376, 376, 376, 376, 376, 376, 377, 377, 378, 378, 378, 378, 378, 378,
            378, 379, 379, 379, 379, 379, 380, 380, 381, 382, 382, 382, 382, 382, 382, 383, 383,
            384, 384, 385, 385, 385, 386, 387, 387, 388, 388, 389, 390, 391, 392, 392, 392, 393,
            393, 393, 393, 393, 393, 394, 395, 396, 396, 397, 397, 397, 398, 399, 399, 399, 399,
            399, 399, 399, 400, 401, 401, 401, 401, 402, 403, 404, 405, 406, 407, 408, 408, 408,
            409, 410, 410, 410, 410, 410, 411, 411, 412, 412, 412, 412, 412, 412, 413, 414, 414,
            415, 415, 415, 416, 417, 417, 417, 418, 418, 418, 418, 418, 418, 419, 420, 420, 420,
            420, 420, 421, 421, 422, 423, 423, 423, 423, 423, 423, 424, 425, 426, 426, 426, 427,
            428, 428, 428, 429, 429, 429, 429, 429, 430, 431, 432, 432, 432, 433, 433, 433, 433,
            434, 434, 435, 435, 435, 435, 436, 437, 438, 438, 439, 439, 439, 440, 440, 440, 441,
            442, 442, 443, 444, 445, 446, 446, 447, 448, 449, 449, 450, 451, 452, 453, 454, 454,
            454, 455, 456, 457, 458, 458, 458, 458, 458, 459, 460, 460, 460, 460, 460, 461, 461,
            461, 461, 461, 461, 461, 461, 461, 461, 461, 461, 461, 461, 461, 461, 461, 461, 461,
            462, 462, 462, 462, 462, 462, 462, 462, 462, 462, 462, 463, 463, 463, 463, 463, 464,
            464, 464, 464, 464, 464, 464, 465, 465, 465, 466, 466, 466, 467, 468, 468, 469, 469,
            469, 470, 471, 472, 473, 473, 473, 473, 473, 473, 473, 474, 474, 474, 474, 474, 474,
            474, 474, 474, 474, 475, 476, 477, 478, 479, 480, 481, 482, 483, 484, 484, 484, 484,
            485, 485, 486, 486, 487, 488, 489, 490, 491, 492, 493, 493, 494, 495, 495, 495, 495,
            496, 497, 498, 498, 498, 498, 498, 498, 498, 499, 500, 500, 500, 500, 501, 502, 502,
            503, 504, 505, 506, 507, 507, 508, 509, 510, 510, 511, 512, 512, 512, 512, 512, 512,
            512, 512, 512, 512, 513, 513, 513, 513, 513, 514, 514, 515, 516, 517, 517, 518, 519,
            519, 520, 521, 521, 522, 523, 523, 524, 524, 524, 525, 526, 526, 527, 527, 527, 527,
            528, 529, 529, 529, 529, 530, 531, 531, 532, 533, 533, 533, 534, 534, 534, 534, 534,
            534, 535, 535, 535, 536, 536, 536, 537, 538, 538, 539, 540, 541, 541, 541, 542, 542,
            542, 543, 543, 544, 544, 545, 546, 546, 546, 546, 547, 547, 547, 547, 547, 547, 547,
            548, 549, 549, 549, 550, 551, 551, 551, 551, 551, 551, 551, 551, 551, 551, 551, 551,
            551, 552, 553, 553, 553, 553, 553, 554, 554, 554, 555, 555, 556, 557, 558, 558, 558,
            558, 558, 558, 558, 558, 558, 558, 558, 558, 558, 558, 558, 558, 558, 558, 558, 558,
            558, 559, 559, 559, 559, 559, 560, 560, 560, 560, 560, 560, 560, 561, 561, 562, 563,
            564, 565, 566, 567, 567, 567, 567, 568, 569, 570, 571, 572, 573, 574, 575, 575, 575,
            575, 576, 577, 578, 579, 580, 580, 581, 582, 583, 584, 584, 585, 586, 587, 588, 589,
            590, 591, 592, 593, 594, 595, 596, 597, 598, 599, 600, 601, 601, 602, 603, 604, 605,
            606, 607, 607, 608, 609, 609, 610, 610, 611, 611, 612, 613, 613, 613, 614, 614, 615,
            615, 616, 616, 616, 616, 616, 616, 617, 618, 619, 620, 621, 622, 623, 624, 625, 626,
            627, 628, 629, 630, 631, 632, 633, 634, 635, 636, 637, 638, 639, 640, 641, 642, 643,
            644, 645, 646, 647, 648, 649, 650, 651, 652, 653, 654, 655, 656, 657, 658, 659, 660,
            661, 662, 663, 664, 665, 666, 667, 668, 669, 670, 671, 672, 673, 674, 675, 676, 677,
            678, 679, 680, 681, 682, 683, 684, 685, 686, 687, 688, 689, 690, 691, 692, 693, 694,
            695, 696, 697, 698, 699, 700, 701, 702, 703, 704, 705, 706, 707, 708, 709, 710, 711,
            712, 713, 714, 715, 716, 717, 718, 719, 720, 721, 722, 723, 724, 725, 726, 727, 728,
            729, 730, 731, 732, 732, 732, 732, 732, 732, 732, 733, 734, 735, 735, 735, 736, 737,
            738, 739, 740, 741, 742, 743, 744, 745, 746, 747, 748, 749, 750, 751, 752, 753, 754,
            755, 756, 757, 758, 759, 760, 761, 762, 763, 764, 765, 766, 767, 768, 769, 770, 771,
            772, 773, 774, 775, 776, 777, 778, 779, 780, 781, 782, 783, 784, 784, 784, 784, 784,
            784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784,
            784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784,
            784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784,
            784, 784, 784, 784, 784, 784, 784, 784, 784, 785, 785, 785, 785, 785, 785, 785, 785,
            785, 786, 787, 788, 789, 790, 791, 791, 792, 792, 792, 792, 792, 792, 792, 792, 792,
            792, 792, 792, 792, 792, 792, 792, 793, 793, 793, 793, 794, 795, 796, 797, 797, 798,
            798, 798, 798, 799, 800, 801, 801, 802, 802, 803, 803, 804, 804, 805, 805, 806, 807,
            808, 809, 810, 810, 811, 812, 813, 814, 815, 816, 817, 818, 819, 820, 821, 821, 822,
            823, 824, 825, 825, 825, 825, 825, 825, 826, 827, 828, 828, 829, 830, 830, 830, 830,
            830, 830, 830, 830, 830, 830, 830, 830, 830, 830, 830, 830, 830, 830, 830, 830, 830,
            831, 831, 831, 831, 831, 831, 831, 831, 831, 831, 831, 831, 831, 831, 831, 831, 831,
            831, 831, 831, 831, 831, 831, 831, 831, 831, 831, 831, 831, 831, 831, 831, 831, 831,
            831, 832, 833, 833, 834, 835, 835, 836, 836, 837, 838, 839, 840, 841, 841, 841, 841,
            842, 843, 843, 843, 843, 844, 845, 845, 845, 845, 845, 845, 845, 845, 845, 846, 847,
            848, 849, 850, 851, 852, 853, 854, 855, 856, 857, 858, 859, 860, 861, 862, 863, 864,
            865, 866, 867, 867, 867, 868, 869, 870, 871, 872, 873, 874, 875, 876, 877, 878, 879,
            880, 880, 880, 880, 881, 882, 883, 884, 885, 886, 887, 888, 889, 890, 891, 892, 892,
            892, 892, 892, 893, 894, 895, 895, 896, 897, 898, 899, 899, 900, 900, 901, 901, 901,
            902, 902,
        ],
        selects: &[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 13, 14, 15, 18, 19, 25, 26, 28, 29, 30, 31, 34, 35,
            36, 37, 38, 39, 40, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 55, 57, 59, 60, 65,
            66, 67, 70, 71, 73, 74, 75, 76, 80, 88, 89, 91, 93, 94, 97, 98, 101, 115, 119, 120,
            121, 123, 132, 141, 143, 153, 157, 158, 161, 170, 174, 176, 177, 180, 181, 183, 187,
            188, 193, 194, 200, 201, 202, 204, 205, 207, 208, 209, 210, 211, 212, 213, 214, 216,
            217, 220, 221, 222, 225, 229, 230, 231, 232, 233, 235, 236, 237, 238, 240, 241, 242,
            245, 247, 249, 251, 252, 253, 254, 255, 256, 257, 258, 260, 267, 268, 269, 271, 272,
            275, 278, 280, 281, 282, 283, 285, 286, 287, 288, 289, 290, 291, 292, 293, 294, 295,
            298, 305, 306, 308, 309, 311, 312, 313, 314, 317, 318, 320, 323, 324, 325, 326, 327,
            328, 329, 332, 333, 335, 336, 337, 338, 339, 340, 341, 343, 345, 346, 349, 351, 352,
            355, 356, 357, 359, 361, 362, 364, 365, 366, 367, 368, 370, 371, 372, 373, 374, 375,
            377, 378, 382, 383, 385, 386, 387, 388, 389, 390, 391, 392, 394, 395, 396, 397, 398,
            401, 403, 408, 409, 410, 412, 413, 414, 415, 416, 417, 418, 423, 424, 430, 432, 435,
            438, 439, 454, 455, 456, 457, 460, 461, 462, 463, 464, 465, 466, 467, 468, 469, 470,
            471, 472, 473, 475, 476, 477, 478, 481, 482, 483, 485, 486, 490, 493, 494, 495, 496,
            497, 500, 501, 502, 503, 505, 506, 508, 509, 510, 511, 512, 513, 514, 515, 516, 517,
            518, 519, 521, 523, 525, 526, 528, 529, 530, 533, 535, 538, 539, 540, 541, 542, 543,
            544, 545, 547, 548, 549, 551, 552, 554, 555, 556, 557, 559, 560, 561, 562, 563, 564,
            565, 566, 567, 568, 569, 570, 575, 579, 582, 584, 590, 596, 597, 598, 599, 601, 602,
            605, 606, 609, 612, 616, 617, 618, 619, 620, 621, 622, 623, 624, 626, 635, 636, 637,
            638, 639, 640, 641, 642, 643, 645, 648, 649, 651, 653, 655, 656, 657, 659, 660, 663,
            665, 668, 670, 671, 673, 677, 678, 679, 682, 695, 701, 702, 704, 705, 706, 707, 708,
            709, 710, 711, 712, 713, 715, 716, 718, 719, 720, 721, 722, 723, 724, 725, 726, 727,
            728, 729, 730, 731, 732, 734, 735, 736, 737, 738, 739, 740, 741, 742, 743, 744, 745,
            746, 747, 748, 749, 750, 751, 752, 753, 754, 755, 756, 757, 758, 759, 760, 761, 762,
            763, 764, 765, 766, 767, 768, 769, 770, 771, 772, 773, 774, 775, 776, 777, 778, 779,
            780, 781, 782, 783, 784, 785, 786, 787, 788, 789, 794, 800, 801, 804, 805, 815, 816,
            819, 820, 822, 824, 825, 826, 827, 829, 830, 832, 833, 835, 839, 840, 841, 842, 843,
            846, 847, 848, 849, 859, 860, 861, 864, 867, 869, 871, 874, 875, 880, 881, 882, 883,
            884, 885, 886, 888, 890, 891, 892, 893, 894, 895, 897, 898, 899, 900, 902, 905, 906,
            907, 908, 909, 911, 913, 915, 916, 919, 921, 926, 927, 929, 930, 931, 932, 933, 937,
            939, 940, 943, 944, 945, 946, 947, 948, 951, 952, 953, 961, 962, 965, 966, 967, 968,
            970, 972, 973, 974, 975, 976, 979, 981, 982, 985, 986, 988, 989, 990, 991, 992, 995,
            996, 997, 998, 1000, 1003, 1004, 1005, 1006, 1007, 1011, 1012, 1015, 1016, 1018, 1019,
            1020, 1021, 1025, 1026, 1028, 1029, 1030, 1032, 1034, 1035, 1036, 1040, 1042, 1043,
            1045, 1046, 1049, 1054, 1058, 1064, 1065, 1070, 1071, 1072, 1073, 1076, 1077, 1078,
            1079, 1081, 1082, 1083, 1084, 1085, 1086, 1087, 1088, 1089, 1090, 1091, 1092, 1093,
            1094, 1095, 1096, 1097, 1098, 1100, 1101, 1102, 1103, 1104, 1105, 1106, 1107, 1108,
            1109, 1111, 1112, 1113, 1114, 1116, 1117, 1118, 1119, 1120, 1121, 1123, 1124, 1126,
            1127, 1130, 1132, 1133, 1138, 1139, 1140, 1141, 1142, 1143, 1145, 1146, 1147, 1148,
            1149, 1150, 1151, 1152, 1153, 1164, 1165, 1166, 1168, 1170, 1178, 1181, 1182, 1183,
            1187, 1188, 1189, 1190, 1191, 1192, 1195, 1196, 1197, 1200, 1206, 1210, 1213, 1214,
            1215, 1216, 1217, 1218, 1219, 1220, 1221, 1223, 1224, 1225, 1226, 1228, 1232, 1235,
            1238, 1241, 1243, 1244, 1247, 1249, 1250, 1251, 1254, 1255, 1256, 1259, 1262, 1263,
            1265, 1266, 1267, 1268, 1269, 1271, 1272, 1274, 1275, 1278, 1282, 1283, 1285, 1286,
            1288, 1290, 1293, 1294, 1295, 1297, 1298, 1299, 1300, 1301, 1302, 1305, 1306, 1309,
            1310, 1311, 1312, 1313, 1314, 1315, 1316, 1317, 1318, 1319, 1320, 1323, 1324, 1325,
            1326, 1328, 1329, 1331, 1335, 1336, 1337, 1338, 1339, 1340, 1341, 1342, 1343, 1344,
            1345, 1346, 1347, 1348, 1349, 1350, 1351, 1352, 1353, 1354, 1356, 1357, 1358, 1359,
            1361, 1362, 1363, 1364, 1365, 1366, 1368, 1375, 1376, 1377, 1386, 1387, 1388, 1394,
            1399, 1417, 1424, 1427, 1429, 1431, 1434, 1435, 1437, 1439, 1441, 1442, 1443, 1444,
            1445, 1562, 1563, 1564, 1565, 1566, 1567, 1571, 1572, 1622, 1623, 1624, 1625, 1626,
            1627, 1628, 1629, 1630, 1631, 1632, 1633, 1634, 1635, 1636, 1637, 1638, 1639, 1640,
            1641, 1642, 1643, 1644, 1645, 1646, 1647, 1648, 1649, 1650, 1651, 1652, 1653, 1654,
            1655, 1656, 1657, 1658, 1659, 1660, 1661, 1662, 1663, 1664, 1665, 1666, 1667, 1668,
            1669, 1670, 1671, 1672, 1673, 1674, 1675, 1676, 1677, 1678, 1679, 1680, 1681, 1682,
            1683, 1684, 1685, 1687, 1688, 1689, 1690, 1691, 1692, 1693, 1694, 1701, 1703, 1704,
            1705, 1706, 1707, 1708, 1709, 1710, 1711, 1712, 1713, 1714, 1715, 1716, 1717, 1719,
            1720, 1721, 1726, 1728, 1729, 1730, 1734, 1736, 1738, 1740, 1742, 1748, 1760, 1765,
            1766, 1767, 1768, 1769, 1773, 1776, 1777, 1778, 1779, 1780, 1781, 1782, 1783, 1784,
            1785, 1786, 1787, 1788, 1789, 1790, 1791, 1792, 1793, 1794, 1795, 1797, 1798, 1799,
            1800, 1801, 1802, 1803, 1804, 1805, 1806, 1807, 1808, 1809, 1810, 1811, 1812, 1813,
            1814, 1815, 1816, 1817, 1818, 1819, 1820, 1821, 1822, 1823, 1824, 1825, 1826, 1827,
            1828, 1829, 1830, 1833, 1836, 1838, 1844, 1845, 1846, 1849, 1850, 1851, 1854, 1855,
            1856, 1857, 1858, 1859, 1860, 1861, 1884, 1885, 1899, 1900, 1901, 1914, 1915, 1916,
            1917, 1921, 1926, 1928, 1930, 1931, 1933,
        ],
        selects0: &[
            0, 10, 12, 16, 17, 20, 21, 22, 23, 24, 27, 32, 33, 41, 54, 56, 58, 61, 62, 63, 64, 68,
            69, 72, 77, 78, 79, 81, 82, 83, 84, 85, 86, 87, 90, 92, 95, 96, 99, 100, 102, 103, 104,
            105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 116, 117, 118, 122, 124, 125, 126,
            127, 128, 129, 130, 131, 133, 134, 135, 136, 137, 138, 139, 140, 142, 144, 145, 146,
            147, 148, 149, 150, 151, 152, 154, 155, 156, 159, 160, 162, 163, 164, 165, 166, 167,
            168, 169, 171, 172, 173, 175, 178, 179, 182, 184, 185, 186, 189, 190, 191, 192, 195,
            196, 197, 198, 199, 203, 206, 215, 218, 219, 223, 224, 226, 227, 228, 234, 239, 243,
            244, 246, 248, 250, 259, 261, 262, 263, 264, 265, 266, 270, 273, 274, 276, 277, 279,
            284, 296, 297, 299, 300, 301, 302, 303, 304, 307, 310, 315, 316, 319, 321, 322, 330,
            331, 334, 342, 344, 347, 348, 350, 353, 354, 358, 360, 363, 369, 376, 379, 380, 381,
            384, 393, 399, 400, 402, 404, 405, 406, 407, 411, 419, 420, 421, 422, 425, 426, 427,
            428, 429, 431, 433, 434, 436, 437, 440, 441, 442, 443, 444, 445, 446, 447, 448, 449,
            450, 451, 452, 453, 458, 459, 474, 479, 480, 484, 487, 488, 489, 491, 492, 498, 499,
            504, 507, 520, 522, 524, 527, 531, 532, 534, 536, 537, 546, 550, 553, 558, 571, 572,
            573, 574, 576, 577, 578, 580, 581, 583, 585, 586, 587, 588, 589, 591, 592, 593, 594,
            595, 600, 603, 604, 607, 608, 610, 611, 613, 614, 615, 625, 627, 628, 629, 630, 631,
            632, 633, 634, 644, 646, 647, 650, 652, 654, 658, 661, 662, 664, 666, 667, 669, 672,
            674, 675, 676, 680, 681, 683, 684, 685, 686, 687, 688, 689, 690, 691, 692, 693, 694,
            696, 697, 698, 699, 700, 703, 714, 717, 733, 790, 791, 792, 793, 795, 796, 797, 798,
            799, 802, 803, 806, 807, 808, 809, 810, 811, 812, 813, 814, 817, 818, 821, 823, 828,
            831, 834, 836, 837, 838, 844, 845, 850, 851, 852, 853, 854, 855, 856, 857, 858, 862,
            863, 865, 866, 868, 870, 872, 873, 876, 877, 878, 879, 887, 889, 896, 901, 903, 904,
            910, 912, 914, 917, 918, 920, 922, 923, 924, 925, 928, 934, 935, 936, 938, 941, 942,
            949, 950, 954, 955, 956, 957, 958, 959, 960, 963, 964, 969, 971, 977, 978, 980, 983,
            984, 987, 993, 994, 999, 1001, 1002, 1008, 1009, 1010, 1013, 1014, 1017, 1022, 1023,
            1024, 1027, 1031, 1033, 1037, 1038, 1039, 1041, 1044, 1047, 1048, 1050, 1051, 1052,
            1053, 1055, 1056, 1057, 1059, 1060, 1061, 1062, 1063, 1066, 1067, 1068, 1069, 1074,
            1075, 1080, 1099, 1110, 1115, 1122, 1125, 1128, 1129, 1131, 1134, 1135, 1136, 1137,
            1144, 1154, 1155, 1156, 1157, 1158, 1159, 1160, 1161, 1162, 1163, 1167, 1169, 1171,
            1172, 1173, 1174, 1175, 1176, 1177, 1179, 1180, 1184, 1185, 1186, 1193, 1194, 1198,
            1199, 1201, 1202, 1203, 1204, 1205, 1207, 1208, 1209, 1211, 1212, 1222, 1227, 1229,
            1230, 1231, 1233, 1234, 1236, 1237, 1239, 1240, 1242, 1245, 1246, 1248, 1252, 1253,
            1257, 1258, 1260, 1261, 1264, 1270, 1273, 1276, 1277, 1279, 1280, 1281, 1284, 1287,
            1289, 1291, 1292, 1296, 1303, 1304, 1307, 1308, 1321, 1322, 1327, 1330, 1332, 1333,
            1334, 1355, 1360, 1367, 1369, 1370, 1371, 1372, 1373, 1374, 1378, 1379, 1380, 1381,
            1382, 1383, 1384, 1385, 1389, 1390, 1391, 1392, 1393, 1395, 1396, 1397, 1398, 1400,
            1401, 1402, 1403, 1404, 1405, 1406, 1407, 1408, 1409, 1410, 1411, 1412, 1413, 1414,
            1415, 1416, 1418, 1419, 1420, 1421, 1422, 1423, 1425, 1426, 1428, 1430, 1432, 1433,
            1436, 1438, 1440, 1446, 1447, 1448, 1449, 1450, 1451, 1452, 1453, 1454, 1455, 1456,
            1457, 1458, 1459, 1460, 1461, 1462, 1463, 1464, 1465, 1466, 1467, 1468, 1469, 1470,
            1471, 1472, 1473, 1474, 1475, 1476, 1477, 1478, 1479, 1480, 1481, 1482, 1483, 1484,
            1485, 1486, 1487, 1488, 1489, 1490, 1491, 1492, 1493, 1494, 1495, 1496, 1497, 1498,
            1499, 1500, 1501, 1502, 1503, 1504, 1505, 1506, 1507, 1508, 1509, 1510, 1511, 1512,
            1513, 1514, 1515, 1516, 1517, 1518, 1519, 1520, 1521, 1522, 1523, 1524, 1525, 1526,
            1527, 1528, 1529, 1530, 1531, 1532, 1533, 1534, 1535, 1536, 1537, 1538, 1539, 1540,
            1541, 1542, 1543, 1544, 1545, 1546, 1547, 1548, 1549, 1550, 1551, 1552, 1553, 1554,
            1555, 1556, 1557, 1558, 1559, 1560, 1561, 1568, 1569, 1570, 1573, 1574, 1575, 1576,
            1577, 1578, 1579, 1580, 1581, 1582, 1583, 1584, 1585, 1586, 1587, 1588, 1589, 1590,
            1591, 1592, 1593, 1594, 1595, 1596, 1597, 1598, 1599, 1600, 1601, 1602, 1603, 1604,
            1605, 1606, 1607, 1608, 1609, 1610, 1611, 1612, 1613, 1614, 1615, 1616, 1617, 1618,
            1619, 1620, 1621, 1686, 1695, 1696, 1697, 1698, 1699, 1700, 1702, 1718, 1722, 1723,
            1724, 1725, 1727, 1731, 1732, 1733, 1735, 1737, 1739, 1741, 1743, 1744, 1745, 1746,
            1747, 1749, 1750, 1751, 1752, 1753, 1754, 1755, 1756, 1757, 1758, 1759, 1761, 1762,
            1763, 1764, 1770, 1771, 1772, 1774, 1775, 1796, 1831, 1832, 1834, 1835, 1837, 1839,
            1840, 1841, 1842, 1843, 1847, 1848, 1852, 1853, 1862, 1863, 1864, 1865, 1866, 1867,
            1868, 1869, 1870, 1871, 1872, 1873, 1874, 1875, 1876, 1877, 1878, 1879, 1880, 1881,
            1882, 1883, 1886, 1887, 1888, 1889, 1890, 1891, 1892, 1893, 1894, 1895, 1896, 1897,
            1898, 1902, 1903, 1904, 1905, 1906, 1907, 1908, 1909, 1910, 1911, 1912, 1913, 1918,
            1919, 1920, 1922, 1923, 1924, 1925, 1927, 1929, 1932,
        ],
    };

    const BAD_TREE_NODE_2: &TestCase = &TestCase {
        bits: &[
            true, true, true, true, true, true, true, true, true, false, true, false, true, true,
            true, false, false, true, true, false, false, false, false, false, true, true, false,
            true, true, true, true, false, false, true, true, true, true, true, true, true, false,
            true, true, true, true, true, true, true, true, true, true, true, true, false, true,
            false, true, false, true, true, false, false, false, false, true, true, true, false,
            false, true, true, false, true, true, true, true, false, false, false, true, false,
            false, false, false, false, false, false, true, true, false, true, false, true, true,
            false, false, true, true, false, false, true, false, false, false, false, false, false,
            false, false, false, false, false, false, false, true, false, false, false, true, true,
            true, false, true, false, false, false, false, false, false, false, false, true, false,
            false, false, false, false, false, false, false, true, false, true, false, false,
            false, false, false, false, false, false, false, true, false, false, false, true, true,
            false, false, true, false, false, false, false, false, false, false, false, true,
            false, false, false, true, false, true, true, false, false, true, true, false, true,
            false, false, false, true, true, false, false, false, false, true, true, false, false,
            false, false, false, true, true, true, false, true, true, false, true, true, true,
            true, true, true, true, true, false, true, true, false, false, true, true, true, false,
            false, true, false, false, false, true, true, true, true, true, false, true, true,
            true, true, false, true, true, true, false, false, true, false, true, false, true,
            false, true, true, true, true, true, true, true, true, false, true, false, false,
            false, false, false, false, true, true, true, false, true, true, false, false, true,
            false, false, true, false, true, true, true, true, false, true, true, true, true, true,
            true, true, true, true, true, true, false, false, true, false, false, false, false,
            false, false, true, true, false, true, true, false, true, true, true, true, false,
            false, true, true, false, true, false, false, true, true, true, true, true, true, true,
            false, false, true, true, false, true, true, true, true, true, true, true, false, true,
            false, true, true, false, false, true, false, true, true, false, false, true, true,
            true, false, true, false, true, true, false, true, true, true, true, true, false, true,
            true, true, true, true, true, false, true, true, false, false, false, true, true,
            false, true, true, true, true, true, true, true, true, false, true, true, true, true,
            true, false, false, true, false, true, false, false, false, false, true, true, true,
            false, true, true, true, true, true, true, true, false, false, false, false, true,
            true, false, false, false, false, false, true, false, true, false, false, true, false,
            false, true, true, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, true, true, true, true, false, false, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, false, true,
            true, true, true, false, false, true, true, true, false, true, true, false, false,
            false, true, false, false, true, true, true, true, true, false, false, true, true,
            true, true, false, true, true, false, true, true, true, true, true, true, true, true,
            true, true, true, true, false, true, false, true, false, true, true, false, true, true,
            true, false, false, true, false, true, false, false, true, true, true, true, true,
            true, true, true, false, true, true, true, false, true, true, false, true, true, true,
            true, false, true, true, true, true, true, true, true, true, true, true, true, true,
            false, false, false, false, true, false, false, false, true, false, false, true, false,
            true, false, false, false, false, false, true, false, false, false, false, false, true,
            true, true, true, false, true, true, false, false, true, true, false, false, true,
            false, false, true, false, false, false, true, true, true, true, true, true, true,
            true, true, false, true, false, false, false, false, false, false, false, false, true,
            true, true, true, true, true, true, true, true, false, true, false, false, true, true,
            false, true, false, true, false, true, true, true, false, true, true, false, false,
            true, false, true, false, false, true, false, true, true, false, true, false, false,
            false, true, true, true, false, false, true, false, false, false, false, false, false,
            false, false, false, false, false, false, true, false, false, false, false, false,
            true, true, false, true, true, true, true, true, true, true, true, true, true, false,
            true, true, false, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, false, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, false, false, false, false, true, false, false, false,
            false, false, true, true, false, false, true, true, false, false, false, false, false,
            false, false, false, false, true, true, false, false, true, true, false, true, false,
            true, true, true, true, false, true, true, false, true, true, false, true, false,
            false, false, true, true, true, true, true, false, false, true, true, true, true,
            false, false, false, false, false, false, false, false, false, true, true, true, false,
            false, true, false, false, true, false, true, false, true, false, false, true, true,
            false, false, false, false, true, true, true, true, true, true, true, false, true,
            false, true, true, true, true, true, true, false, true, true, true, true, false, true,
            false, false, true, true, true, true, true, false, true, false, true, false, true,
            true, false, false, true, false, true, false, false, false, false, true, true, false,
            true, true, true, true, true, false, false, false, true, false, true, true, false,
            false, true, true, true, true, true, true, false, false, true, true, true, false,
            false, false, false, false, false, false, true, true, false, false, true, true, true,
            true, false, true, false, true, true, true, true, true, false, false, true, false,
            true, true, false, false, true, true, false, true, true, true, true, true, false,
            false, true, true, true, true, false, true, false, false, true, true, true, true, true,
            false, false, false, true, true, false, false, true, true, false, true, true, true,
            true, false, false, false, true, true, false, true, true, true, false, true, false,
            true, true, true, false, false, false, true, false, true, true, false, true, true,
            false, false, true, false, false, false, false, true, false, false, false, true, false,
            false, false, false, false, true, true, false, false, false, false, true, true, true,
            true, false, false, true, true, true, true, false, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, false, true,
            true, true, true, true, true, true, true, true, true, false, true, true, true, true,
            false, true, true, true, true, true, true, false, true, true, false, true, true, false,
            false, true, false, true, true, false, false, false, false, true, true, true, true,
            true, true, false, true, true, true, true, true, true, true, true, true, false, false,
            false, false, false, false, false, false, false, false, true, true, true, false, true,
            false, true, false, false, false, false, false, false, false, true, false, false, true,
            true, true, false, false, false, true, true, true, true, true, true, false, false,
            true, true, true, false, false, true, false, false, false, false, false, true, false,
            false, false, true, false, false, true, true, true, true, true, true, true, true, true,
            false, true, true, true, true, false, true, false, false, false, true, false, false,
            true, false, false, true, false, false, true, false, true, true, false, false, true,
            false, true, true, true, false, false, true, true, true, false, false, true, false,
            false, true, true, false, true, true, true, true, true, false, true, true, false, true,
            true, false, false, true, false, false, false, true, true, false, true, true, false,
            true, false, true, false, false, true, true, true, false, true, true, true, true, true,
            true, false, false, true, true, false, false, true, true, true, true, true, true, true,
            true, true, true, true, true, false, false, true, true, true, true, false, true, true,
            false, true, false, false, false, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, false, true, true,
            true, true, false, true, true, true, true, true, true, false, true, false, false,
            false, false, false, false, true, true, true, false, false, false, false, false, false,
            false, false, true, true, true, false, false, false, false, false, true, false, false,
            false, false, true, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, true, false, false, false,
            false, false, false, true, false, false, true, false, true, false, true, false, false,
            true, true, false, true, false, true, false, true, true, true, true, true, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, true, true, true, true, true, true,
            false, false, false, true, true, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, false, true, true, true, true, true, true, true, true, false, false, false,
            false, false, false, true, false, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, false, true, true, true, false, false, false,
            false, true, false, true, true, true, false, false, false, true, false, true, false,
            true, false, true, false, true, false, false, false, false, false, true, false, false,
            false, false, false, false, false, false, false, false, false, true, false, false,
            false, false, true, true, true, true, true, false, false, false, true, false, false,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, false, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, false,
            false, true, false, false, true, false, true, false, false, false, false, false, true,
            true, true, false, false, true, true, true, false, false, true, true, true, true, true,
            true, true, true, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            true, true, false, false, false, false, false, false, false, false, false, false,
            false, false, false, true, true, true, false, false, false, false, false, false, false,
            false, false, false, false, false, true, true, true, true, false, false, false, true,
            false, false, false, false, true, false, true, false, true, true, false, true,
        ],
        ranks: &[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 9, 10, 10, 11, 12, 13, 13, 13, 14, 15, 15, 15, 15, 15,
            15, 16, 17, 17, 18, 19, 20, 21, 21, 21, 22, 23, 24, 25, 26, 27, 28, 28, 29, 30, 31, 32,
            33, 34, 35, 36, 37, 38, 39, 40, 40, 41, 41, 42, 42, 43, 44, 44, 44, 44, 44, 45, 46, 47,
            47, 47, 48, 49, 49, 50, 51, 52, 53, 53, 53, 53, 54, 54, 54, 54, 54, 54, 54, 54, 55, 56,
            56, 57, 57, 58, 59, 59, 59, 60, 61, 61, 61, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62,
            62, 62, 62, 63, 63, 63, 63, 64, 65, 66, 66, 67, 67, 67, 67, 67, 67, 67, 67, 67, 68, 68,
            68, 68, 68, 68, 68, 68, 68, 69, 69, 70, 70, 70, 70, 70, 70, 70, 70, 70, 70, 71, 71, 71,
            71, 72, 73, 73, 73, 74, 74, 74, 74, 74, 74, 74, 74, 74, 75, 75, 75, 75, 76, 76, 77, 78,
            78, 78, 79, 80, 80, 81, 81, 81, 81, 82, 83, 83, 83, 83, 83, 84, 85, 85, 85, 85, 85, 85,
            86, 87, 88, 88, 89, 90, 90, 91, 92, 93, 94, 95, 96, 97, 98, 98, 99, 100, 100, 100, 101,
            102, 103, 103, 103, 104, 104, 104, 104, 105, 106, 107, 108, 109, 109, 110, 111, 112,
            113, 113, 114, 115, 116, 116, 116, 117, 117, 118, 118, 119, 119, 120, 121, 122, 123,
            124, 125, 126, 127, 127, 128, 128, 128, 128, 128, 128, 128, 129, 130, 131, 131, 132,
            133, 133, 133, 134, 134, 134, 135, 135, 136, 137, 138, 139, 139, 140, 141, 142, 143,
            144, 145, 146, 147, 148, 149, 150, 150, 150, 151, 151, 151, 151, 151, 151, 151, 152,
            153, 153, 154, 155, 155, 156, 157, 158, 159, 159, 159, 160, 161, 161, 162, 162, 162,
            163, 164, 165, 166, 167, 168, 169, 169, 169, 170, 171, 171, 172, 173, 174, 175, 176,
            177, 178, 178, 179, 179, 180, 181, 181, 181, 182, 182, 183, 184, 184, 184, 185, 186,
            187, 187, 188, 188, 189, 190, 190, 191, 192, 193, 194, 195, 195, 196, 197, 198, 199,
            200, 201, 201, 202, 203, 203, 203, 203, 204, 205, 205, 206, 207, 208, 209, 210, 211,
            212, 213, 213, 214, 215, 216, 217, 218, 218, 218, 219, 219, 220, 220, 220, 220, 220,
            221, 222, 223, 223, 224, 225, 226, 227, 228, 229, 230, 230, 230, 230, 230, 231, 232,
            232, 232, 232, 232, 232, 233, 233, 234, 234, 234, 235, 235, 235, 236, 237, 237, 237,
            237, 237, 237, 237, 237, 237, 237, 237, 237, 237, 237, 237, 238, 239, 240, 241, 241,
            241, 242, 243, 244, 245, 246, 247, 248, 249, 250, 251, 252, 253, 254, 255, 255, 256,
            257, 258, 259, 259, 259, 260, 261, 262, 262, 263, 264, 264, 264, 264, 265, 265, 265,
            266, 267, 268, 269, 270, 270, 270, 271, 272, 273, 274, 274, 275, 276, 276, 277, 278,
            279, 280, 281, 282, 283, 284, 285, 286, 287, 288, 288, 289, 289, 290, 290, 291, 292,
            292, 293, 294, 295, 295, 295, 296, 296, 297, 297, 297, 298, 299, 300, 301, 302, 303,
            304, 305, 305, 306, 307, 308, 308, 309, 310, 310, 311, 312, 313, 314, 314, 315, 316,
            317, 318, 319, 320, 321, 322, 323, 324, 325, 326, 326, 326, 326, 326, 327, 327, 327,
            327, 328, 328, 328, 329, 329, 330, 330, 330, 330, 330, 330, 331, 331, 331, 331, 331,
            331, 332, 333, 334, 335, 335, 336, 337, 337, 337, 338, 339, 339, 339, 340, 340, 340,
            341, 341, 341, 341, 342, 343, 344, 345, 346, 347, 348, 349, 350, 350, 351, 351, 351,
            351, 351, 351, 351, 351, 351, 352, 353, 354, 355, 356, 357, 358, 359, 360, 360, 361,
            361, 361, 362, 363, 363, 364, 364, 365, 365, 366, 367, 368, 368, 369, 370, 370, 370,
            371, 371, 372, 372, 372, 373, 373, 374, 375, 375, 376, 376, 376, 376, 377, 378, 379,
            379, 379, 380, 380, 380, 380, 380, 380, 380, 380, 380, 380, 380, 380, 380, 381, 381,
            381, 381, 381, 381, 382, 383, 383, 384, 385, 386, 387, 388, 389, 390, 391, 392, 393,
            393, 394, 395, 395, 396, 397, 398, 399, 400, 401, 402, 403, 404, 405, 406, 407, 408,
            409, 410, 410, 411, 412, 413, 414, 415, 416, 417, 418, 419, 420, 421, 422, 423, 424,
            425, 426, 427, 428, 429, 430, 431, 432, 433, 434, 435, 436, 437, 438, 439, 440, 441,
            442, 443, 444, 445, 446, 447, 448, 449, 450, 451, 452, 453, 454, 455, 456, 457, 458,
            459, 460, 461, 462, 463, 464, 465, 466, 466, 466, 466, 466, 467, 467, 467, 467, 467,
            467, 468, 469, 469, 469, 470, 471, 471, 471, 471, 471, 471, 471, 471, 471, 471, 472,
            473, 473, 473, 474, 475, 475, 476, 476, 477, 478, 479, 480, 480, 481, 482, 482, 483,
            484, 484, 485, 485, 485, 485, 486, 487, 488, 489, 490, 490, 490, 491, 492, 493, 494,
            494, 494, 494, 494, 494, 494, 494, 494, 494, 495, 496, 497, 497, 497, 498, 498, 498,
            499, 499, 500, 500, 501, 501, 501, 502, 503, 503, 503, 503, 503, 504, 505, 506, 507,
            508, 509, 510, 510, 511, 511, 512, 513, 514, 515, 516, 517, 517, 518, 519, 520, 521,
            521, 522, 522, 522, 523, 524, 525, 526, 527, 527, 528, 528, 529, 529, 530, 531, 531,
            531, 532, 532, 533, 533, 533, 533, 533, 534, 535, 535, 536, 537, 538, 539, 540, 540,
            540, 540, 541, 541, 542, 543, 543, 543, 544, 545, 546, 547, 548, 549, 549, 549, 550,
            551, 552, 552, 552, 552, 552, 552, 552, 552, 553, 554, 554, 554, 555, 556, 557, 558,
            558, 559, 559, 560, 561, 562, 563, 564, 564, 564, 565, 565, 566, 567, 567, 567, 568,
            569, 569, 570, 571, 572, 573, 574, 574, 574, 575, 576, 577, 578, 578, 579, 579, 579,
            580, 581, 582, 583, 584, 584, 584, 584, 585, 586, 586, 586, 587, 588, 588, 589, 590,
            591, 592, 592, 592, 592, 593, 594, 594, 595, 596, 597, 597, 598, 598, 599, 600, 601,
            601, 601, 601, 602, 602, 603, 604, 604, 605, 606, 606, 606, 607, 607, 607, 607, 607,
            608, 608, 608, 608, 609, 609, 609, 609, 609, 609, 610, 611, 611, 611, 611, 611, 612,
            613, 614, 615, 615, 615, 616, 617, 618, 619, 619, 620, 621, 622, 623, 624, 625, 626,
            627, 628, 629, 630, 631, 632, 633, 634, 635, 636, 637, 637, 638, 639, 640, 641, 642,
            643, 644, 645, 646, 647, 647, 648, 649, 650, 651, 651, 652, 653, 654, 655, 656, 657,
            657, 658, 659, 659, 660, 661, 661, 661, 662, 662, 663, 664, 664, 664, 664, 664, 665,
            666, 667, 668, 669, 670, 670, 671, 672, 673, 674, 675, 676, 677, 678, 679, 679, 679,
            679, 679, 679, 679, 679, 679, 679, 679, 680, 681, 682, 682, 683, 683, 684, 684, 684,
            684, 684, 684, 684, 684, 685, 685, 685, 686, 687, 688, 688, 688, 688, 689, 690, 691,
            692, 693, 694, 694, 694, 695, 696, 697, 697, 697, 698, 698, 698, 698, 698, 698, 699,
            699, 699, 699, 700, 700, 700, 701, 702, 703, 704, 705, 706, 707, 708, 709, 709, 710,
            711, 712, 713, 713, 714, 714, 714, 714, 715, 715, 715, 716, 716, 716, 717, 717, 717,
            718, 718, 719, 720, 720, 720, 721, 721, 722, 723, 724, 724, 724, 725, 726, 727, 727,
            727, 728, 728, 728, 729, 730, 730, 731, 732, 733, 734, 735, 735, 736, 737, 737, 738,
            739, 739, 739, 740, 740, 740, 740, 741, 742, 742, 743, 744, 744, 745, 745, 746, 746,
            746, 747, 748, 749, 749, 750, 751, 752, 753, 754, 755, 755, 755, 756, 757, 757, 757,
            758, 759, 760, 761, 762, 763, 764, 765, 766, 767, 768, 769, 769, 769, 770, 771, 772,
            773, 773, 774, 775, 775, 776, 776, 776, 776, 777, 778, 779, 780, 781, 782, 783, 784,
            785, 786, 787, 788, 789, 790, 791, 792, 793, 794, 795, 796, 796, 797, 798, 799, 800,
            800, 801, 802, 803, 804, 805, 806, 806, 807, 807, 807, 807, 807, 807, 807, 808, 809,
            810, 810, 810, 810, 810, 810, 810, 810, 810, 811, 812, 813, 813, 813, 813, 813, 813,
            814, 814, 814, 814, 814, 815, 815, 815, 815, 815, 815, 815, 815, 815, 815, 815, 815,
            815, 815, 815, 815, 815, 815, 816, 816, 816, 816, 816, 816, 816, 817, 817, 817, 818,
            818, 819, 819, 820, 820, 820, 821, 822, 822, 823, 823, 824, 824, 825, 826, 827, 828,
            829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829,
            829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829,
            829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829,
            829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829,
            829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829,
            829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829,
            829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 829, 830, 831,
            832, 833, 834, 835, 835, 835, 835, 836, 837, 837, 837, 837, 837, 837, 837, 837, 837,
            837, 837, 837, 837, 837, 837, 837, 837, 837, 837, 837, 837, 837, 837, 837, 837, 837,
            837, 837, 837, 837, 837, 837, 837, 837, 837, 837, 837, 837, 837, 837, 837, 837, 837,
            837, 837, 837, 837, 837, 837, 837, 838, 839, 840, 841, 842, 843, 844, 845, 846, 847,
            848, 849, 850, 851, 852, 853, 854, 855, 856, 857, 858, 859, 860, 861, 862, 863, 864,
            865, 866, 867, 868, 869, 870, 871, 872, 873, 874, 875, 876, 877, 878, 879, 880, 881,
            882, 883, 884, 885, 886, 887, 888, 889, 890, 891, 892, 893, 894, 895, 896, 897, 898,
            899, 900, 901, 901, 902, 903, 904, 905, 906, 907, 908, 909, 909, 909, 909, 909, 909,
            909, 910, 910, 911, 912, 913, 914, 915, 916, 917, 918, 919, 920, 921, 922, 923, 924,
            925, 925, 926, 927, 928, 928, 928, 928, 928, 929, 929, 930, 931, 932, 932, 932, 932,
            933, 933, 934, 934, 935, 935, 936, 936, 937, 937, 937, 937, 937, 937, 938, 938, 938,
            938, 938, 938, 938, 938, 938, 938, 938, 938, 939, 939, 939, 939, 939, 940, 941, 942,
            943, 944, 944, 944, 944, 945, 945, 945, 946, 947, 948, 949, 950, 951, 952, 953, 954,
            955, 956, 957, 958, 959, 960, 961, 962, 963, 964, 965, 965, 966, 967, 968, 969, 970,
            971, 972, 973, 974, 975, 976, 977, 978, 979, 980, 981, 982, 983, 984, 985, 986, 987,
            988, 989, 990, 991, 992, 993, 994, 995, 996, 997, 998, 999, 999, 999, 1000, 1000, 1000,
            1001, 1001, 1002, 1002, 1002, 1002, 1002, 1002, 1003, 1004, 1005, 1005, 1005, 1006,
            1007, 1008, 1008, 1008, 1009, 1010, 1011, 1012, 1013, 1014, 1015, 1016, 1016, 1016,
            1016, 1016, 1016, 1016, 1016, 1016, 1016, 1016, 1016, 1016, 1016, 1016, 1016, 1016,
            1016, 1016, 1016, 1016, 1016, 1016, 1017, 1018, 1018, 1018, 1018, 1018, 1018, 1018,
            1018, 1018, 1018, 1018, 1018, 1018, 1018, 1019, 1020, 1021, 1021, 1021, 1021, 1021,
            1021, 1021, 1021, 1021, 1021, 1021, 1021, 1021, 1022, 1023, 1024, 1025, 1025, 1025,
            1025, 1026, 1026, 1026, 1026, 1026, 1027, 1027, 1028, 1028, 1029, 1030, 1030, 1031,
        ],
        ranks0: &[
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 2, 2, 2, 2, 3, 4, 4, 4, 5, 6, 7, 8, 9, 9, 9, 10,
            10, 10, 10, 10, 11, 12, 12, 12, 12, 12, 12, 12, 12, 13, 13, 13, 13, 13, 13, 13, 13, 13,
            13, 13, 13, 13, 14, 14, 15, 15, 16, 16, 16, 17, 18, 19, 20, 20, 20, 20, 21, 22, 22, 22,
            23, 23, 23, 23, 23, 24, 25, 26, 26, 27, 28, 29, 30, 31, 32, 33, 33, 33, 34, 34, 35, 35,
            35, 36, 37, 37, 37, 38, 39, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 52,
            53, 54, 55, 55, 55, 55, 56, 56, 57, 58, 59, 60, 61, 62, 63, 64, 64, 65, 66, 67, 68, 69,
            70, 71, 72, 72, 73, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 82, 83, 84, 85, 85, 85, 86,
            87, 87, 88, 89, 90, 91, 92, 93, 94, 95, 95, 96, 97, 98, 98, 99, 99, 99, 100, 101, 101,
            101, 102, 102, 103, 104, 105, 105, 105, 106, 107, 108, 109, 109, 109, 110, 111, 112,
            113, 114, 114, 114, 114, 115, 115, 115, 116, 116, 116, 116, 116, 116, 116, 116, 116,
            117, 117, 117, 118, 119, 119, 119, 119, 120, 121, 121, 122, 123, 124, 124, 124, 124,
            124, 124, 125, 125, 125, 125, 125, 126, 126, 126, 126, 127, 128, 128, 129, 129, 130,
            130, 131, 131, 131, 131, 131, 131, 131, 131, 131, 132, 132, 133, 134, 135, 136, 137,
            138, 138, 138, 138, 139, 139, 139, 140, 141, 141, 142, 143, 143, 144, 144, 144, 144,
            144, 145, 145, 145, 145, 145, 145, 145, 145, 145, 145, 145, 145, 146, 147, 147, 148,
            149, 150, 151, 152, 153, 153, 153, 154, 154, 154, 155, 155, 155, 155, 155, 156, 157,
            157, 157, 158, 158, 159, 160, 160, 160, 160, 160, 160, 160, 160, 161, 162, 162, 162,
            163, 163, 163, 163, 163, 163, 163, 163, 164, 164, 165, 165, 165, 166, 167, 167, 168,
            168, 168, 169, 170, 170, 170, 170, 171, 171, 172, 172, 172, 173, 173, 173, 173, 173,
            173, 174, 174, 174, 174, 174, 174, 174, 175, 175, 175, 176, 177, 178, 178, 178, 179,
            179, 179, 179, 179, 179, 179, 179, 179, 180, 180, 180, 180, 180, 180, 181, 182, 182,
            183, 183, 184, 185, 186, 187, 187, 187, 187, 188, 188, 188, 188, 188, 188, 188, 188,
            189, 190, 191, 192, 192, 192, 193, 194, 195, 196, 197, 197, 198, 198, 199, 200, 200,
            201, 202, 202, 202, 203, 204, 205, 206, 207, 208, 209, 210, 211, 212, 213, 214, 215,
            216, 216, 216, 216, 216, 217, 218, 218, 218, 218, 218, 218, 218, 218, 218, 218, 218,
            218, 218, 218, 218, 219, 219, 219, 219, 219, 220, 221, 221, 221, 221, 222, 222, 222,
            223, 224, 225, 225, 226, 227, 227, 227, 227, 227, 227, 228, 229, 229, 229, 229, 229,
            230, 230, 230, 231, 231, 231, 231, 231, 231, 231, 231, 231, 231, 231, 231, 231, 232,
            232, 233, 233, 234, 234, 234, 235, 235, 235, 235, 236, 237, 237, 238, 238, 239, 240,
            240, 240, 240, 240, 240, 240, 240, 240, 241, 241, 241, 241, 242, 242, 242, 243, 243,
            243, 243, 243, 244, 244, 244, 244, 244, 244, 244, 244, 244, 244, 244, 244, 244, 245,
            246, 247, 248, 248, 249, 250, 251, 251, 252, 253, 253, 254, 254, 255, 256, 257, 258,
            259, 259, 260, 261, 262, 263, 264, 264, 264, 264, 264, 265, 265, 265, 266, 267, 267,
            267, 268, 269, 269, 270, 271, 271, 272, 273, 274, 274, 274, 274, 274, 274, 274, 274,
            274, 274, 275, 275, 276, 277, 278, 279, 280, 281, 282, 283, 283, 283, 283, 283, 283,
            283, 283, 283, 283, 284, 284, 285, 286, 286, 286, 287, 287, 288, 288, 289, 289, 289,
            289, 290, 290, 290, 291, 292, 292, 293, 293, 294, 295, 295, 296, 296, 296, 297, 297,
            298, 299, 300, 300, 300, 300, 301, 302, 302, 303, 304, 305, 306, 307, 308, 309, 310,
            311, 312, 313, 314, 314, 315, 316, 317, 318, 319, 319, 319, 320, 320, 320, 320, 320,
            320, 320, 320, 320, 320, 320, 321, 321, 321, 322, 322, 322, 322, 322, 322, 322, 322,
            322, 322, 322, 322, 322, 322, 322, 322, 323, 323, 323, 323, 323, 323, 323, 323, 323,
            323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323,
            323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323,
            323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 323, 324, 325, 326,
            327, 327, 328, 329, 330, 331, 332, 332, 332, 333, 334, 334, 334, 335, 336, 337, 338,
            339, 340, 341, 342, 343, 343, 343, 344, 345, 345, 345, 346, 346, 347, 347, 347, 347,
            347, 348, 348, 348, 349, 349, 349, 350, 350, 351, 352, 353, 353, 353, 353, 353, 353,
            354, 355, 355, 355, 355, 355, 356, 357, 358, 359, 360, 361, 362, 363, 364, 364, 364,
            364, 365, 366, 366, 367, 368, 368, 369, 369, 370, 370, 371, 372, 372, 372, 373, 374,
            375, 376, 376, 376, 376, 376, 376, 376, 376, 377, 377, 378, 378, 378, 378, 378, 378,
            378, 379, 379, 379, 379, 379, 380, 380, 381, 382, 382, 382, 382, 382, 382, 383, 383,
            384, 384, 385, 385, 385, 386, 387, 387, 388, 388, 389, 390, 391, 392, 392, 392, 393,
            393, 393, 393, 393, 393, 394, 395, 396, 396, 397, 397, 397, 398, 399, 399, 399, 399,
            399, 399, 399, 400, 401, 401, 401, 401, 402, 403, 404, 405, 406, 407, 408, 408, 408,
            409, 410, 410, 410, 410, 410, 411, 411, 412, 412, 412, 412, 412, 412, 413, 414, 414,
            415, 415, 415, 416, 417, 417, 417, 418, 418, 418, 418, 418, 418, 419, 420, 420, 420,
            420, 420, 421, 421, 422, 423, 423, 423, 423, 423, 423, 424, 425, 426, 426, 426, 427,
            428, 428, 428, 429, 429, 429, 429, 429, 430, 431, 432, 432, 432, 433, 433, 433, 433,
            434, 434, 435, 435, 435, 435, 436, 437, 438, 438, 439, 439, 439, 440, 440, 440, 441,
            442, 442, 443, 444, 445, 446, 446, 447, 448, 449, 449, 450, 451, 452, 453, 454, 454,
            454, 455, 456, 457, 458, 458, 458, 458, 458, 459, 460, 460, 460, 460, 460, 461, 461,
            461, 461, 461, 461, 461, 461, 461, 461, 461, 461, 461, 461, 461, 461, 461, 461, 461,
            462, 462, 462, 462, 462, 462, 462, 462, 462, 462, 462, 463, 463, 463, 463, 463, 464,
            464, 464, 464, 464, 464, 464, 465, 465, 465, 466, 466, 466, 467, 468, 468, 469, 469,
            469, 470, 471, 472, 473, 473, 473, 473, 473, 473, 473, 474, 474, 474, 474, 474, 474,
            474, 474, 474, 474, 475, 476, 477, 478, 479, 480, 481, 482, 483, 484, 484, 484, 484,
            485, 485, 486, 486, 487, 488, 489, 490, 491, 492, 493, 493, 494, 495, 495, 495, 495,
            496, 497, 498, 498, 498, 498, 498, 498, 498, 499, 500, 500, 500, 500, 501, 502, 502,
            503, 504, 505, 506, 507, 507, 508, 509, 510, 510, 511, 512, 512, 512, 512, 512, 512,
            512, 512, 512, 512, 513, 513, 513, 513, 513, 514, 514, 515, 516, 517, 517, 518, 519,
            519, 520, 521, 521, 522, 523, 523, 524, 524, 524, 525, 526, 526, 527, 527, 527, 527,
            528, 529, 529, 529, 529, 530, 531, 531, 532, 533, 533, 533, 534, 534, 534, 534, 534,
            534, 535, 535, 535, 536, 536, 536, 537, 538, 538, 539, 540, 541, 541, 541, 542, 542,
            542, 543, 543, 544, 544, 545, 546, 546, 546, 546, 547, 547, 547, 547, 547, 547, 547,
            548, 549, 549, 549, 550, 551, 551, 551, 551, 551, 551, 551, 551, 551, 551, 551, 551,
            551, 552, 553, 553, 553, 553, 553, 554, 554, 554, 555, 555, 556, 557, 558, 558, 558,
            558, 558, 558, 558, 558, 558, 558, 558, 558, 558, 558, 558, 558, 558, 558, 558, 558,
            558, 559, 559, 559, 559, 559, 560, 560, 560, 560, 560, 560, 560, 561, 561, 562, 563,
            564, 565, 566, 567, 567, 567, 567, 568, 569, 570, 571, 572, 573, 574, 575, 575, 575,
            575, 576, 577, 578, 579, 580, 580, 581, 582, 583, 584, 584, 585, 586, 587, 588, 589,
            590, 591, 592, 593, 594, 595, 596, 597, 598, 599, 600, 601, 601, 602, 603, 604, 605,
            606, 607, 607, 608, 609, 609, 610, 610, 611, 611, 612, 613, 613, 613, 614, 614, 615,
            615, 616, 616, 616, 616, 616, 616, 617, 618, 619, 620, 621, 622, 623, 624, 625, 626,
            627, 628, 629, 630, 631, 632, 633, 634, 635, 636, 637, 638, 639, 640, 641, 642, 643,
            644, 645, 646, 647, 648, 649, 650, 651, 652, 653, 654, 655, 656, 657, 658, 659, 660,
            661, 662, 663, 664, 665, 666, 667, 668, 669, 670, 671, 672, 673, 674, 675, 676, 677,
            678, 679, 680, 681, 682, 683, 684, 685, 686, 687, 688, 689, 690, 691, 692, 693, 694,
            695, 696, 697, 698, 699, 700, 701, 702, 703, 704, 705, 706, 707, 708, 709, 710, 711,
            712, 713, 714, 715, 716, 717, 718, 719, 720, 721, 722, 723, 724, 725, 726, 727, 728,
            729, 730, 731, 732, 732, 732, 732, 732, 732, 732, 733, 734, 735, 735, 735, 736, 737,
            738, 739, 740, 741, 742, 743, 744, 745, 746, 747, 748, 749, 750, 751, 752, 753, 754,
            755, 756, 757, 758, 759, 760, 761, 762, 763, 764, 765, 766, 767, 768, 769, 770, 771,
            772, 773, 774, 775, 776, 777, 778, 779, 780, 781, 782, 783, 784, 784, 784, 784, 784,
            784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784,
            784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784,
            784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784, 784,
            784, 784, 784, 784, 784, 784, 784, 784, 784, 785, 785, 785, 785, 785, 785, 785, 785,
            785, 786, 787, 788, 789, 790, 791, 791, 792, 792, 792, 792, 792, 792, 792, 792, 792,
            792, 792, 792, 792, 792, 792, 792, 793, 793, 793, 793, 794, 795, 796, 797, 797, 798,
            798, 798, 798, 799, 800, 801, 801, 802, 802, 803, 803, 804, 804, 805, 805, 806, 807,
            808, 809, 810, 810, 811, 812, 813, 814, 815, 816, 817, 818, 819, 820, 821, 821, 822,
            823, 824, 825, 825, 825, 825, 825, 825, 826, 827, 828, 828, 829, 830, 830, 830, 830,
            830, 830, 830, 830, 830, 830, 830, 830, 830, 830, 830, 830, 830, 830, 830, 830, 830,
            831, 831, 831, 831, 831, 831, 831, 831, 831, 831, 831, 831, 831, 831, 831, 831, 831,
            831, 831, 831, 831, 831, 831, 831, 831, 831, 831, 831, 831, 831, 831, 831, 831, 831,
            831, 832, 833, 833, 834, 835, 835, 836, 836, 837, 838, 839, 840, 841, 841, 841, 841,
            842, 843, 843, 843, 843, 844, 845, 845, 845, 845, 845, 845, 845, 845, 845, 846, 847,
            848, 849, 850, 851, 852, 853, 854, 855, 856, 857, 858, 859, 860, 861, 862, 863, 864,
            865, 866, 867, 867, 867, 868, 869, 870, 871, 872, 873, 874, 875, 876, 877, 878, 879,
            880, 880, 880, 880, 881, 882, 883, 884, 885, 886, 887, 888, 889, 890, 891, 892, 892,
            892, 892, 892, 893, 894, 895, 895, 896, 897, 898, 899, 899, 900, 900, 901, 901, 901,
            902, 902,
        ],
        selects: &[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 13, 14, 15, 18, 19, 25, 26, 28, 29, 30, 31, 34, 35,
            36, 37, 38, 39, 40, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 55, 57, 59, 60, 65,
            66, 67, 70, 71, 73, 74, 75, 76, 80, 88, 89, 91, 93, 94, 97, 98, 101, 115, 119, 120,
            121, 123, 132, 141, 143, 153, 157, 158, 161, 170, 174, 176, 177, 180, 181, 183, 187,
            188, 193, 194, 200, 201, 202, 204, 205, 207, 208, 209, 210, 211, 212, 213, 214, 216,
            217, 220, 221, 222, 225, 229, 230, 231, 232, 233, 235, 236, 237, 238, 240, 241, 242,
            245, 247, 249, 251, 252, 253, 254, 255, 256, 257, 258, 260, 267, 268, 269, 271, 272,
            275, 278, 280, 281, 282, 283, 285, 286, 287, 288, 289, 290, 291, 292, 293, 294, 295,
            298, 305, 306, 308, 309, 311, 312, 313, 314, 317, 318, 320, 323, 324, 325, 326, 327,
            328, 329, 332, 333, 335, 336, 337, 338, 339, 340, 341, 343, 345, 346, 349, 351, 352,
            355, 356, 357, 359, 361, 362, 364, 365, 366, 367, 368, 370, 371, 372, 373, 374, 375,
            377, 378, 382, 383, 385, 386, 387, 388, 389, 390, 391, 392, 394, 395, 396, 397, 398,
            401, 403, 408, 409, 410, 412, 413, 414, 415, 416, 417, 418, 423, 424, 430, 432, 435,
            438, 439, 454, 455, 456, 457, 460, 461, 462, 463, 464, 465, 466, 467, 468, 469, 470,
            471, 472, 473, 475, 476, 477, 478, 481, 482, 483, 485, 486, 490, 493, 494, 495, 496,
            497, 500, 501, 502, 503, 505, 506, 508, 509, 510, 511, 512, 513, 514, 515, 516, 517,
            518, 519, 521, 523, 525, 526, 528, 529, 530, 533, 535, 538, 539, 540, 541, 542, 543,
            544, 545, 547, 548, 549, 551, 552, 554, 555, 556, 557, 559, 560, 561, 562, 563, 564,
            565, 566, 567, 568, 569, 570, 575, 579, 582, 584, 590, 596, 597, 598, 599, 601, 602,
            605, 606, 609, 612, 616, 617, 618, 619, 620, 621, 622, 623, 624, 626, 635, 636, 637,
            638, 639, 640, 641, 642, 643, 645, 648, 649, 651, 653, 655, 656, 657, 659, 660, 663,
            665, 668, 670, 671, 673, 677, 678, 679, 682, 695, 701, 702, 704, 705, 706, 707, 708,
            709, 710, 711, 712, 713, 715, 716, 718, 719, 720, 721, 722, 723, 724, 725, 726, 727,
            728, 729, 730, 731, 732, 734, 735, 736, 737, 738, 739, 740, 741, 742, 743, 744, 745,
            746, 747, 748, 749, 750, 751, 752, 753, 754, 755, 756, 757, 758, 759, 760, 761, 762,
            763, 764, 765, 766, 767, 768, 769, 770, 771, 772, 773, 774, 775, 776, 777, 778, 779,
            780, 781, 782, 783, 784, 785, 786, 787, 788, 789, 794, 800, 801, 804, 805, 815, 816,
            819, 820, 822, 824, 825, 826, 827, 829, 830, 832, 833, 835, 839, 840, 841, 842, 843,
            846, 847, 848, 849, 859, 860, 861, 864, 867, 869, 871, 874, 875, 880, 881, 882, 883,
            884, 885, 886, 888, 890, 891, 892, 893, 894, 895, 897, 898, 899, 900, 902, 905, 906,
            907, 908, 909, 911, 913, 915, 916, 919, 921, 926, 927, 929, 930, 931, 932, 933, 937,
            939, 940, 943, 944, 945, 946, 947, 948, 951, 952, 953, 961, 962, 965, 966, 967, 968,
            970, 972, 973, 974, 975, 976, 979, 981, 982, 985, 986, 988, 989, 990, 991, 992, 995,
            996, 997, 998, 1000, 1003, 1004, 1005, 1006, 1007, 1011, 1012, 1015, 1016, 1018, 1019,
            1020, 1021, 1025, 1026, 1028, 1029, 1030, 1032, 1034, 1035, 1036, 1040, 1042, 1043,
            1045, 1046, 1049, 1054, 1058, 1064, 1065, 1070, 1071, 1072, 1073, 1076, 1077, 1078,
            1079, 1081, 1082, 1083, 1084, 1085, 1086, 1087, 1088, 1089, 1090, 1091, 1092, 1093,
            1094, 1095, 1096, 1097, 1098, 1100, 1101, 1102, 1103, 1104, 1105, 1106, 1107, 1108,
            1109, 1111, 1112, 1113, 1114, 1116, 1117, 1118, 1119, 1120, 1121, 1123, 1124, 1126,
            1127, 1130, 1132, 1133, 1138, 1139, 1140, 1141, 1142, 1143, 1145, 1146, 1147, 1148,
            1149, 1150, 1151, 1152, 1153, 1164, 1165, 1166, 1168, 1170, 1178, 1181, 1182, 1183,
            1187, 1188, 1189, 1190, 1191, 1192, 1195, 1196, 1197, 1200, 1206, 1210, 1213, 1214,
            1215, 1216, 1217, 1218, 1219, 1220, 1221, 1223, 1224, 1225, 1226, 1228, 1232, 1235,
            1238, 1241, 1243, 1244, 1247, 1249, 1250, 1251, 1254, 1255, 1256, 1259, 1262, 1263,
            1265, 1266, 1267, 1268, 1269, 1271, 1272, 1274, 1275, 1278, 1282, 1283, 1285, 1286,
            1288, 1290, 1293, 1294, 1295, 1297, 1298, 1299, 1300, 1301, 1302, 1305, 1306, 1309,
            1310, 1311, 1312, 1313, 1314, 1315, 1316, 1317, 1318, 1319, 1320, 1323, 1324, 1325,
            1326, 1328, 1329, 1331, 1335, 1336, 1337, 1338, 1339, 1340, 1341, 1342, 1343, 1344,
            1345, 1346, 1347, 1348, 1349, 1350, 1351, 1352, 1353, 1354, 1356, 1357, 1358, 1359,
            1361, 1362, 1363, 1364, 1365, 1366, 1368, 1375, 1376, 1377, 1386, 1387, 1388, 1394,
            1399, 1417, 1424, 1427, 1429, 1431, 1434, 1435, 1437, 1439, 1441, 1442, 1443, 1444,
            1445, 1562, 1563, 1564, 1565, 1566, 1567, 1571, 1572, 1622, 1623, 1624, 1625, 1626,
            1627, 1628, 1629, 1630, 1631, 1632, 1633, 1634, 1635, 1636, 1637, 1638, 1639, 1640,
            1641, 1642, 1643, 1644, 1645, 1646, 1647, 1648, 1649, 1650, 1651, 1652, 1653, 1654,
            1655, 1656, 1657, 1658, 1659, 1660, 1661, 1662, 1663, 1664, 1665, 1666, 1667, 1668,
            1669, 1670, 1671, 1672, 1673, 1674, 1675, 1676, 1677, 1678, 1679, 1680, 1681, 1682,
            1683, 1684, 1685, 1687, 1688, 1689, 1690, 1691, 1692, 1693, 1694, 1701, 1703, 1704,
            1705, 1706, 1707, 1708, 1709, 1710, 1711, 1712, 1713, 1714, 1715, 1716, 1717, 1719,
            1720, 1721, 1726, 1728, 1729, 1730, 1734, 1736, 1738, 1740, 1742, 1748, 1760, 1765,
            1766, 1767, 1768, 1769, 1773, 1776, 1777, 1778, 1779, 1780, 1781, 1782, 1783, 1784,
            1785, 1786, 1787, 1788, 1789, 1790, 1791, 1792, 1793, 1794, 1795, 1797, 1798, 1799,
            1800, 1801, 1802, 1803, 1804, 1805, 1806, 1807, 1808, 1809, 1810, 1811, 1812, 1813,
            1814, 1815, 1816, 1817, 1818, 1819, 1820, 1821, 1822, 1823, 1824, 1825, 1826, 1827,
            1828, 1829, 1830, 1833, 1836, 1838, 1844, 1845, 1846, 1849, 1850, 1851, 1854, 1855,
            1856, 1857, 1858, 1859, 1860, 1861, 1884, 1885, 1899, 1900, 1901, 1914, 1915, 1916,
            1917, 1921, 1926, 1928, 1930, 1931, 1933,
        ],
        selects0: &[
            0, 10, 12, 16, 17, 20, 21, 22, 23, 24, 27, 32, 33, 41, 54, 56, 58, 61, 62, 63, 64, 68,
            69, 72, 77, 78, 79, 81, 82, 83, 84, 85, 86, 87, 90, 92, 95, 96, 99, 100, 102, 103, 104,
            105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 116, 117, 118, 122, 124, 125, 126,
            127, 128, 129, 130, 131, 133, 134, 135, 136, 137, 138, 139, 140, 142, 144, 145, 146,
            147, 148, 149, 150, 151, 152, 154, 155, 156, 159, 160, 162, 163, 164, 165, 166, 167,
            168, 169, 171, 172, 173, 175, 178, 179, 182, 184, 185, 186, 189, 190, 191, 192, 195,
            196, 197, 198, 199, 203, 206, 215, 218, 219, 223, 224, 226, 227, 228, 234, 239, 243,
            244, 246, 248, 250, 259, 261, 262, 263, 264, 265, 266, 270, 273, 274, 276, 277, 279,
            284, 296, 297, 299, 300, 301, 302, 303, 304, 307, 310, 315, 316, 319, 321, 322, 330,
            331, 334, 342, 344, 347, 348, 350, 353, 354, 358, 360, 363, 369, 376, 379, 380, 381,
            384, 393, 399, 400, 402, 404, 405, 406, 407, 411, 419, 420, 421, 422, 425, 426, 427,
            428, 429, 431, 433, 434, 436, 437, 440, 441, 442, 443, 444, 445, 446, 447, 448, 449,
            450, 451, 452, 453, 458, 459, 474, 479, 480, 484, 487, 488, 489, 491, 492, 498, 499,
            504, 507, 520, 522, 524, 527, 531, 532, 534, 536, 537, 546, 550, 553, 558, 571, 572,
            573, 574, 576, 577, 578, 580, 581, 583, 585, 586, 587, 588, 589, 591, 592, 593, 594,
            595, 600, 603, 604, 607, 608, 610, 611, 613, 614, 615, 625, 627, 628, 629, 630, 631,
            632, 633, 634, 644, 646, 647, 650, 652, 654, 658, 661, 662, 664, 666, 667, 669, 672,
            674, 675, 676, 680, 681, 683, 684, 685, 686, 687, 688, 689, 690, 691, 692, 693, 694,
            696, 697, 698, 699, 700, 703, 714, 717, 733, 790, 791, 792, 793, 795, 796, 797, 798,
            799, 802, 803, 806, 807, 808, 809, 810, 811, 812, 813, 814, 817, 818, 821, 823, 828,
            831, 834, 836, 837, 838, 844, 845, 850, 851, 852, 853, 854, 855, 856, 857, 858, 862,
            863, 865, 866, 868, 870, 872, 873, 876, 877, 878, 879, 887, 889, 896, 901, 903, 904,
            910, 912, 914, 917, 918, 920, 922, 923, 924, 925, 928, 934, 935, 936, 938, 941, 942,
            949, 950, 954, 955, 956, 957, 958, 959, 960, 963, 964, 969, 971, 977, 978, 980, 983,
            984, 987, 993, 994, 999, 1001, 1002, 1008, 1009, 1010, 1013, 1014, 1017, 1022, 1023,
            1024, 1027, 1031, 1033, 1037, 1038, 1039, 1041, 1044, 1047, 1048, 1050, 1051, 1052,
            1053, 1055, 1056, 1057, 1059, 1060, 1061, 1062, 1063, 1066, 1067, 1068, 1069, 1074,
            1075, 1080, 1099, 1110, 1115, 1122, 1125, 1128, 1129, 1131, 1134, 1135, 1136, 1137,
            1144, 1154, 1155, 1156, 1157, 1158, 1159, 1160, 1161, 1162, 1163, 1167, 1169, 1171,
            1172, 1173, 1174, 1175, 1176, 1177, 1179, 1180, 1184, 1185, 1186, 1193, 1194, 1198,
            1199, 1201, 1202, 1203, 1204, 1205, 1207, 1208, 1209, 1211, 1212, 1222, 1227, 1229,
            1230, 1231, 1233, 1234, 1236, 1237, 1239, 1240, 1242, 1245, 1246, 1248, 1252, 1253,
            1257, 1258, 1260, 1261, 1264, 1270, 1273, 1276, 1277, 1279, 1280, 1281, 1284, 1287,
            1289, 1291, 1292, 1296, 1303, 1304, 1307, 1308, 1321, 1322, 1327, 1330, 1332, 1333,
            1334, 1355, 1360, 1367, 1369, 1370, 1371, 1372, 1373, 1374, 1378, 1379, 1380, 1381,
            1382, 1383, 1384, 1385, 1389, 1390, 1391, 1392, 1393, 1395, 1396, 1397, 1398, 1400,
            1401, 1402, 1403, 1404, 1405, 1406, 1407, 1408, 1409, 1410, 1411, 1412, 1413, 1414,
            1415, 1416, 1418, 1419, 1420, 1421, 1422, 1423, 1425, 1426, 1428, 1430, 1432, 1433,
            1436, 1438, 1440, 1446, 1447, 1448, 1449, 1450, 1451, 1452, 1453, 1454, 1455, 1456,
            1457, 1458, 1459, 1460, 1461, 1462, 1463, 1464, 1465, 1466, 1467, 1468, 1469, 1470,
            1471, 1472, 1473, 1474, 1475, 1476, 1477, 1478, 1479, 1480, 1481, 1482, 1483, 1484,
            1485, 1486, 1487, 1488, 1489, 1490, 1491, 1492, 1493, 1494, 1495, 1496, 1497, 1498,
            1499, 1500, 1501, 1502, 1503, 1504, 1505, 1506, 1507, 1508, 1509, 1510, 1511, 1512,
            1513, 1514, 1515, 1516, 1517, 1518, 1519, 1520, 1521, 1522, 1523, 1524, 1525, 1526,
            1527, 1528, 1529, 1530, 1531, 1532, 1533, 1534, 1535, 1536, 1537, 1538, 1539, 1540,
            1541, 1542, 1543, 1544, 1545, 1546, 1547, 1548, 1549, 1550, 1551, 1552, 1553, 1554,
            1555, 1556, 1557, 1558, 1559, 1560, 1561, 1568, 1569, 1570, 1573, 1574, 1575, 1576,
            1577, 1578, 1579, 1580, 1581, 1582, 1583, 1584, 1585, 1586, 1587, 1588, 1589, 1590,
            1591, 1592, 1593, 1594, 1595, 1596, 1597, 1598, 1599, 1600, 1601, 1602, 1603, 1604,
            1605, 1606, 1607, 1608, 1609, 1610, 1611, 1612, 1613, 1614, 1615, 1616, 1617, 1618,
            1619, 1620, 1621, 1686, 1695, 1696, 1697, 1698, 1699, 1700, 1702, 1718, 1722, 1723,
            1724, 1725, 1727, 1731, 1732, 1733, 1735, 1737, 1739, 1741, 1743, 1744, 1745, 1746,
            1747, 1749, 1750, 1751, 1752, 1753, 1754, 1755, 1756, 1757, 1758, 1759, 1761, 1762,
            1763, 1764, 1770, 1771, 1772, 1774, 1775, 1796, 1831, 1832, 1834, 1835, 1837, 1839,
            1840, 1841, 1842, 1843, 1847, 1848, 1852, 1853, 1862, 1863, 1864, 1865, 1866, 1867,
            1868, 1869, 1870, 1871, 1872, 1873, 1874, 1875, 1876, 1877, 1878, 1879, 1880, 1881,
            1882, 1883, 1886, 1887, 1888, 1889, 1890, 1891, 1892, 1893, 1894, 1895, 1896, 1897,
            1898, 1902, 1903, 1904, 1905, 1906, 1907, 1908, 1909, 1910, 1911, 1912, 1913, 1918,
            1919, 1920, 1922, 1923, 1924, 1925, 1927, 1929, 1932,
        ],
    };

    const BAD_TREE_NODE_3: &TestCase = &TestCase {
        bits: &[
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, false, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, false, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, false, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, false, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, false, true, true, true, true, true, true, false, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, false, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, false, false, false, false, false, true, true,
            true, true, true, true, true, true, true, true, true, true, false, false, false, false,
            false, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, false, true, true, false, false, true, true, true, true,
            false, true, true, true, true, true, false, true, true, true, false, false, true, true,
            true, true, true, true, true, true, true, true, false, true, true, false, true, true,
            true, false, true, true, true, true, true, true, true, true, true, true, true, true,
            false, true, true, true, true, true, true, true, true, true, true, true, false, false,
            true, true, true, true, true, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, true, true, true, true, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, true, false, false, false, false,
            false, false, false, false, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, false, true, true, true, true, true, true, true, true, true,
            true, true, false, true, true, true, true, true, true, true, true, true, true, false,
            true, true, true, true, true, false, false, false, false, false, true, true, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            true, true, true, true, true, false, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, false, false, false, false, false,
            false, false, false, false, false,
        ],
        ranks: &[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45,
            46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67,
            68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89,
            90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108,
            109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125,
            126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142,
            143, 144, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158,
            159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 174,
            175, 176, 177, 178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191,
            192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207, 208,
            209, 210, 211, 212, 213, 214, 215, 216, 216, 217, 218, 219, 220, 221, 222, 223, 224,
            225, 226, 227, 228, 229, 230, 231, 232, 233, 234, 235, 236, 237, 238, 239, 240, 241,
            242, 243, 244, 245, 246, 247, 248, 249, 250, 251, 252, 253, 254, 255, 256, 257, 258,
            259, 260, 261, 262, 263, 264, 265, 266, 267, 268, 269, 270, 271, 272, 273, 273, 274,
            275, 276, 277, 278, 279, 280, 281, 282, 283, 284, 285, 286, 287, 288, 289, 290, 291,
            292, 293, 294, 295, 296, 297, 298, 299, 300, 301, 302, 303, 304, 305, 306, 307, 308,
            309, 310, 311, 312, 313, 314, 315, 316, 317, 318, 319, 320, 321, 322, 323, 324, 325,
            326, 327, 328, 329, 330, 331, 332, 333, 334, 335, 336, 337, 338, 339, 340, 341, 342,
            343, 344, 345, 346, 347, 348, 349, 350, 351, 352, 353, 354, 355, 356, 357, 358, 359,
            360, 361, 362, 363, 364, 365, 366, 367, 368, 369, 370, 371, 372, 373, 374, 375, 376,
            377, 378, 379, 380, 381, 382, 383, 384, 385, 386, 387, 388, 389, 390, 391, 391, 392,
            393, 394, 395, 396, 397, 397, 398, 399, 400, 401, 402, 403, 404, 405, 406, 407, 408,
            409, 410, 411, 412, 413, 414, 415, 416, 417, 418, 419, 420, 421, 422, 423, 424, 425,
            426, 427, 428, 429, 430, 431, 432, 433, 434, 435, 436, 437, 438, 439, 440, 441, 442,
            443, 444, 445, 446, 447, 448, 449, 450, 451, 452, 453, 454, 455, 456, 457, 458, 459,
            460, 461, 462, 463, 464, 465, 466, 466, 467, 468, 469, 470, 471, 472, 473, 474, 475,
            476, 477, 478, 479, 480, 481, 482, 483, 484, 485, 486, 487, 488, 489, 490, 491, 492,
            493, 494, 495, 496, 497, 498, 499, 500, 501, 502, 503, 504, 505, 506, 507, 508, 509,
            510, 511, 512, 513, 514, 515, 516, 517, 518, 519, 520, 521, 522, 523, 524, 525, 526,
            527, 528, 529, 530, 531, 531, 531, 531, 531, 531, 532, 533, 534, 535, 536, 537, 538,
            539, 540, 541, 542, 543, 543, 543, 543, 543, 543, 544, 545, 546, 547, 548, 549, 550,
            551, 552, 553, 554, 555, 556, 557, 558, 559, 560, 561, 561, 562, 563, 563, 563, 564,
            565, 566, 567, 567, 568, 569, 570, 571, 572, 572, 573, 574, 575, 575, 575, 576, 577,
            578, 579, 580, 581, 582, 583, 584, 585, 585, 586, 587, 587, 588, 589, 590, 590, 591,
            592, 593, 594, 595, 596, 597, 598, 599, 600, 601, 602, 602, 603, 604, 605, 606, 607,
            608, 609, 610, 611, 612, 613, 613, 613, 614, 615, 616, 617, 618, 618, 618, 618, 618,
            618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618,
            618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618,
            618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618,
            618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618,
            618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 618, 619, 620, 621,
            622, 622, 622, 622, 622, 622, 622, 622, 622, 622, 622, 622, 622, 622, 622, 622, 622,
            622, 622, 622, 622, 622, 622, 622, 622, 622, 622, 622, 622, 622, 622, 622, 622, 622,
            622, 622, 622, 622, 622, 622, 622, 622, 622, 622, 622, 622, 622, 623, 623, 623, 623,
            623, 623, 623, 623, 623, 624, 625, 626, 627, 628, 629, 630, 631, 632, 633, 634, 635,
            636, 637, 637, 638, 639, 640, 641, 642, 643, 644, 645, 646, 647, 648, 648, 649, 650,
            651, 652, 653, 654, 655, 656, 657, 658, 658, 659, 660, 661, 662, 663, 663, 663, 663,
            663, 663, 664, 665, 665, 665, 665, 665, 665, 665, 665, 665, 665, 665, 665, 665, 665,
            666, 667, 668, 669, 670, 670, 671, 672, 673, 674, 675, 676, 677, 678, 679, 680, 681,
            682, 683, 684, 685, 686, 687, 688, 689, 690, 691, 692, 693, 694, 695, 696, 697, 698,
            699, 700, 700, 700, 700, 700, 700, 700, 700, 700, 700, 700,
        ],
        ranks0: &[
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
            1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
            2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
            3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
            3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
            4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
            4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
            4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
            4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 5, 5, 5, 5, 5, 5, 5, 6, 6, 6,
            6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6,
            6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6,
            6, 6, 6, 6, 6, 6, 6, 6, 6, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
            7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
            7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 8, 9, 10, 11, 12, 12, 12, 12, 12,
            12, 12, 12, 12, 12, 12, 12, 12, 13, 14, 15, 16, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17,
            17, 17, 17, 17, 17, 17, 17, 17, 17, 18, 18, 18, 19, 20, 20, 20, 20, 20, 21, 21, 21, 21,
            21, 21, 22, 22, 22, 22, 23, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 25, 25, 25, 26,
            26, 26, 26, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 28, 28, 28, 28, 28, 28,
            28, 28, 28, 28, 28, 28, 29, 30, 30, 30, 30, 30, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39,
            40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61,
            62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83,
            84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103,
            104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 116, 116, 116, 116,
            117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133,
            134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150,
            151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 162, 163, 164, 165, 166,
            167, 168, 169, 170, 170, 170, 170, 170, 170, 170, 170, 170, 170, 170, 170, 170, 170,
            170, 171, 171, 171, 171, 171, 171, 171, 171, 171, 171, 171, 171, 172, 172, 172, 172,
            172, 172, 172, 172, 172, 172, 172, 173, 173, 173, 173, 173, 173, 174, 175, 176, 177,
            178, 178, 178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191, 191,
            191, 191, 191, 191, 192, 192, 192, 192, 192, 192, 192, 192, 192, 192, 192, 192, 192,
            192, 192, 192, 192, 192, 192, 192, 192, 192, 192, 192, 192, 192, 192, 192, 192, 192,
            192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202,
        ],
        selects: &[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45,
            46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67,
            68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89,
            90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108,
            109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125,
            126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142,
            143, 144, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160,
            161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 177, 178,
            179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191, 192, 193, 194, 195,
            196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210, 211, 212,
            213, 214, 215, 216, 217, 218, 220, 221, 222, 223, 224, 225, 226, 227, 228, 229, 230,
            231, 232, 233, 234, 235, 236, 237, 238, 239, 240, 241, 242, 243, 244, 245, 246, 247,
            248, 249, 250, 251, 252, 253, 254, 255, 256, 257, 258, 259, 260, 261, 262, 263, 264,
            265, 266, 267, 268, 269, 270, 271, 272, 273, 274, 275, 276, 278, 279, 280, 281, 282,
            283, 284, 285, 286, 287, 288, 289, 290, 291, 292, 293, 294, 295, 296, 297, 298, 299,
            300, 301, 302, 303, 304, 305, 306, 307, 308, 309, 310, 311, 312, 313, 314, 315, 316,
            317, 318, 319, 320, 321, 322, 323, 324, 325, 326, 327, 328, 329, 330, 331, 332, 333,
            334, 335, 336, 337, 338, 339, 340, 341, 342, 343, 344, 345, 346, 347, 348, 349, 350,
            351, 352, 353, 354, 355, 356, 357, 358, 359, 360, 361, 362, 363, 364, 365, 366, 367,
            368, 369, 370, 371, 372, 373, 374, 375, 376, 377, 378, 379, 380, 381, 382, 383, 384,
            385, 386, 387, 388, 389, 390, 391, 392, 393, 394, 395, 397, 398, 399, 400, 401, 402,
            404, 405, 406, 407, 408, 409, 410, 411, 412, 413, 414, 415, 416, 417, 418, 419, 420,
            421, 422, 423, 424, 425, 426, 427, 428, 429, 430, 431, 432, 433, 434, 435, 436, 437,
            438, 439, 440, 441, 442, 443, 444, 445, 446, 447, 448, 449, 450, 451, 452, 453, 454,
            455, 456, 457, 458, 459, 460, 461, 462, 463, 464, 465, 466, 467, 468, 469, 470, 471,
            472, 474, 475, 476, 477, 478, 479, 480, 481, 482, 483, 484, 485, 486, 487, 488, 489,
            490, 491, 492, 493, 494, 495, 496, 497, 498, 499, 500, 501, 502, 503, 504, 505, 506,
            507, 508, 509, 510, 511, 512, 513, 514, 515, 516, 517, 518, 519, 520, 521, 522, 523,
            524, 525, 526, 527, 528, 529, 530, 531, 532, 533, 534, 535, 536, 537, 538, 544, 545,
            546, 547, 548, 549, 550, 551, 552, 553, 554, 555, 561, 562, 563, 564, 565, 566, 567,
            568, 569, 570, 571, 572, 573, 574, 575, 576, 577, 578, 580, 581, 584, 585, 586, 587,
            589, 590, 591, 592, 593, 595, 596, 597, 600, 601, 602, 603, 604, 605, 606, 607, 608,
            609, 611, 612, 614, 615, 616, 618, 619, 620, 621, 622, 623, 624, 625, 626, 627, 628,
            629, 631, 632, 633, 634, 635, 636, 637, 638, 639, 640, 641, 644, 645, 646, 647, 648,
            735, 736, 737, 738, 785, 794, 795, 796, 797, 798, 799, 800, 801, 802, 803, 804, 805,
            806, 807, 809, 810, 811, 812, 813, 814, 815, 816, 817, 818, 819, 821, 822, 823, 824,
            825, 826, 827, 828, 829, 830, 832, 833, 834, 835, 836, 842, 843, 857, 858, 859, 860,
            861, 863, 864, 865, 866, 867, 868, 869, 870, 871, 872, 873, 874, 875, 876, 877, 878,
            879, 880, 881, 882, 883, 884, 885, 886, 887, 888, 889, 890, 891, 892,
        ],
        selects0: &[
            0, 145, 176, 219, 277, 396, 403, 473, 539, 540, 541, 542, 543, 556, 557, 558, 559, 560,
            579, 582, 583, 588, 594, 598, 599, 610, 613, 617, 630, 642, 643, 649, 650, 651, 652,
            653, 654, 655, 656, 657, 658, 659, 660, 661, 662, 663, 664, 665, 666, 667, 668, 669,
            670, 671, 672, 673, 674, 675, 676, 677, 678, 679, 680, 681, 682, 683, 684, 685, 686,
            687, 688, 689, 690, 691, 692, 693, 694, 695, 696, 697, 698, 699, 700, 701, 702, 703,
            704, 705, 706, 707, 708, 709, 710, 711, 712, 713, 714, 715, 716, 717, 718, 719, 720,
            721, 722, 723, 724, 725, 726, 727, 728, 729, 730, 731, 732, 733, 734, 739, 740, 741,
            742, 743, 744, 745, 746, 747, 748, 749, 750, 751, 752, 753, 754, 755, 756, 757, 758,
            759, 760, 761, 762, 763, 764, 765, 766, 767, 768, 769, 770, 771, 772, 773, 774, 775,
            776, 777, 778, 779, 780, 781, 782, 783, 784, 786, 787, 788, 789, 790, 791, 792, 793,
            808, 820, 831, 837, 838, 839, 840, 841, 844, 845, 846, 847, 848, 849, 850, 851, 852,
            853, 854, 855, 856, 862, 893, 894, 895, 896, 897, 898, 899, 900, 901, 902,
        ],
    };

    const BAD_TREE_NODE_4: &TestCase = &TestCase {
        bits: &[
            false, false, false, false, false, false, false, false, false, false, false, true,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, true, true, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, true, false, false, false, true, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            true, false, false, false, false, false, true, false, false, false, false, false,
            false, false, false, false, false, false, false, true, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, true, true, false, true, true, false, true,
            false, false, false, false, false, false, false, true, true, true, true, false, false,
            false, false, false, false, false, false, false, false, false, true, false, false,
            false, false, false, false, false, false, false, false, false, true, true, true, false,
            false, false, false, false, true, false, false, false, false, false, false, true,
            false, true, true, false, false, false, false, true, true, false, false, true, false,
            true, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, true, true, true, true, true, true, true, true, true, false, true,
            false, false, false, false, false, false, false, false, false, true, true, true, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, true, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, true, false, false, true, false, false, false, true, false, true, false, true,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, true, false, false, true, false, false,
            false, true, false, false, false, false, false, false, false, true, true, true, false,
            true, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, true,
            false, false, false, true, true, false, false, true, false, false, true, true, true,
            false, true, true, true, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            true, true, true, true, false, false, false, false, false, false, false, false, false,
            false, false, false, true, true, true, true, true, true, true, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, true, false, false, true, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, true, true, true, true, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, true, true, true, true, true, true, true, true, true, true, false,
            false, false, false, false, false, false, false, false, false, false, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true,
        ],
        ranks: &[
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
            1, 1, 1, 1, 1, 2, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
            3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
            3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
            3, 3, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
            5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 6, 6, 6, 6, 6, 6, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
            7, 7, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8,
            8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 9, 10, 10, 11, 12, 12, 13,
            13, 13, 13, 13, 13, 13, 13, 14, 15, 16, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17,
            18, 18, 18, 18, 18, 18, 18, 18, 18, 18, 18, 18, 19, 20, 21, 21, 21, 21, 21, 21, 22, 22,
            22, 22, 22, 22, 22, 23, 23, 24, 25, 25, 25, 25, 25, 26, 27, 27, 27, 28, 28, 29, 29, 29,
            29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29,
            29, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 38, 39, 39, 39, 39, 39, 39, 39, 39, 39, 39,
            40, 41, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42,
            42, 42, 42, 43, 43, 43, 43, 43, 43, 43, 43, 43, 43, 43, 43, 43, 43, 43, 43, 44, 44, 44,
            45, 45, 45, 45, 46, 46, 47, 47, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 49, 49, 49, 50, 50, 50, 50, 51, 51, 51, 51, 51, 51, 51, 51, 52, 53,
            54, 54, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55,
            55, 55, 55, 56, 56, 56, 56, 57, 58, 58, 58, 59, 59, 59, 60, 61, 62, 62, 63, 64, 65, 65,
            65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65,
            65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65,
            65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 66, 67, 68, 69, 69, 69, 69, 69, 69, 69,
            69, 69, 69, 69, 69, 69, 70, 71, 72, 73, 74, 75, 76, 76, 76, 76, 76, 76, 76, 76, 76, 76,
            76, 76, 76, 76, 76, 76, 76, 76, 76, 76, 76, 77, 77, 77, 78, 78, 78, 78, 78, 78, 78, 78,
            78, 78, 78, 78, 78, 78, 78, 78, 78, 78, 78, 78, 78, 78, 78, 78, 78, 78, 78, 78, 78, 78,
            78, 78, 78, 78, 78, 78, 78, 78, 78, 78, 78, 78, 78, 78, 78, 79, 80, 81, 82, 82, 82, 82,
            82, 82, 82, 82, 82, 82, 82, 82, 82, 82, 82, 82, 82, 82, 82, 82, 82, 82, 82, 82, 82, 82,
            82, 82, 82, 82, 82, 82, 82, 82, 82, 82, 82, 82, 82, 82, 82, 82, 82, 82, 83, 84, 85, 86,
            87, 88, 89, 90, 91, 92, 92, 92, 92, 92, 92, 92, 92, 92, 92, 92, 92, 93, 94, 95, 96, 97,
            98, 99, 100, 101, 102, 103, 104, 105, 106,
        ],
        ranks0: &[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
            23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 32, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42,
            43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64,
            65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86,
            87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106,
            107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 120, 121, 122,
            123, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138,
            139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 151, 152, 153, 154,
            155, 156, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 168, 169,
            170, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181, 182, 183, 184, 185, 186,
            187, 188, 189, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203,
            204, 205, 206, 207, 208, 209, 210, 211, 212, 213, 214, 214, 214, 215, 215, 215, 216,
            216, 217, 218, 219, 220, 221, 222, 223, 223, 223, 223, 223, 224, 225, 226, 227, 228,
            229, 230, 231, 232, 233, 234, 234, 235, 236, 237, 238, 239, 240, 241, 242, 243, 244,
            245, 245, 245, 245, 246, 247, 248, 249, 250, 250, 251, 252, 253, 254, 255, 256, 256,
            257, 257, 257, 258, 259, 260, 261, 261, 261, 262, 263, 263, 264, 264, 265, 266, 267,
            268, 269, 270, 271, 272, 273, 274, 275, 276, 277, 278, 279, 280, 281, 282, 283, 284,
            285, 286, 287, 288, 289, 290, 290, 290, 290, 290, 290, 290, 290, 290, 290, 291, 291,
            292, 293, 294, 295, 296, 297, 298, 299, 300, 300, 300, 300, 301, 302, 303, 304, 305,
            306, 307, 308, 309, 310, 311, 312, 313, 314, 315, 316, 317, 318, 319, 320, 321, 322,
            322, 323, 324, 325, 326, 327, 328, 329, 330, 331, 332, 333, 334, 335, 336, 337, 337,
            338, 339, 339, 340, 341, 342, 342, 343, 343, 344, 344, 345, 346, 347, 348, 349, 350,
            351, 352, 353, 354, 355, 356, 357, 358, 359, 360, 361, 362, 362, 363, 364, 364, 365,
            366, 367, 367, 368, 369, 370, 371, 372, 373, 374, 374, 374, 374, 375, 375, 376, 377,
            378, 379, 380, 381, 382, 383, 384, 385, 386, 387, 388, 389, 390, 391, 392, 393, 394,
            395, 396, 397, 397, 398, 399, 400, 400, 400, 401, 402, 402, 403, 404, 404, 404, 404,
            405, 405, 405, 405, 406, 407, 408, 409, 410, 411, 412, 413, 414, 415, 416, 417, 418,
            419, 420, 421, 422, 423, 424, 425, 426, 427, 428, 429, 430, 431, 432, 433, 434, 435,
            436, 437, 438, 439, 440, 441, 442, 443, 444, 445, 446, 447, 448, 449, 450, 451, 452,
            453, 454, 455, 456, 457, 458, 459, 460, 461, 462, 462, 462, 462, 462, 463, 464, 465,
            466, 467, 468, 469, 470, 471, 472, 473, 474, 474, 474, 474, 474, 474, 474, 474, 475,
            476, 477, 478, 479, 480, 481, 482, 483, 484, 485, 486, 487, 488, 489, 490, 491, 492,
            493, 494, 494, 495, 496, 496, 497, 498, 499, 500, 501, 502, 503, 504, 505, 506, 507,
            508, 509, 510, 511, 512, 513, 514, 515, 516, 517, 518, 519, 520, 521, 522, 523, 524,
            525, 526, 527, 528, 529, 530, 531, 532, 533, 534, 535, 536, 537, 538, 539, 540, 540,
            540, 540, 540, 541, 542, 543, 544, 545, 546, 547, 548, 549, 550, 551, 552, 553, 554,
            555, 556, 557, 558, 559, 560, 561, 562, 563, 564, 565, 566, 567, 568, 569, 570, 571,
            572, 573, 574, 575, 576, 577, 578, 579, 580, 581, 582, 583, 583, 583, 583, 583, 583,
            583, 583, 583, 583, 583, 584, 585, 586, 587, 588, 589, 590, 591, 592, 593, 594, 594,
            594, 594, 594, 594, 594, 594, 594, 594, 594, 594, 594, 594, 594,
        ],
        selects: &[
            0, 12, 34, 35, 124, 128, 157, 163, 176, 223, 224, 226, 227, 229, 237, 238, 239, 240,
            252, 264, 265, 266, 272, 279, 281, 282, 287, 288, 291, 293, 320, 321, 322, 323, 324,
            325, 326, 327, 328, 330, 340, 341, 342, 365, 381, 384, 388, 390, 392, 411, 414, 418,
            426, 427, 428, 430, 453, 457, 458, 461, 464, 465, 466, 468, 469, 470, 528, 529, 530,
            531, 544, 545, 546, 547, 548, 549, 550, 571, 574, 619, 620, 621, 622, 666, 667, 668,
            669, 670, 671, 672, 673, 674, 675, 687, 688, 689, 690, 691, 692, 693, 694, 695, 696,
            697, 698, 699, 700,
        ],
        selects0: &[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
            25, 26, 27, 28, 29, 30, 31, 32, 33, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48,
            49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70,
            71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92,
            93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111,
            112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 125, 126, 127, 129, 130,
            131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147,
            148, 149, 150, 151, 152, 153, 154, 155, 156, 158, 159, 160, 161, 162, 164, 165, 166,
            167, 168, 169, 170, 171, 172, 173, 174, 175, 177, 178, 179, 180, 181, 182, 183, 184,
            185, 186, 187, 188, 189, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199, 200, 201,
            202, 203, 204, 205, 206, 207, 208, 209, 210, 211, 212, 213, 214, 215, 216, 217, 218,
            219, 220, 221, 222, 225, 228, 230, 231, 232, 233, 234, 235, 236, 241, 242, 243, 244,
            245, 246, 247, 248, 249, 250, 251, 253, 254, 255, 256, 257, 258, 259, 260, 261, 262,
            263, 267, 268, 269, 270, 271, 273, 274, 275, 276, 277, 278, 280, 283, 284, 285, 286,
            289, 290, 292, 294, 295, 296, 297, 298, 299, 300, 301, 302, 303, 304, 305, 306, 307,
            308, 309, 310, 311, 312, 313, 314, 315, 316, 317, 318, 319, 329, 331, 332, 333, 334,
            335, 336, 337, 338, 339, 343, 344, 345, 346, 347, 348, 349, 350, 351, 352, 353, 354,
            355, 356, 357, 358, 359, 360, 361, 362, 363, 364, 366, 367, 368, 369, 370, 371, 372,
            373, 374, 375, 376, 377, 378, 379, 380, 382, 383, 385, 386, 387, 389, 391, 393, 394,
            395, 396, 397, 398, 399, 400, 401, 402, 403, 404, 405, 406, 407, 408, 409, 410, 412,
            413, 415, 416, 417, 419, 420, 421, 422, 423, 424, 425, 429, 431, 432, 433, 434, 435,
            436, 437, 438, 439, 440, 441, 442, 443, 444, 445, 446, 447, 448, 449, 450, 451, 452,
            454, 455, 456, 459, 460, 462, 463, 467, 471, 472, 473, 474, 475, 476, 477, 478, 479,
            480, 481, 482, 483, 484, 485, 486, 487, 488, 489, 490, 491, 492, 493, 494, 495, 496,
            497, 498, 499, 500, 501, 502, 503, 504, 505, 506, 507, 508, 509, 510, 511, 512, 513,
            514, 515, 516, 517, 518, 519, 520, 521, 522, 523, 524, 525, 526, 527, 532, 533, 534,
            535, 536, 537, 538, 539, 540, 541, 542, 543, 551, 552, 553, 554, 555, 556, 557, 558,
            559, 560, 561, 562, 563, 564, 565, 566, 567, 568, 569, 570, 572, 573, 575, 576, 577,
            578, 579, 580, 581, 582, 583, 584, 585, 586, 587, 588, 589, 590, 591, 592, 593, 594,
            595, 596, 597, 598, 599, 600, 601, 602, 603, 604, 605, 606, 607, 608, 609, 610, 611,
            612, 613, 614, 615, 616, 617, 618, 623, 624, 625, 626, 627, 628, 629, 630, 631, 632,
            633, 634, 635, 636, 637, 638, 639, 640, 641, 642, 643, 644, 645, 646, 647, 648, 649,
            650, 651, 652, 653, 654, 655, 656, 657, 658, 659, 660, 661, 662, 663, 664, 665, 676,
            677, 678, 679, 680, 681, 682, 683, 684, 685, 686,
        ],
    };

    const BAD_TREE_NODE_5: &TestCase = &TestCase {
        bits: &[
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false, false, false, true, true, true,
            true, true, true, true, true, true, true, true, true,
        ],
        ranks: &[
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
        ],
        ranks0: &[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45,
            46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67,
            68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89,
            90, 91, 92, 93, 94, 94, 94, 94, 94, 94, 94, 94, 94, 94, 94, 94, 94,
        ],
        selects: &[0, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106],
        selects0: &[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45,
            46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67,
            68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89,
            90, 91, 92, 93, 94,
        ],
    };

    const BAD_TREE_NODE_6: &TestCase = &TestCase {
        bits: &[
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, false, true, true, true, false, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, false, true, true, true, true, true,
        ],
        ranks: &[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45,
            45, 46, 47, 48, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65,
            66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 86,
            87, 88, 89, 90, 91,
        ],
        ranks0: &[
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2,
            2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
            2, 2, 3, 3, 3, 3, 3,
        ],
        selects: &[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45,
            47, 48, 49, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69,
            70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 90, 91, 92,
            93, 94,
        ],
        selects0: &[0, 46, 50, 89],
    };

    macro_rules! test_cases {
        () => {
            #[test]
            fn empty() {
                run(super::super::EMPTY);
            }

            #[test]
            fn one_bit_false() {
                run(super::super::ONE_BIT_FALSE);
            }

            #[test]
            fn one_bit_true() {
                run(super::super::ONE_BIT_TRUE);
            }

            #[test]
            fn evens() {
                run(super::super::EVENS);
            }

            #[test]
            fn odds() {
                run(super::super::ODDS);
            }

            #[test]
            fn half_empty() {
                run(super::super::HALF_EMPTY);
            }

            #[test]
            fn bad_tree_node_1() {
                run(super::super::BAD_TREE_NODE_1);
            }

            #[test]
            fn bad_tree_node_2() {
                run(super::super::BAD_TREE_NODE_2);
            }

            #[test]
            fn bad_tree_node_3() {
                run(super::super::BAD_TREE_NODE_3);
            }

            #[test]
            fn bad_tree_node_4() {
                run(super::super::BAD_TREE_NODE_4);
            }

            #[test]
            fn bad_tree_node_5() {
                run(super::super::BAD_TREE_NODE_5);
            }

            #[test]
            fn bad_tree_node_6() {
                run(super::super::BAD_TREE_NODE_6);
            }
        };
    }

    macro_rules! test_BitVector {
        ($name:ident, $bv:path) => {
            mod $name {
                use crate::bit_vector::BitVector;
                use crate::bit_vector::ReferenceBitVector;
                use crate::builder::Builder;

                use super::TestCase;

                fn run<F: Fn(&TestCase, &$bv)>(t: &TestCase, f: F) {
                    let mut buf = vec![];
                    let mut builder = Builder::new(&mut buf);
                    <$bv as BitVector>::construct(t.bits, &mut builder).unwrap();
                    drop(builder);
                    let bv = <$bv>::parse(&buf).unwrap().0;
                    f(t, &bv);
                }

                mod access {
                    use super::TestCase;

                    fn run(t: &TestCase) {
                        super::run(t, |t, bv| TestCase::access(t, bv));
                    }

                    test_cases!();
                }

                mod rank {
                    use super::TestCase;

                    fn run(t: &TestCase) {
                        super::run(t, |t, bv| TestCase::rank(t, bv));
                    }

                    test_cases!();
                }

                mod rank0 {
                    use super::TestCase;

                    fn run(t: &TestCase) {
                        super::run(t, |t, bv| TestCase::rank0(t, bv));
                    }

                    test_cases!();
                }

                mod select {
                    use super::TestCase;

                    fn run(t: &TestCase) {
                        super::run(t, |t, bv| TestCase::select(t, bv));
                    }

                    test_cases!();
                }

                mod select0 {
                    use super::TestCase;

                    fn run(t: &TestCase) {
                        super::run(t, |t, bv| TestCase::select0(t, bv));
                    }

                    test_cases!();
                }

                proptest::prop_compose! {
                    pub fn arb_bit_vector()(bv in proptest::collection::vec(proptest::arbitrary::any::<bool>(), 0..630)) -> Vec<bool> {
                        bv
                    }
                }

                proptest::proptest! {
                    #[test]
                    fn properties(bv in arb_bit_vector()) {
                        let mut exp_buf = vec![];
                        let mut exp_builder = Builder::new(&mut exp_buf);
                        ReferenceBitVector::construct(&bv, &mut exp_builder).expect("bit vector should construct");
                        drop(exp_builder);
                        let exp = ReferenceBitVector::parse(exp_buf.as_slice()).unwrap().0;

                        let mut got_buf = vec![];
                        let mut got_builder = Builder::new(&mut got_buf);
                        <$bv as BitVector>::construct(&bv, &mut got_builder)
                            .expect("bit vector should construct");
                        drop(got_builder);
                        let got = <$bv>::parse(got_buf.as_slice()).unwrap().0;

                        assert_eq!(exp.len(), got.len());
                        assert_eq!(exp.is_empty(), got.is_empty());
                        for i in 0..=exp.len() {
                            assert_eq!(exp.access(i), got.access(i));
                            assert_eq!(exp.rank(i), got.rank(i));
                            assert_eq!(exp.rank0(i), got.rank0(i));
                            assert_eq!(exp.select(i), got.select(i));
                            if let Some(selected) = exp.select(i) {
                                assert_eq!(Some(i), exp.rank(selected));
                            }
                        }
                    }
                }
            }
        };
    }

    test_BitVector!(reference, ReferenceBitVector);
    test_BitVector!(rrr, super::super::rrr::BitVector);
    test_BitVector!(sparse, super::super::sparse::BitVector);
}
