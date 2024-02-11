use buffertk::Unpackable;
use prototk::FieldNumber;

use super::Builder;
use super::Helper;
use super::Error;
use super::psi;
use super::sampled::SampledArray;
use super::sigma::Sigma;

//////////////////////////////////////////// SuffixArray ///////////////////////////////////////////

pub trait SuffixArray {
    // TODO(rescrv): Make it an associated type to have parameters.
    fn construct<H: Helper>(sampling: usize, sa: &[usize], builder: &mut Builder<H>) -> Result<(), Error>;
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
    #[prototk(3, bytes)]
    sampled: &'a [u8],
}

impl<'a> TryFrom<&'a SampledSuffixArrayStub<'a>> for SampledSuffixArray<'a> {
    type Error = Error;

    fn try_from(ssas: &'a SampledSuffixArrayStub) -> Result<Self, Self::Error> {
        let SampledSuffixArrayStub { sampling, zero, sampled } = ssas;
        let (sampled, _) = SampledArray::parse(sampled)?;
        Ok(SampledSuffixArray { sampling: *sampling, zero: *zero, sampled })
    }
}

//////////////////////////////////////// SampledSuffixArray ////////////////////////////////////////

pub struct SampledSuffixArray<'a> {
    sampling: u32,
    zero: u64,
    sampled: SampledArray<'a>,
}

impl<'a> SuffixArray for SampledSuffixArray<'a> {
    fn construct<H: Helper>(sampling: usize, sa: &[usize], builder: &mut Builder<H>) -> Result<(), Error> {
        if sampling > 63 {
            return Err(Error::InvalidSuffixArray);
        }
        let mut values = vec![];
        for (idx, sa) in sa.iter().enumerate() {
            if sa % (1 << sampling) == 0 {
                values.push((idx, sa >> sampling));
            }
        }
        builder.append_u32(FieldNumber::must(1), sampling as u32);
        builder.append_u64(FieldNumber::must(2), sa[0] as u64);
        SampledArray::construct(&values, &mut builder.sub(FieldNumber::must(3)))?;
        Ok(())
    }

    fn lookup<PSI: psi::Psi>(&self, sigma: &Sigma, psi: &PSI, mut idx: usize) -> Result<usize, Error> {
        let mut k = 0usize;
        loop {
            if idx == 0 {
                return Ok(self.zero as usize - k);
            }
            if let Some(sa) = self.sampled.lookup(idx) {
                let sa = sa << self.sampling;
                return Ok(sa - k);
            }
            idx = psi.lookup(sigma, idx)?;
            k += 1;
        }
    }
}

impl<'a> Unpackable<'a> for SampledSuffixArray<'a> {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Self::Error> {
        let (stub, buf) = SampledSuffixArrayStub::unpack(buf).map_err(|_| Error::InvalidSuffixArray)?;
        Ok((Self {
            sampling: stub.sampling,
            zero: stub.zero,
            sampled: SampledArray::parse(stub.sampled)?.0,
        }, buf))
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use crate::psi::ReferencePsi;
    use crate::test_util::TestCase;
    use crate::test_cases_for;

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
