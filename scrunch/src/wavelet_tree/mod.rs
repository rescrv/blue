use buffertk::Unpackable;

use crate::builder::{Builder, Helper};
use crate::Error;

pub mod prefix;

//////////////////////////////////////////// WaveletTree ///////////////////////////////////////////

pub trait WaveletTree {
    /// Construct in the builder a byte-wise representation of the wavelet tree.
    fn construct<H: Helper>(
        text: &[u32],
        builder: &mut Builder<'_, H>,
    ) -> Result<(), Error>;

    /// The length of this [WaveletTree].  Always the number of symbols.
    fn len(&self) -> usize;
    /// A [WaveletTree] `is_empty` when it has zero symbols.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Computes `access[x]`, the value of the x'th symbol.
    fn access(&self, x: usize) -> Option<u32>;
    /// Computes `rank_q[x]`, the number of symbols q to the left of index x.
    fn rank_q(&self, q: u32, x: usize) -> Option<usize>;
    /// Computes `select_q[x]`, the offset of the x'th symbol q.
    fn select_q(&self, q: u32, x: usize) -> Option<usize>;
}

///////////////////////////////////// ReferenceWaveletTreeStub /////////////////////////////////////

#[derive(Debug, Default, prototk_derive::Message)]
pub struct ReferenceWaveletTreeStub {
    #[prototk(1, fixed32)]
    text: Vec<u32>,
}

/////////////////////////////////////// ReferenceWaveletTree ///////////////////////////////////////

pub struct ReferenceWaveletTree {
    text: Vec<u32>,
}

impl WaveletTree for ReferenceWaveletTree {
    fn construct<H: Helper>(
        text: &[u32],
        builder: &mut Builder<'_, H>,
    ) -> Result<(), Error> {
        let this = ReferenceWaveletTreeStub {
            text: text.to_vec(),
        };
        builder.append_raw_packable(&this);
        Ok(())
    }

    fn len(&self) -> usize {
        self.text.len()
    }

    fn access(&self, x: usize) -> Option<u32> {
        self.text.get(x).copied()
    }

    fn rank_q(&self, q: u32, x: usize) -> Option<usize> {
        let mut rank: usize = 0;
        for i in 0..self.text.len() {
            if i == x {
                return Some(rank);
            }
            if self.text[i] == q {
                rank += 1;
            }
        }
        if x == self.text.len() {
            Some(rank)
        } else {
            None
        }
    }

    fn select_q(&self, q: u32, x: usize) -> Option<usize> {
        let mut rank: usize = 0;
        for i in 0..self.text.len() {
            if rank == x {
                return Some(i);
            }
            if self.text[i] == q {
                rank += 1;
            }
        }
        if rank == x {
            Some(self.text.len())
        } else {
            None
        }
    }
}

impl<'a> Unpackable<'a> for ReferenceWaveletTree {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Self::Error> {
        let (stub, buf) = ReferenceWaveletTreeStub::unpack(buf).map_err(|_| Error::InvalidWaveletTree)?;
        Ok((Self {
            text: stub.text,
        }, buf))
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
pub mod tests {
    use crate::builder::Builder;
    use crate::encoder::FixedWidthEncoder;

    use super::prefix::WaveletTree as PrefixWaveletTree;
    use super::*;

    pub fn simple_evens<'a, 'b: 'a, WT: WaveletTree + Unpackable<'a>>(buf: &'b mut Vec<u8>) {
        let mut builder = Builder::new(buf);
        // try 010101
        WT::construct(&[0, 1, 0, 1, 0, 1], &mut builder).unwrap();
        drop(builder);
        let wt = WT::unpack(buf).unwrap().0;

        assert_eq!(Some(0), wt.access(0));
        assert_eq!(Some(1), wt.access(1));
        assert_eq!(Some(0), wt.access(2));
        assert_eq!(Some(1), wt.access(3));
        assert_eq!(Some(0), wt.access(4));
        assert_eq!(Some(1), wt.access(5));

        assert_eq!(Some(0), wt.rank_q(1, 0));
        assert_eq!(Some(0), wt.rank_q(1, 1));
        assert_eq!(Some(1), wt.rank_q(1, 2));
        assert_eq!(Some(1), wt.rank_q(1, 3));
        assert_eq!(Some(2), wt.rank_q(1, 4));
        assert_eq!(Some(2), wt.rank_q(1, 5));
        assert_eq!(Some(3), wt.rank_q(1, 6));
        assert_eq!(None, wt.rank_q(1, 7));

        assert_eq!(Some(0), wt.rank_q(0, 0));
        assert_eq!(Some(1), wt.rank_q(0, 1));
        assert_eq!(Some(1), wt.rank_q(0, 2));
        assert_eq!(Some(2), wt.rank_q(0, 3));
        assert_eq!(Some(2), wt.rank_q(0, 4));
        assert_eq!(Some(3), wt.rank_q(0, 5));
        assert_eq!(Some(3), wt.rank_q(0, 6));
        assert_eq!(None, wt.rank_q(0, 7));

        assert_eq!(Some(0), wt.select_q(1, 0));
        assert_eq!(Some(2), wt.select_q(1, 1));
        assert_eq!(Some(4), wt.select_q(1, 2));
        assert_eq!(Some(6), wt.select_q(1, 3));
        assert_eq!(None, wt.select_q(1, 4));

        assert_eq!(Some(0), wt.select_q(0, 0));
        assert_eq!(Some(1), wt.select_q(0, 1));
        assert_eq!(Some(3), wt.select_q(0, 2));
        assert_eq!(Some(5), wt.select_q(0, 3));
        assert_eq!(None, wt.select_q(0, 4));
    }

    pub fn simple_odds<'a, 'b: 'a, WT: WaveletTree + Unpackable<'a>>(buf: &'b mut Vec<u8>) {
        let mut builder = Builder::new(buf);
        // try 010101
        WT::construct(&[1, 0, 1, 0, 1, 0], &mut builder).unwrap();
        drop(builder);
        let wt = WT::unpack(buf).unwrap().0;

        assert_eq!(Some(1), wt.access(0));
        assert_eq!(Some(0), wt.access(1));
        assert_eq!(Some(1), wt.access(2));
        assert_eq!(Some(0), wt.access(3));
        assert_eq!(Some(1), wt.access(4));
        assert_eq!(Some(0), wt.access(5));

        assert_eq!(Some(0), wt.rank_q(1, 0));
        assert_eq!(Some(1), wt.rank_q(1, 1));
        assert_eq!(Some(1), wt.rank_q(1, 2));
        assert_eq!(Some(2), wt.rank_q(1, 3));
        assert_eq!(Some(2), wt.rank_q(1, 4));
        assert_eq!(Some(3), wt.rank_q(1, 5));
        assert_eq!(Some(3), wt.rank_q(1, 6));
        assert_eq!(None, wt.rank_q(1, 7));

        assert_eq!(Some(0), wt.rank_q(0, 0));
        assert_eq!(Some(0), wt.rank_q(0, 1));
        assert_eq!(Some(1), wt.rank_q(0, 2));
        assert_eq!(Some(1), wt.rank_q(0, 3));
        assert_eq!(Some(2), wt.rank_q(0, 4));
        assert_eq!(Some(2), wt.rank_q(0, 5));
        assert_eq!(Some(3), wt.rank_q(0, 6));
        assert_eq!(None, wt.rank_q(0, 7));

        assert_eq!(Some(0), wt.select_q(1, 0));
        assert_eq!(Some(1), wt.select_q(1, 1));
        assert_eq!(Some(3), wt.select_q(1, 2));
        assert_eq!(Some(5), wt.select_q(1, 3));
        assert_eq!(None, wt.select_q(1, 4));

        assert_eq!(Some(0), wt.select_q(0, 0));
        assert_eq!(Some(2), wt.select_q(0, 1));
        assert_eq!(Some(4), wt.select_q(0, 2));
        assert_eq!(Some(6), wt.select_q(0, 3));
        assert_eq!(None, wt.select_q(0, 4));
    }

    pub fn bad_case_1<'a, 'b: 'a, WT: WaveletTree + Unpackable<'a>>(buf: &'b mut Vec<u8>) {
        const TEXT: &[u32] = &[
            32, 73, 73, 32, 32, 73, 73, 73, 73, 60, 73, 60, 73, 73, 73, 60, 60, 73, 73, 60, 60, 60,
            60, 60, 73, 73, 60, 73, 73, 73, 73, 60, 74, 73, 73, 73, 73, 73, 73, 73, 60, 73, 73, 73,
            61, 73, 73, 73, 73, 73, 73, 73, 73, 60, 32, 60, 73, 60, 73, 73, 60, 60, 60, 60, 73, 73,
            32, 60, 60, 73, 73, 60, 32, 73, 73, 73, 60, 60, 60, 73, 60, 60, 60, 60, 60, 60, 60, 73,
            61, 74, 73, 74, 73, 73, 60, 60, 73, 73, 60, 60, 32, 60, 60, 60, 60, 60, 60, 60, 60, 60,
            60, 60, 60, 60, 32, 60, 60, 60, 32, 32, 32, 60, 32, 60, 60, 60, 60, 60, 60, 60, 60, 32,
            60, 60, 60, 60, 60, 60, 60, 60, 32, 60, 73, 60, 60, 60, 60, 60, 60, 60, 60, 60, 32, 60,
            60, 60, 32, 32, 60, 60, 32, 60, 60, 60, 60, 60, 60, 60, 60, 32, 60, 60, 60, 32, 60, 32,
            32, 60, 60, 32, 32, 60, 32, 60, 60, 60, 32, 32, 60, 60, 60, 60, 32, 32, 60, 60, 60, 60,
            60, 32, 32, 32, 60, 32, 32, 60, 73, 73, 73, 73, 32, 73, 73, 73, 60, 73, 73, 60, 60, 32,
            73, 73, 60, 60, 32, 60, 60, 74, 73, 73, 73, 73, 73, 60, 73, 61, 73, 73, 60, 73, 61, 73,
            60, 74, 73, 60, 32, 60, 32, 60, 73, 73, 73, 73, 73, 73, 73, 73, 60, 73, 60, 60, 60, 60,
            60, 60, 73, 73, 73, 60, 73, 73, 60, 60, 32, 60, 60, 73, 60, 73, 73, 73, 73, 66, 73, 73,
            73, 61, 73, 73, 73, 73, 73, 73, 73, 60, 60, 32, 60, 60, 60, 60, 60, 60, 73, 32, 60, 73,
            73, 60, 61, 73, 73, 73, 60, 60, 73, 73, 74, 73, 60, 60, 73, 73, 73, 73, 73, 32, 73, 60,
            60, 61, 73, 60, 73, 73, 61, 73, 73, 73, 73, 74, 32, 60, 73, 73, 60, 60, 73, 60, 73, 73,
            60, 60, 73, 73, 73, 60, 73, 60, 73, 73, 60, 73, 73, 73, 73, 73, 60, 73, 73, 73, 73, 73,
            73, 60, 73, 32, 72, 60, 74, 73, 73, 60, 73, 73, 73, 73, 73, 73, 73, 73, 70, 73, 73, 73,
            73, 73, 60, 60, 73, 60, 73, 60, 60, 60, 60, 73, 73, 73, 60, 32, 32, 32, 73, 73, 73, 73,
            60, 60, 60, 60, 32, 73, 60, 60, 60, 60, 60, 32, 60, 32, 60, 60, 32, 60, 60, 32, 32, 60,
            60, 60, 60, 60, 60, 60, 60, 60, 60, 60, 60, 60, 60, 32, 73, 73, 73, 60, 60, 73, 73, 32,
            32, 73, 32, 73, 73, 61, 73, 73, 73, 73, 61, 66, 73, 73, 73, 73, 60, 60, 73, 73, 73, 60,
            73, 73, 60, 60, 60, 73, 74, 74, 73, 61, 32, 32, 73, 60, 74, 73, 73, 73, 73, 74, 32, 32,
            60, 73, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 74, 32, 60, 32, 60, 73, 32, 60, 32,
            32, 32, 60, 60, 32, 60, 73, 60, 74, 32, 32, 73, 73, 73, 73, 73, 73, 74, 73, 73, 73, 74,
            73, 73, 74, 73, 73, 73, 73, 60, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 60, 60,
            60, 60, 73, 60, 60, 60, 73, 60, 60, 32, 60, 73, 74, 60, 60, 60, 60, 32, 60, 60, 60, 60,
            60, 32, 32, 32, 32, 60, 32, 32, 60, 74, 73, 73, 74, 74, 73, 60, 60, 32, 60, 60, 60, 73,
            73, 73, 73, 73, 73, 73, 73, 73, 74, 73, 60, 66, 60, 60, 60, 60, 60, 74, 73, 73, 73, 73,
            73, 73, 32, 73, 73, 60, 73, 74, 74, 73, 73, 60, 73, 60, 73, 60, 73, 73, 73, 60, 73, 73,
            74, 74, 32, 60, 73, 60, 74, 73, 60, 73, 73, 74, 61, 60, 60, 60, 32, 73, 73, 60, 60, 73,
            60, 60, 60, 60, 60, 60, 60, 60, 60, 60, 60, 60, 32, 60, 60, 60, 60, 60, 32, 73, 60, 32,
            73, 73, 73, 73, 73, 73, 73, 73, 73, 60, 73, 73, 60, 73, 73, 73, 73, 73, 73, 73, 73, 73,
            73, 73, 73, 73, 73, 73, 60, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73,
            73, 73, 73, 45, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73,
            73, 73, 45, 73, 73, 73, 73, 73, 73, 73, 73, 32, 73, 61, 61, 61, 61, 61, 73, 74, 74, 74,
            74, 73, 74, 74, 74, 74, 74, 73, 73, 60, 74, 73, 73, 60, 60, 60, 60, 60, 60, 60, 60, 60,
            32, 32, 74, 74, 73, 73, 74, 73, 60, 73, 73, 61, 32, 60, 73, 32, 60, 73, 73, 60, 73, 60,
            60, 60, 73, 73, 73, 73, 73, 60, 60, 73, 73, 73, 32, 60, 60, 60, 60, 60, 60, 60, 60, 60,
            32, 73, 61, 60, 60, 32, 60, 60, 73, 74, 73, 60, 73, 60, 60, 32, 73, 60, 60, 60, 60, 73,
            73, 73, 73, 73, 73, 73, 60, 73, 60, 73, 73, 73, 73, 73, 73, 60, 73, 32, 73, 32, 60, 73,
            60, 60, 32, 73, 73, 73, 73, 60, 73, 60, 32, 74, 73, 73, 60, 60, 32, 74, 32, 60, 60, 60,
            46, 73, 73, 60, 73, 73, 73, 73, 73, 74, 60, 66, 73, 74, 73, 73, 60, 60, 73, 61, 61, 73,
            73, 73, 60, 60, 32, 73, 73, 60, 66, 60, 60, 60, 60, 60, 73, 73, 60, 60, 73, 73, 32, 73,
            60, 73, 60, 73, 73, 73, 32, 73, 60, 60, 32, 60, 73, 73, 60, 74, 73, 73, 60, 73, 32, 73,
            32, 32, 60, 46, 73, 73, 73, 32, 60, 73, 60, 60, 73, 73, 73, 73, 73, 74, 60, 60, 73, 73,
            60, 60, 73, 32, 60, 73, 73, 73, 73, 60, 60, 74, 32, 73, 74, 73, 73, 73, 74, 32, 60, 32,
            32, 32, 74, 60, 60, 45, 60, 32, 32, 60, 32, 73, 60, 60, 32, 60, 60, 60, 60, 73, 60, 60,
            60, 32, 60, 60, 60, 60, 60, 73, 73, 60, 60, 60, 60, 73, 32, 73, 32, 74, 60, 73, 32, 73,
            73, 60, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 60, 73,
            73, 73, 73, 73, 73, 73, 73, 73, 61, 74, 73, 32, 73, 73, 74, 32, 73, 73, 73, 73, 73, 60,
            73, 73, 60, 73, 73, 74, 60, 73, 60, 73, 61, 74, 74, 74, 66, 73, 73, 73, 73, 73, 73, 60,
            73, 73, 73, 73, 73, 73, 73, 73, 73, 74, 74, 74, 60, 60, 60, 60, 60, 60, 60, 61, 73, 32,
            60, 32, 60, 32, 60, 60, 60, 60, 60, 60, 60, 32, 60, 60, 32, 73, 32, 60, 60, 60, 32, 32,
            32, 32, 32, 32, 60, 60, 32, 32, 32, 60, 60, 32, 60, 60, 60, 60, 60, 73, 60, 60, 60, 61,
            60, 60, 32, 73, 73, 32, 32, 32, 73, 32, 73, 60, 73, 73, 73, 73, 60, 32, 60, 60, 60, 61,
            60, 60, 32, 60, 60, 73, 60, 60, 32, 60, 32, 32, 60, 60, 32, 60, 32, 32, 32, 60, 60, 32,
            32, 32, 60, 60, 32, 60, 60, 32, 32, 60, 32, 73, 73, 73, 61, 74, 32, 73, 74, 61, 73, 74,
            74, 73, 78, 78, 50, 73, 73, 78, 73, 73, 78, 61, 60, 73, 60, 60, 73, 73, 73, 60, 32, 73,
            73, 73, 61, 73, 60, 60, 32, 73, 60, 60, 73, 73, 73, 73, 73, 32, 73, 73, 73, 73, 73, 73,
            60, 60, 73, 32, 73, 32, 60, 73, 32, 60, 73, 78, 78, 50, 73, 73, 73, 73, 32, 73, 73, 32,
            73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 72, 73, 73, 73, 73, 72, 73, 73, 73, 73,
            73, 73, 74, 76, 74, 74, 74, 74, 74, 74, 73, 73, 73, 70, 70, 70, 70, 60, 70, 70, 70, 73,
            73, 73, 70, 70, 60, 78, 70, 61, 70, 72, 78, 70, 76, 60, 70, 70, 72, 70, 70, 70, 74, 60,
            72, 70, 74, 70, 72, 72, 60, 70, 61, 70, 70, 70, 70, 70, 70, 61, 60, 60, 76, 72, 76, 70,
            73, 60, 78, 73, 61, 60, 73, 60, 73, 60, 73, 73, 73, 73, 73, 78, 60, 70, 70, 70, 70, 70,
            70, 70, 70, 70, 70, 70, 72, 70, 70, 70, 70, 70, 70, 70, 70, 70, 70, 70, 72, 72, 70, 70,
            70, 70, 70, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37,
            37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37,
            37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37,
            37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 37, 65,
            65, 65, 65, 65, 65, 37, 78, 74, 73, 73, 74, 74, 74, 72, 72, 72, 72, 72, 72, 72, 72, 72,
            72, 72, 72, 72, 72, 72, 72, 72, 72, 72, 72, 72, 72, 72, 72, 72, 72, 72, 72, 72, 72, 72,
            72, 72, 72, 72, 72, 72, 72, 72, 72, 72, 72, 72, 72, 72, 72, 73, 73, 73, 76, 73, 73, 73,
            76, 73, 73, 76, 73, 76, 76, 76, 76, 73, 73, 73, 73, 73, 73, 76, 73, 76, 76, 73, 73, 73,
            76, 63, 73, 76, 76, 73, 73, 73, 76, 76, 73, 73, 73, 73, 73, 73, 59, 76, 49, 49, 49, 49,
            49, 49, 49, 49, 49, 49, 49, 73, 73, 73, 73, 73, 76, 31, 59, 59, 59, 59, 59, 73, 73, 59,
            62, 62, 62, 62, 62, 62, 59, 62, 59, 59, 59, 59, 59, 59, 59, 59, 59, 59, 59, 59, 59, 59,
            59, 1, 73, 73, 73, 60, 60, 60, 60, 71, 60, 73, 73, 73, 60, 60, 60, 73, 60, 59, 60, 59,
            70, 59, 70, 59, 70, 70, 78, 60, 70, 61, 60, 70, 60, 60, 60, 70, 70, 60, 70, 78, 70, 61,
            70, 60, 70, 70, 61, 59, 59, 61, 59, 60, 60, 70, 61, 60, 60, 73, 73, 73, 73, 2, 2, 2,
            73, 73, 63, 63, 63, 73, 73, 61, 73, 73, 73, 61, 73, 72, 45, 73, 73, 73, 73, 73, 73, 73,
            73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 45, 73, 73, 73, 73, 73, 73, 73,
            73, 73, 73, 73, 60, 60, 61, 60, 60, 68, 60, 2, 66, 66, 1, 66, 66, 71, 71, 2, 70, 70,
            71, 2, 71, 66, 66, 2, 65, 71, 71, 2, 2, 2, 2, 1, 66, 66, 66, 66, 66, 66, 66, 66, 66,
            66, 74, 74, 74, 74, 74, 66, 74, 46, 74, 74, 74, 68, 61, 60, 60, 60, 60, 60, 60, 60, 60,
            60, 60, 60, 74, 74, 59, 59, 59, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 73, 73,
            59, 59, 62, 62, 62, 59, 62, 62, 62, 62, 59, 62, 59, 62, 59, 59, 62, 59,
        ];
        const CHARS: &[u32] = &[
            1, 2, 31, 32, 37, 45, 46, 49, 50, 59, 60, 61, 62, 63, 64, 65, 66, 68, 70, 71, 72, 73,
            74, 76, 78,
        ];
        let expected = ReferenceWaveletTree {
            text: TEXT.to_vec(),
        };
        let mut builder = Builder::new(buf);
        WT::construct(TEXT, &mut builder).unwrap();
        drop(builder);
        let wt = WT::unpack(buf).unwrap().0;
        for (i, t) in TEXT.iter().enumerate() {
            assert_eq!(Some(*t), expected.access(i), "i = {}", i);
            assert_eq!(Some(*t), wt.access(i), "i = {}", i);
        }
        for c in CHARS.iter() {
            for i in 0..TEXT.len() {
                assert_eq!(expected.rank_q(*c, i), wt.rank_q(*c, i));
                assert_eq!(expected.select_q(*c, i), wt.select_q(*c, i));
            }
        }
    }

    macro_rules! test_WaveletTree {
        ($name:ident, $WT:path) => {
            mod $name {
                use buffertk::Unpackable;

                use $crate::builder::Builder;
                use $crate::wavelet_tree::ReferenceWaveletTree;
                use $crate::wavelet_tree::WaveletTree;

                #[test]
                fn simple_evens() {
                    let mut buf = vec![];
                    $crate::wavelet_tree::tests::simple_evens::<$WT>(&mut buf);
                }

                #[test]
                fn simple_odds() {
                    let mut buf = vec![];
                    $crate::wavelet_tree::tests::simple_odds::<$WT>(&mut buf);
                }

                #[test]
                fn bad_case_1() {
                    let mut buf = vec![];
                    $crate::wavelet_tree::tests::bad_case_1::<$WT>(&mut buf);
                }

                proptest::prop_compose! {
                    pub fn arb_wavelet_tree()(bv in proptest::collection::vec(1u32..=16u32, 0..64)) -> Vec<u32> {
                        bv
                    }
                }

                proptest::proptest! {
                    #[test]
                    fn properties(bv in arb_wavelet_tree()) {
                        let mut exp_buf = vec![];
                        let mut exp_builder = Builder::new(&mut exp_buf);
                        ReferenceWaveletTree::construct(&bv, &mut exp_builder).unwrap();
                        drop(exp_builder);
                        let exp = ReferenceWaveletTree::unpack(exp_buf.as_slice()).unwrap().0;

                        let mut got_buf = vec![];
                        let mut got_builder = Builder::new(&mut got_buf);
                        <$WT as WaveletTree>::construct(&bv, &mut got_builder).unwrap();
                        drop(got_builder);
                        let got = <$WT as Unpackable>::unpack(got_buf.as_slice()).unwrap().0;

                        assert_eq!(exp.len(), got.len());
                        assert_eq!(exp.is_empty(), got.is_empty());
                        for x in 0..=bv.len() {
                            assert_eq!(exp.access(x), got.access(x));
                            for q in bv.iter() {
                                assert_eq!(exp.rank_q(*q, x), got.rank_q(*q, x));
                                assert_eq!(exp.select_q(*q, x), got.select_q(*q, x));
                            }
                        }
                    }
                }
            }
        };
    }

    test_WaveletTree!(reference, super::ReferenceWaveletTree);
    test_WaveletTree!(
        prefix_fixed,
        super::PrefixWaveletTree<super::FixedWidthEncoder>
    );
}
