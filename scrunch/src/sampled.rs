use buffertk::Unpackable;
use prototk::FieldNumber;

use crate::bit_array::{BitArray, Builder as BitArrayBuilder};
use crate::bit_vector::sparse::BitVector;
use crate::bit_vector::BitVector as BitVectorTrait;
use crate::builder::{Builder, Helper};
use crate::Error;

///////////////////////////////////////// SampledArrayStub /////////////////////////////////////////

#[derive(Clone, Debug, Default, prototk_derive::Message)]
pub struct SampledArrayStub<'a> {
    #[prototk(1, uint32)]
    bits: u32,
    #[prototk(2, bytes)]
    values: &'a [u8],
    #[prototk(3, bytes)]
    present: &'a [u8],
}

/////////////////////////////////////////// SampledArray ///////////////////////////////////////////

#[derive(Debug)]
pub struct SampledArray<'a> {
    bits: u8,
    values: BitArray<'a>,
    present: BitVector<'a>,
}

impl<'a> SampledArray<'a> {
    pub fn construct<H: Helper>(
        values: &[(usize, usize)],
        builder: &mut Builder<H>,
    ) -> Result<(), Error> {
        fn bits_required(values: &[(usize, usize)]) -> u8 {
            let mut max = 1;
            for (_, val) in values.iter() {
                max = std::cmp::max(max, *val);
            }
            let max = max.next_power_of_two();
            let bits = max.ilog2() as u8;
            bits + 1
        }
        let bits = bits_required(values);
        let mut sparse = Vec::with_capacity(values.len());
        let mut bitwords = BitArrayBuilder::with_capacity(values.len());
        for (offset, value) in values.iter() {
            sparse.push(*offset);
            bitwords.push_word(*value as u64, bits as usize);
        }
        builder.append_u32(FieldNumber::must(1), bits as u32);
        BitVector::from_indices(
            128,
            values[values.len() - 1].0 + 1,
            &sparse,
            &mut builder.sub(FieldNumber::must(3)),
        );
        let bitwords = bitwords.seal();
        builder.append_bytes(FieldNumber::must(2), &bitwords);
        Ok(())
    }

    pub fn parse<'b, 'c: 'b>(buf: &'c [u8]) -> Result<(SampledArray<'b>, &'c [u8]), Error> {
        let (
            SampledArrayStub {
                bits,
                values,
                present,
            },
            buf,
        ) = SampledArrayStub::unpack(buf).map_err(|_| Error::InvalidBitVector)?;
        if bits > 64 {
            return Err(Error::InvalidSuffixArray);
        }
        let bits = bits as u8;
        let values = BitArray::new(values);
        let present = BitVector::parse(present)?.0;
        Ok((
            SampledArray {
                bits,
                values,
                present,
            },
            buf,
        ))
    }

    pub fn lookup(&self, x: usize) -> Option<usize> {
        // TODO(rescrv): access_rank.
        if self.present.access(x)? {
            let rank = self.present.rank(x)?;
            let bits = self.bits as usize;
            if let Some(v) = self.values.load(bits * rank, bits) {
                v.try_into().ok()
            } else {
                None
            }
        } else {
            None
        }
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    proptest::prop_compose! {
        pub fn arb_values()(values in proptest::collection::vec(0usize..1<<32usize, 1..1024)) -> Vec<usize> {
            values
        }
    }

    proptest::proptest! {
        #[test]
        fn properties(values in arb_values()) {
            for gap in 1..5 {
                let mut sampling = vec![];
                for (i, v) in values.iter().enumerate() {
                    sampling.push((i * gap, *v));
                }
                let mut buf = vec![];
                let mut builder = Builder::new(&mut buf);
                SampledArray::construct(&sampling, &mut builder).expect("sampled array should construct");
                drop(builder);
                let sampled = SampledArray::parse(&buf).expect("sampled array should parse").0;
                for (i, value) in values.iter().enumerate().take(sampling.len()) {
                    assert_eq!(Some(*value), sampled.lookup(i * gap));
                    for j in 1..gap {
                        assert_eq!(None, sampled.lookup(i * gap + j));
                    }
                }
            }
        }
    }
}
