use buffertk::Unpackable;
use prototk::FieldNumber;

use super::sampled::SampledArray;
use super::Builder;
use super::Error;
use super::Helper;

//////////////////////////////////////// InverseSuffixArray ////////////////////////////////////////

pub trait InverseSuffixArray {
    fn construct<H: Helper>(
        isa: &[usize],
        to_sample: &[usize],
        builder: &mut Builder<H>,
    ) -> Result<(), Error>;
    fn lookup(&self, idx: usize) -> Result<usize, Error>;
}

////////////////////////////////// ReferenceInverseSuffixArrayStub /////////////////////////////////

#[derive(Clone, Debug, Default, prototk_derive::Message)]
pub struct ReferenceInverseSuffixArrayStub {
    #[prototk(1, uint64)]
    isa: Vec<u64>,
}

impl From<&[usize]> for ReferenceInverseSuffixArrayStub {
    fn from(isa: &[usize]) -> Self {
        let isa = isa.iter().map(|x| *x as u64).collect();
        Self { isa }
    }
}

impl From<ReferenceInverseSuffixArray> for ReferenceInverseSuffixArrayStub {
    fn from(risa: ReferenceInverseSuffixArray) -> Self {
        let isa: &[usize] = &risa.isa;
        Self::from(isa)
    }
}

impl TryFrom<ReferenceInverseSuffixArrayStub> for ReferenceInverseSuffixArray {
    type Error = Error;

    fn try_from(risas: ReferenceInverseSuffixArrayStub) -> Result<Self, Self::Error> {
        let ReferenceInverseSuffixArrayStub { isa } = risas;
        if isa.iter().any(|x| *x > usize::MAX as u64) {
            return Err(Error::IntoUsize);
        }
        let isa = isa.into_iter().map(|x| x as usize).collect();
        Ok(ReferenceInverseSuffixArray { isa })
    }
}

//////////////////////////////////// ReferenceInverseSuffixArray ///////////////////////////////////

pub struct ReferenceInverseSuffixArray {
    isa: Vec<usize>,
}

impl InverseSuffixArray for ReferenceInverseSuffixArray {
    fn construct<H: Helper>(
        isa: &[usize],
        _: &[usize],
        builder: &mut Builder<H>,
    ) -> Result<(), Error> {
        let stub = ReferenceInverseSuffixArrayStub::from(isa);
        builder.append_raw_packable(&stub);
        Ok(())
    }

    fn lookup(&self, idx: usize) -> Result<usize, Error> {
        self.isa.get(idx).copied().ok_or(Error::BadIndex(idx))
    }
}

impl<'a> Unpackable<'a> for ReferenceInverseSuffixArray {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Self::Error> {
        let (risa, buf) = <ReferenceInverseSuffixArrayStub as Unpackable>::unpack(buf)
            .map_err(|_| Error::InvalidDocument)?;
        Ok((risa.try_into()?, buf))
    }
}

/////////////////////////////////// SampledInverseSuffixArrayStub //////////////////////////////////

#[derive(Clone, Debug, Default, prototk_derive::Message)]
pub struct SampledInverseSuffixArrayStub<'a> {
    #[prototk(1, bytes)]
    sampled: &'a [u8],
}

impl<'a> TryFrom<&'a SampledInverseSuffixArrayStub<'a>> for SampledInverseSuffixArray<'a> {
    type Error = Error;

    fn try_from(ssas: &'a SampledInverseSuffixArrayStub) -> Result<Self, Self::Error> {
        let SampledInverseSuffixArrayStub { sampled } = ssas;
        let (sampled, _) = SampledArray::parse(sampled)?;
        Ok(SampledInverseSuffixArray { sampled })
    }
}

///////////////////////////////////// SampledInverseSuffixArray ////////////////////////////////////

#[derive(Debug)]
pub struct SampledInverseSuffixArray<'a> {
    sampled: SampledArray<'a>,
}

impl<'a> InverseSuffixArray for SampledInverseSuffixArray<'a> {
    fn construct<H: Helper>(
        isa: &[usize],
        to_sample: &[usize],
        builder: &mut Builder<H>,
    ) -> Result<(), Error> {
        let mut values: Vec<(usize, usize)> = vec![];
        for sampled in to_sample.iter() {
            if *sampled >= isa.len()
                || (!values.is_empty() && values[values.len() - 1].0 >= *sampled)
            {
                return Err(Error::InvalidInverseSuffixArray);
            }
            values.push((*sampled, isa[*sampled]));
        }
        SampledArray::construct(&values, &mut builder.sub(FieldNumber::must(1)))?;
        Ok(())
    }

    fn lookup(&self, idx: usize) -> Result<usize, Error> {
        self.sampled.lookup(idx).ok_or(Error::BadIndex(idx))
    }
}

impl<'a> Unpackable<'a> for SampledInverseSuffixArray<'a> {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Self::Error> {
        let (stub, buf) = SampledInverseSuffixArrayStub::unpack(buf)
            .map_err(|_| Error::InvalidInverseSuffixArray)?;
        Ok((
            Self {
                sampled: SampledArray::parse(stub.sampled)?.0,
            },
            buf,
        ))
    }
}
