use buffertk::Unpackable;
use prototk::FieldNumber;

use super::bit_array::{BitArray, Builder as BitArrayBuilder};
use super::bit_vector::cf_rrr::BitVector;
use super::bit_vector::BitVector as BitVectorTrait;
use super::bits_required;
use super::psi;
use super::sigma::Sigma;
use super::Builder;
use super::Error;
use super::Helper;

//////////////////////////////////////////// SuffixArray ///////////////////////////////////////////

pub trait SuffixArray {
    // TODO(rescrv): Make it an associated type to have parameters.
    fn construct<H: Helper>(
        sampling: usize,
        sa: &[usize],
        builder: &mut Builder<H>,
    ) -> Result<(), Error>;
    fn lookup<PSI: psi::Psi>(&self, sigma: &Sigma, psi: &PSI, idx: usize) -> Result<usize, Error>;
}

///////////////////////////////////// ReferenceSuffixArrayStub /////////////////////////////////////

#[derive(Clone, Debug, Default, prototk_derive::Message)]
pub struct ReferenceSuffixArrayStub {
    #[prototk(1, uint64)]
    sa: Vec<u64>,
}

impl From<&[usize]> for ReferenceSuffixArrayStub {
    fn from(sa: &[usize]) -> Self {
        let sa = sa.iter().map(|x| *x as u64).collect();
        Self { sa }
    }
}

impl From<ReferenceSuffixArray> for ReferenceSuffixArrayStub {
    fn from(rsa: ReferenceSuffixArray) -> Self {
        let sa: &[usize] = &rsa.sa;
        Self::from(sa)
    }
}

impl TryFrom<ReferenceSuffixArrayStub> for ReferenceSuffixArray {
    type Error = Error;

    fn try_from(rsas: ReferenceSuffixArrayStub) -> Result<Self, Self::Error> {
        let ReferenceSuffixArrayStub { sa } = rsas;
        if sa.iter().any(|x| *x > usize::MAX as u64) {
            return Err(Error::IntoUsize);
        }
        let sa = sa.into_iter().map(|x| x as usize).collect();
        Ok(ReferenceSuffixArray { sa })
    }
}

/////////////////////////////////////// ReferenceSuffixArray ///////////////////////////////////////

pub struct ReferenceSuffixArray {
    sa: Vec<usize>,
}

impl SuffixArray for ReferenceSuffixArray {
    fn construct<H: Helper>(_: usize, sa: &[usize], builder: &mut Builder<H>) -> Result<(), Error> {
        let stub = ReferenceSuffixArrayStub::from(sa);
        builder.append_raw_packable(&stub);
        Ok(())
    }

    fn lookup<PSI: psi::Psi>(&self, _: &Sigma, _: &PSI, idx: usize) -> Result<usize, Error> {
        self.sa.get(idx).copied().ok_or(Error::BadIndex(idx))
    }
}

impl<'a> Unpackable<'a> for ReferenceSuffixArray {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Self::Error> {
        let (rsa, buf) = <ReferenceSuffixArrayStub as Unpackable>::unpack(buf)
            .map_err(|_| Error::InvalidDocument)?;
        Ok((rsa.try_into()?, buf))
    }
}

////////////////////////////////////// SampledSuffixArrayStub //////////////////////////////////////

#[derive(Clone, Debug, Default, prototk_derive::Message)]
pub struct SampledSuffixArrayStub<'a> {
    #[prototk(1, uint32)]
    sampling: u32,
    #[prototk(2, uint64)]
    zero: u64,
    #[prototk(4, uint32)]
    bits: u32,
    #[prototk(5, bytes)]
    values: &'a [u8],
    #[prototk(6, bytes)]
    present: &'a [u8],
}

impl<'a> TryFrom<&'a SampledSuffixArrayStub<'a>> for SampledSuffixArray<'a> {
    type Error = Error;

    fn try_from(ssas: &'a SampledSuffixArrayStub) -> Result<Self, Self::Error> {
        let SampledSuffixArrayStub {
            sampling,
            zero,
            bits,
            values,
            present,
        } = ssas;
        if *bits > 64 {
            return Err(Error::InvalidSuffixArray);
        }
        let bits = *bits as u8;
        let values = BitArray::new(values);
        let present = BitVector::parse(present)?.0;
        Ok(SampledSuffixArray {
            sampling: *sampling,
            zero: *zero,
            bits,
            values,
            present,
        })
    }
}

//////////////////////////////////////// SampledSuffixArray ////////////////////////////////////////

pub struct SampledSuffixArray<'a> {
    sampling: u32,
    zero: u64,
    bits: u8,
    values: BitArray<'a>,
    present: BitVector<'a>,
}

impl<'a> SampledSuffixArray<'a> {
    fn value(&self, x: usize) -> Option<usize> {
        let (access, rank) = self.present.access_rank(x)?;
        if access {
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

impl<'a> SuffixArray for SampledSuffixArray<'a> {
    fn construct<H: Helper>(
        sampling: usize,
        sa: &[usize],
        builder: &mut Builder<H>,
    ) -> Result<(), Error> {
        if sampling > 63 {
            return Err(Error::InvalidSuffixArray);
        }
        let sample = |x: usize| -> Option<u64> {
            if x % (1 << sampling) == 0 {
                Some((x >> sampling) as u64)
            } else {
                None
            }
        };
        let max_value = sa
            .iter()
            .cloned()
            .filter_map(sample)
            .max()
            .unwrap_or_default();
        let bits = bits_required(max_value) as usize;
        let mut values = BitArrayBuilder::with_capacity(bits * sa.len());
        let mut present = vec![];
        for (idx, value) in sa
            .iter()
            .cloned()
            .enumerate()
            .filter_map(|(idx, v)| Some((idx, sample(v)?)))
        {
            while present.len() < idx {
                present.push(false);
            }
            present.push(true);
            values.push_word(value, bits);
        }
        builder.append_u32(FieldNumber::must(1), sampling as u32);
        builder.append_u64(FieldNumber::must(2), sa[0] as u64);
        builder.append_u32(FieldNumber::must(3), bits as u32);
        let values = values.seal();
        builder.append_bytes(FieldNumber::must(4), &values);
        BitVector::construct(&present, &mut builder.sub(FieldNumber::must(5)))?;
        Ok(())
    }

    fn lookup<PSI: psi::Psi>(
        &self,
        sigma: &Sigma,
        psi: &PSI,
        mut idx: usize,
    ) -> Result<usize, Error> {
        let mut k = 0usize;
        loop {
            if idx == 0 {
                return Ok(self.zero as usize - k);
            }
            if let Some(sa) = self.value(idx) {
                let sa = sa << self.sampling;
                return Ok(sa - k);
            }
            idx = psi.lookup(sigma, idx)?;
            k += 1;
        }
    }
}

impl<'a> std::fmt::Debug for SampledSuffixArray<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.debug_struct("SampledSuffixArray")
            .field("sampling", &self.sampling)
            .field("zero", &self.zero)
            .field("bits", &self.bits)
            .field("values", &self.values)
            .field("present", &self.present.len())
            .finish()
    }
}

impl<'a> Unpackable<'a> for SampledSuffixArray<'a> {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Self::Error> {
        let (stub, buf) =
            SampledSuffixArrayStub::unpack(buf).map_err(|_| Error::InvalidSuffixArray)?;
        if stub.bits > 64 {
            return Err(Error::InvalidSuffixArray);
        }
        let bits = stub.bits as u8;
        let values = BitArray::new(stub.values);
        let present = BitVector::parse(stub.present)?.0;
        Ok((
            Self {
                sampling: stub.sampling,
                zero: stub.zero,
                bits,
                values,
                present,
            },
            buf,
        ))
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use crate::psi::ReferencePsi;
    use crate::test_cases_for;
    use crate::test_util::TestCase;

    use super::*;

    fn check_sampled(t: &TestCase) {
        let sigma = t.sigma();
        let sigma = Sigma::unpack(&sigma).expect("test should unpack").0;
        let psi = ReferencePsi::new(t.PSI);
        let mut buf = vec![];
        let mut builder = Builder::new(&mut buf);
        SampledSuffixArray::construct(2, t.SA, &mut builder).expect("should construct");
        drop(builder);
        let sa = SampledSuffixArray::unpack(&buf).expect("should parse").0;
        for (idx, exp) in t.SA.iter().enumerate() {
            assert_eq!(*exp, sa.lookup(&sigma, &psi, idx).expect("should succeed"));
        }
    }

    test_cases_for! {sampled, super::check_sampled}
}
