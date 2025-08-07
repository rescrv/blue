use buffertk::Unpackable;

use crate::builder::{Builder, Helper};
use crate::sigma::Sigma;
use crate::Error;

pub mod wavelet_tree;

//////////////////////////////////////////////// Psi ///////////////////////////////////////////////

pub trait Psi {
    /// Append the byte-representation of the Psi to buf.
    fn construct<H: Helper>(
        sigma: &Sigma,
        psi: &[usize],
        builder: &mut Builder<H>,
    ) -> Result<(), Error>;

    /// The length of the psi.  Should be the same as the number of symbols in the text +
    /// terminating symbol.
    fn len(&self) -> usize;

    /// True if the psi is empty.  Should always be false because there is always a terminating
    /// symbol.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Lookup offset `idx` in the psi.
    fn lookup(&self, sigma: &Sigma, idx: usize) -> Result<usize, Error>;

    /// Constrain the provided range such that it shrinks range to only those values whos successor
    /// maps to into.  The building block for backwards search.
    ///
    /// This code imposes the following requirements:
    /// - sigma.sa_index_to_sigma(range.0) == sigma.sa_index_to_sigma(range.1)
    /// - range is interpreted as a closed interval
    /// - into is interpreted as a closed interval
    /// - the answer is a closed interval.
    fn constrain(
        &self,
        sigma: &Sigma,
        range: (usize, usize),
        into: (usize, usize),
    ) -> Result<(usize, usize), Error>;
}

///////////////////////////////////////// ReferencePsiStub /////////////////////////////////////////

#[derive(Clone, Debug, Default, prototk_derive::Message)]
pub struct ReferencePsiStub {
    #[prototk(1, uint64)]
    psi: Vec<u64>,
}

impl From<&[usize]> for ReferencePsiStub {
    fn from(psi: &[usize]) -> Self {
        let psi = psi.iter().map(|x| *x as u64).collect();
        Self { psi }
    }
}

impl From<ReferencePsi> for ReferencePsiStub {
    fn from(rpsi: ReferencePsi) -> Self {
        let psi: &[usize] = &rpsi.psi;
        Self::from(psi)
    }
}

impl TryFrom<ReferencePsiStub> for ReferencePsi {
    type Error = Error;

    fn try_from(rpsi: ReferencePsiStub) -> Result<Self, Self::Error> {
        let ReferencePsiStub { psi } = rpsi;
        if psi.iter().any(|x| *x > usize::MAX as u64) {
            return Err(Error::IntoUsize);
        }
        let psi = psi.into_iter().map(|x| x as usize).collect();
        Ok(ReferencePsi { psi })
    }
}

/////////////////////////////////////////// ReferencePsi ///////////////////////////////////////////

pub struct ReferencePsi {
    psi: Vec<usize>,
}

impl ReferencePsi {
    pub fn new(psi: &[usize]) -> Self {
        Self { psi: psi.to_vec() }
    }
}

impl Psi for ReferencePsi {
    fn construct<H: Helper>(
        _: &Sigma,
        psi: &[usize],
        builder: &mut Builder<H>,
    ) -> Result<(), Error> {
        let stub = ReferencePsiStub::from(psi);
        builder.append_raw_packable(&stub);
        Ok(())
    }

    fn len(&self) -> usize {
        self.psi.len()
    }

    fn lookup(&self, _sigma: &Sigma, idx: usize) -> Result<usize, Error> {
        self.psi.get(idx).copied().ok_or(Error::BadIndex(idx))
    }

    fn constrain(
        &self,
        sigma: &Sigma,
        range: (usize, usize),
        into: (usize, usize),
    ) -> Result<(usize, usize), Error> {
        let start = match self.psi[range.0..=range.1].binary_search_by(|probe| probe.cmp(&into.0)) {
            Ok(x) => x + range.0,
            Err(x) => x + range.0,
        };
        let limit =
            match self.psi[range.0..=range.1].binary_search_by(|probe| probe.cmp(&(into.1 + 1))) {
                Ok(x) => x + range.0 - 1,
                Err(x) => x + range.0 - 1,
            };
        assert!(start > limit || sigma.sa_index_to_sigma(start) == sigma.sa_index_to_sigma(limit));
        Ok((start, limit))
    }
}

impl<'a> Unpackable<'a> for ReferencePsi {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Self::Error> {
        let (rpsi, buf) =
            <ReferencePsiStub as Unpackable>::unpack(buf).map_err(|_| Error::InvalidPsi)?;
        Ok((rpsi.try_into()?, buf))
    }
}

////////////////////////////////////////////// compute /////////////////////////////////////////////

pub fn compute(isa: &[usize]) -> Vec<usize> {
    let mut psi = vec![0usize; isa.len()];
    psi[isa[isa.len() - 1]] = isa[0];
    for i in 1..isa.len() {
        psi[isa[i - 1]] = isa[i];
    }
    psi
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
pub mod tests {
    use buffertk::Unpackable;

    use crate::test_util::{assert_eq_with_ctx, test_cases_for, TestCase};

    use super::super::builder::Builder;
    use super::super::psi::ReferencePsi;
    use super::*;

    fn check_compute_psi(t: &TestCase) {
        let psi = super::compute(t.ISA);
        assert_eq_with_ctx!(t.PSI, &psi);
    }

    test_cases_for! {compute_psi, super::check_compute_psi}

    fn check_table(t: &TestCase) {
        let sigma = t.sigma();
        let sigma = Sigma::unpack(&sigma).expect("test should unpack").0;
        let table = wavelet_tree::draw_table(&sigma, t.PSI);
        fn regularize(s: &str) -> String {
            s.chars()
                .filter(|c| c.is_ascii_punctuation() || c.is_ascii_alphanumeric())
                .collect::<String>()
                .trim()
                .replace([' ', '\n'], "")
        }
        let expected = regularize(t.table);
        let returned = regularize(&table);
        if expected != returned {
            println!("expected:\n{}", t.table);
            println!("returned:\n{table}");
            panic!("fix this test");
        }
    }

    test_cases_for! {table, super::check_table}

    fn check_psi<'a, PSI: Psi + Unpackable<'a>>(t: &TestCase, psi_buf: &'a mut Vec<u8>) {
        let sigma = t.sigma();
        let sigma = Sigma::unpack(&sigma).expect("test should unpack").0;
        let mut psi_builder = Builder::new(psi_buf);
        PSI::construct(&sigma, t.PSI, &mut psi_builder).expect("psi should construct");
        drop(psi_builder);
        let psi = PSI::unpack(psi_buf).expect("psi should parse").0;
        for (idx, expected) in t.PSI.iter().enumerate() {
            assert_eq!(
                *expected,
                psi.lookup(&sigma, idx).expect("lookup should succeed")
            );
        }
        for (range, into, answer) in t.constrain.iter() {
            assert_eq_with_ctx!(
                *answer,
                psi.constrain(&sigma, *range, *into).unwrap(),
                *range,
                *into,
                *answer
            );
        }
    }

    fn check_reference_psi(t: &TestCase) {
        let mut psi_buf = vec![];
        check_psi::<ReferencePsi>(t, &mut psi_buf);
    }

    test_cases_for! {wavelet_psi_reference, super::check_reference_psi}

    fn check_wavelet_psi_with_reference(t: &TestCase) {
        let mut psi_buf = vec![];
        check_psi::<wavelet_tree::WaveletTreePsi<super::super::wavelet_tree::ReferenceWaveletTree>>(
            t,
            &mut psi_buf,
        );
    }

    test_cases_for! {wavelet_psi_wavelet_reference, super::check_wavelet_psi_with_reference}

    fn check_wavelet_psi_with_wavelet_tree(t: &TestCase) {
        let mut psi_buf = vec![];
        check_psi::<
            wavelet_tree::WaveletTreePsi<
                super::super::wavelet_tree::prefix::WaveletTree<
                    super::super::encoder::FixedWidthEncoder,
                >,
            >,
        >(t, &mut psi_buf);
    }

    test_cases_for! {wavelet_psi_wavelet_tree, super::check_wavelet_psi_with_wavelet_tree}

    proptest::prop_compose! {
        pub fn arb_text()(text in proptest::collection::vec(1u32..4u32, 16..64)) -> Vec<u32> {
            text
        }
    }

    fn validate_against_reference_impl<'a, PSI: Psi + Unpackable<'a>>(
        text: &[u32],
        psi_buf: &'a mut Vec<u8>,
    ) {
        let mut sigma_buf = vec![];
        let mut sigma_builder = Builder::new(&mut sigma_buf);
        Sigma::construct(text.iter().copied(), &mut sigma_builder).expect("sigma should construct");
        drop(sigma_builder);
        let sigma = Sigma::unpack(&sigma_buf).expect("sigma should parse").0;
        let mut s: Vec<u32> = text
            .iter()
            .map(|c| sigma.char_to_sigma(*c).expect("text should translate"))
            .collect();
        s.push(0);
        let mut sa = vec![0usize; s.len()];
        super::super::sais::sais(&sigma, &s, &mut sa).expect("sais should complete");
        let isa = super::super::inverse(&sa);
        let computed_psi = super::compute(&isa);
        let mut psi_builder = Builder::new(psi_buf);
        PSI::construct(&sigma, &computed_psi, &mut psi_builder).expect("psi should compute");
        drop(psi_builder);
        let psi = PSI::unpack(psi_buf).expect("psi should parse").0;
        for (idx, val) in computed_psi.iter().enumerate() {
            assert_eq_with_ctx!(
                *val,
                psi.lookup(&sigma, idx).expect("psi should lookup"),
                idx
            );
        }
        let reference = ReferencePsi { psi: computed_psi };
        for range_char in 1..sigma.K() as u32 {
            let t = sigma.sigma_to_char(range_char).unwrap();
            let range = sigma.sa_range_for(t).unwrap();
            let into = (0, psi.len());

            let expected = reference
                .constrain(&sigma, range, into)
                .expect("constrain should succeed");
            let returned = psi
                .constrain(&sigma, range, into)
                .expect("constrain should succeed");

            assert_eq_with_ctx!(expected, returned, range, into);

            for into_char in 1..sigma.K() as u32 {
                let t = sigma.sigma_to_char(range_char).unwrap();
                let range = sigma.sa_range_for(t).unwrap();

                let t = sigma.sigma_to_char(into_char).unwrap();
                let into = sigma.sa_range_for(t).unwrap();

                let expected = reference
                    .constrain(&sigma, range, into)
                    .expect("constrain should succeed");
                let returned = psi
                    .constrain(&sigma, range, into)
                    .expect("constrain should succeed");

                assert_eq_with_ctx!(expected, returned, range, into);
            }
        }
    }

    proptest::proptest! {
        #[test]
        fn reference(text in arb_text()) {
            let mut psi_buf = vec![];
            validate_against_reference_impl::<ReferencePsi>(&text, &mut psi_buf);
        }
    }

    proptest::proptest! {
        #[test]
        fn wavelet_tree_reference(text in arb_text()) {
            use crate::wavelet_tree::ReferenceWaveletTree;
            let mut psi_buf = vec![];
            validate_against_reference_impl::<wavelet_tree::WaveletTreePsi<ReferenceWaveletTree>>(&text, &mut psi_buf);
        }
    }
}
