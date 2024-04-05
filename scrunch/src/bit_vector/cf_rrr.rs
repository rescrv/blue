use buffertk::Unpackable;

use crate::bit_array::{BitArray, Builder as BitArrayBuilder, FixedWidthIterator};
use crate::builder::{Builder, Helper};
use crate::Error;

use super::rrr::{decode, encode, u63, SixtyThreeBitWords, L};
use super::BitVector as BitVectorTrait;

/////////////////////////////////////////// BitVectorStub //////////////////////////////////////////

#[derive(Clone, Debug, Default, prototk_derive::Message)]
struct BitVectorStub<'a> {
    #[prototk(1, uint64)]
    bits: u64,
    #[prototk(2, uint32)]
    words_per_block: u32,
    #[prototk(3, uint32)]
    select_sample: u32,
    #[prototk(4, uint64)]
    p_width: u64,
    #[prototk(5, uint64)]
    r_width: u64,
    #[prototk(6, bytes)]
    p: &'a [u8],
    #[prototk(7, bytes)]
    b: &'a [u8],
    #[prototk(8, bytes)]
    s0: &'a [u8],
    #[prototk(9, bytes)]
    s1: &'a [u8],
}

impl<'a> BitVectorStub<'a> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        bits: u64,
        words_per_block: u32,
        select_sample: u32,
        p_width: u64,
        r_width: u64,
        p: &'a [u8],
        b: &'a [u8],
        s0: &'a [u8],
        s1: &'a [u8],
    ) -> Self {
        Self {
            bits,
            words_per_block,
            select_sample,
            p_width,
            r_width,
            p,
            b,
            s0,
            s1,
        }
    }
}

impl<'a> TryFrom<BitVectorStub<'a>> for BitVector<'a> {
    type Error = Error;

    fn try_from(bvs: BitVectorStub<'a>) -> Result<Self, Self::Error> {
        Ok(BitVector {
            bits: bvs.bits.try_into()?,
            words_per_block: bvs.words_per_block,
            select_sample: bvs.select_sample,
            p_width: bvs.p_width.try_into()?,
            r_width: bvs.r_width.try_into()?,
            p: BitArray::new(bvs.p),
            b: BitArray::new(bvs.b),
            s0: BitArray::new(bvs.s0),
            s1: BitArray::new(bvs.s1),
        })
    }
}

///////////////////////////////////////////// BitVector ////////////////////////////////////////////

pub struct BitVector<'a> {
    bits: usize,
    words_per_block: u32,
    select_sample: u32,
    p_width: usize,
    r_width: usize,
    p: BitArray<'a>,
    b: BitArray<'a>,
    s0: BitArray<'a>,
    s1: BitArray<'a>,
}

impl<'a> BitVector<'a> {
    fn calc_width(bits: usize) -> Option<usize> {
        let next_power_of_two = (bits + 1).next_power_of_two();
        if next_power_of_two > 0 {
            Some(std::cmp::max(next_power_of_two.ilog2() as usize + 1, 8))
        } else {
            None
        }
    }

    pub fn access_rank(&self, mut index: usize) -> Option<(bool, usize)> {
        if index > self.len() {
            return None;
        }
        if index == 0 && self.len() == 0 {
            return Some((false, 0));
        }
        // The offset into P.
        let stride = (self.words_per_block * 63) as usize;
        let p_offset = index / stride;
        // The offset into B.
        let b_offset: usize = self
            .p
            .load(p_offset * self.p_width, self.p_width)?
            .try_into()
            .ok()?;
        // Adjust index to account for our jump through P.
        index -= p_offset * stride;
        assert!(index < stride);
        // Now iterate through c.
        let mut rank: usize = self.b.load(b_offset, self.r_width)?.try_into().ok()?;
        let mut iter_c = FixedWidthIterator::new(
            self.b.as_ref(),
            b_offset + self.r_width,
            self.words_per_block as usize * 6,
            6,
        );
        let mut o_rel = 0;
        while index >= 63 {
            let c: usize = iter_c.next()?.try_into().ok()?;
            o_rel += L[c];
            index -= 63;
            rank += c;
        }
        let c: usize = iter_c.next()?.try_into().ok()?;
        let w = self.b.load(
            b_offset + self.r_width + self.words_per_block as usize * 6 + o_rel,
            L[c],
        )?;
        let w = decode(w, c)?;
        rank += (w.0 & ((1u64 << index) - 1)).count_ones() as usize;
        let access = w.0 & (1 << index) != 0;
        Some((access, rank))
    }

    fn select_helper(
        &self,
        x: usize,
        structure: &BitArray<'_>,
        load_rank: impl Fn(usize, usize) -> Option<usize>,
        add_rank: impl Fn(usize) -> usize,
        word_select: impl Fn(&u63, usize) -> Option<usize>,
    ) -> Option<usize> {
        if x == 0 {
            return Some(0);
        }
        let stride = (self.words_per_block * 63) as usize;
        let index_into_structure: usize = (x / stride) * self.r_width;
        let index_into_p: usize = structure
            .load(index_into_structure, self.r_width)?
            .try_into()
            .ok()?;
        for idx in 0..usize::MAX {
            let index_of_block: usize = self
                .p
                .load((index_into_p + idx) * self.p_width, self.p_width)?
                .try_into()
                .ok()?;
            let mut o_rel = 0;
            let mut rank: usize = load_rank(index_into_p + idx, index_of_block)?;
            assert!(rank <= x);
            let mut index: usize = (index_into_p + idx) * self.words_per_block as usize * 63;
            let iter_c = FixedWidthIterator::new(
                self.b.as_ref(),
                index_of_block + self.r_width,
                self.words_per_block as usize * 6,
                6,
            );
            let mut itered = false;
            for c in iter_c {
                itered = true;
                let c: usize = c.try_into().ok()?;
                let r: usize = add_rank(c);
                if rank + r >= x {
                    let o = self.b.load(
                        index_of_block + self.r_width + self.words_per_block as usize * 6 + o_rel,
                        L[c],
                    )?;
                    let w = decode(o, c)?;
                    let answer = index + word_select(&w, x - rank)?;
                    return if answer <= self.len() {
                        Some(answer)
                    } else {
                        None
                    };
                }
                rank += r;
                o_rel += L[c];
                index += 63;
            }
            if !itered {
                break;
            }
        }
        None
    }
}

impl<'a> super::BitVector for BitVector<'a> {
    type Output<'b> = BitVector<'b>;

    fn construct<H: Helper>(bits: &[bool], builder: &mut Builder<'_, H>) -> Result<(), Error> {
        const PARAM_WORDS_PER_BLOCK: usize = 23;
        const PARAM_SELECT_SAMPLE: usize = PARAM_WORDS_PER_BLOCK * 63;
        let r_width = Self::calc_width(bits.len()).ok_or(Error::IntoUsize)?;
        let words = SixtyThreeBitWords::new(bits).collect::<Vec<_>>();
        let mut idx = 0;
        let mut rank = 0u64;
        let mut rank0 = 0u64;
        let mut build = BitArrayBuilder::with_capacity(bits.len());
        let mut build_s0 = BitArrayBuilder::with_capacity(bits.len());
        let mut build_s1 = BitArrayBuilder::with_capacity(bits.len());
        let mut ps = vec![];
        let mut next_select0 = 0u64;
        let mut next_select1 = 0u64;
        while idx < words.len() {
            ps.push(build.len());
            assert!(next_select1 >= rank);
            build.push_word(rank, r_width);
            let amt = if idx + PARAM_WORDS_PER_BLOCK <= words.len() {
                PARAM_WORDS_PER_BLOCK
            } else {
                words.len() - idx
            };
            let o_c: Vec<(u64, usize)> = words[idx..idx + amt]
                .iter()
                .copied()
                .map(encode)
                .collect::<Vec<_>>();
            for (_, c) in o_c.iter() {
                assert!(*c <= 63);
                rank += *c as u64;
                rank0 += 63 - *c as u64;
                build.push_word(*c as u64, 6);
            }
            for _ in 0..PARAM_WORDS_PER_BLOCK - amt {
                build.push_word(0u64, 6);
            }
            for (o, c) in o_c.iter() {
                let l_c = L[*c];
                if l_c > 0 {
                    build.push_word(*o, l_c);
                }
            }
            while rank0 >= next_select0 {
                build_s0.push_word((ps.len() - 1) as u64, r_width);
                next_select0 += PARAM_SELECT_SAMPLE as u64;
            }
            while rank >= next_select1 {
                build_s1.push_word((ps.len() - 1) as u64, r_width);
                next_select1 += PARAM_SELECT_SAMPLE as u64;
            }
            idx += amt;
        }
        if ps.is_empty() {
            ps.push(0);
        }
        let p_width = Self::calc_width(ps[ps.len() - 1] + 1).ok_or(Error::IntoUsize)?;
        let mut build_ps = BitArrayBuilder::with_capacity(p_width * ps.len());
        for ps in ps.into_iter() {
            build_ps.push_word(ps as u64, p_width);
        }
        let buf_blocks = build.seal();
        let buf_ps = build_ps.seal();
        let buf_s0 = build_s0.seal();
        let buf_s1 = build_s1.seal();
        builder.append_raw_packable(&BitVectorStub::new(
            bits.len() as u64,
            PARAM_WORDS_PER_BLOCK
                .try_into()
                .expect("validated parameter should always fit u32"),
            PARAM_SELECT_SAMPLE
                .try_into()
                .expect("validated parameter should always fit u32"),
            p_width as u64,
            r_width as u64,
            &buf_ps,
            &buf_blocks,
            &buf_s0,
            &buf_s1,
        ));
        Ok(())
    }

    fn parse<'b, 'c: 'b>(buf: &'c [u8]) -> Result<(Self::Output<'b>, &'c [u8]), Error> {
        let (bvs, buf) =
            <BitVectorStub as Unpackable>::unpack(buf).map_err(|_| Error::InvalidBitVector)?;
        Ok((bvs.try_into()?, buf))
    }

    fn len(&self) -> usize {
        self.bits
    }

    fn access(&self, index: usize) -> Option<bool> {
        if index < self.len() {
            Some(self.access_rank(index)?.0)
        } else {
            None
        }
    }

    fn rank(&self, index: usize) -> Option<usize> {
        Some(self.access_rank(index)?.1)
    }

    fn select(&self, x: usize) -> Option<usize> {
        let width = self.r_width;
        let load_rank =
            |_: usize, ptr: usize| -> Option<usize> { self.b.load(ptr, width).map(|r| r as usize) };
        let add_rank = |c: usize| c;
        self.select_helper(x, &self.s1, load_rank, add_rank, |w, r| w.select1(r))
    }

    fn select0(&self, x: usize) -> Option<usize> {
        let width = self.r_width;
        let sample = self.select_sample as usize;
        let load_rank = |idx: usize, ptr: usize| -> Option<usize> {
            self.b.load(ptr, width).map(|r| idx * sample - r as usize)
        };
        let add_rank = |c: usize| 63 - c;
        self.select_helper(x, &self.s0, load_rank, add_rank, |w, r| w.select0(r))
    }
}
