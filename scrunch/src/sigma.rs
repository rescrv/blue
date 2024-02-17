//! Sigma is the greek character often used to represent an alphabet when dealing with languages.
//! The sigma module provides a data structure for describing the alphabet used in a piece of text.

use std::collections::HashMap;

use buffertk::Unpackable;
use prototk::FieldNumber;

use crate::bit_vector::sparse::BitVector;
use crate::bit_vector::BitVector as BitVectorTrait;
use crate::builder::{Builder, Helper};
use crate::Error;

///////////////////////////////////////////// SigmaStub ////////////////////////////////////////////

#[derive(Clone, Debug, Default, prototk_derive::Message)]
struct SigmaStub<'a> {
    #[prototk(1, uint32)]
    sigma_to_char: Vec<u32>,
    #[prototk(2, bytes)]
    columns: &'a [u8],
}

/////////////////////////////////////////////// Sigma //////////////////////////////////////////////

pub struct Sigma<'a> {
    char_to_sigma: HashMap<u32, usize>,
    sigma_to_char: Vec<u32>,
    columns: BitVector<'a>,
}

impl<'a> Sigma<'a> {
    pub fn construct<I: IntoIterator<Item = u32>, H: Helper>(
        iter: I,
        builder: &mut Builder<H>,
    ) -> Result<(), Error> {
        // count each character
        let mut counts: HashMap<u32, usize> = HashMap::new();
        for t in iter.into_iter() {
            *counts.entry(t).or_insert(0) += 1;
        }
        // create an array of characters in ascending order
        let mut sigma_to_char: Vec<u32> = Vec::with_capacity(counts.len());
        for (t, _) in counts.iter() {
            sigma_to_char.push(*t);
        }
        sigma_to_char.sort();
        // use the counts and the ascending order to create buckets
        let mut buckets: Vec<usize> = Vec::with_capacity(sigma_to_char.len() + 1);
        buckets.push(0);
        let mut total = 0;
        for t in sigma_to_char.iter() {
            total += counts.get(t).ok_or(Error::LogicError("count not found"))?;
            buckets.push(total);
        }
        // sanity checks on the way out the door.
        assert_eq!(sigma_to_char.len() + 1, buckets.len());
        let columns_len = buckets[buckets.len() - 1];
        // TODO(rescrv):  Make this 16 configurable.
        // Or at least tune it by something other than intuition.
        builder.append_vec_u32(FieldNumber::must(1), &sigma_to_char);
        let mut columns_builder = builder.sub(FieldNumber::must(2));
        BitVector::from_indices(128, columns_len + 1, &buckets, &mut columns_builder)
            .ok_or(Error::CouldNotConstructBitVector)?;
        Ok(())
    }

    /// K is the number of unique characters (including sentinels) in this alphabet.
    #[allow(non_snake_case)]
    pub fn K(&self) -> usize {
        self.sigma_to_char.len() + 1
    }

    /// char_to_sigma takes a character from its original domain to the range [0, K).
    pub fn char_to_sigma(&self, t: u32) -> Option<u32> {
        // SAFETY(rescrv): Sigma should never have more than u32::MAX symbols.
        self.char_to_sigma.get(&t).copied().map(|x| x as u32)
    }

    /// sigma_to_char takes a character from the range [1, K] to its original domain.
    pub fn sigma_to_char(&self, t: u32) -> Option<u32> {
        self.sigma_to_char.get(t as usize - 1).copied()
    }

    /// bucket_starts fills in the provided slice of buckets (with length K) with the indices at
    /// which each of the K symbols will first appear in the suffix array.
    pub fn bucket_starts(&self, buckets: &mut Vec<usize>) -> Result<(), Error> {
        buckets.resize(self.K(), 0);
        for (i, bucket) in buckets.iter_mut().enumerate() {
            *bucket = self.columns.select(i).ok_or(Error::BadSelect(i))?;
        }
        Ok(())
    }

    /// bucket_limits fills in the provided slice of buckets (with length K) with the indices one
    /// past the last occurrence of each of the K symbols in the suffix array.
    pub fn bucket_limits(&self, buckets: &mut Vec<usize>) -> Result<(), Error> {
        buckets.resize(self.K(), 0);
        for (i, bucket) in buckets.iter_mut().enumerate() {
            *bucket = self.columns.select(i + 1).ok_or(Error::BadSelect(i + 1))?;
        }
        Ok(())
    }

    /// sa_index_to_sigma returns the first character of the prefix at position idx in the suffix
    /// array.  The returned value is on the interval [0, K).
    pub fn sa_index_to_sigma(&self, idx: usize) -> Option<u32> {
        // Note that rank will return Some(...) for rank(len()) and we don't want that.
        if idx < self.columns.len() {
            // SAFETY(rescrv):  It's impossible to construct sigma with more than u32::MAX symbols.
            self.columns.rank(idx).map(|x| x as u32)
        } else {
            None
        }
    }

    /// sa_index_to_t returns the first character of the prefix at position idx in the suffix
    /// array.  The returned value is in the original domain of the text.
    pub fn sa_index_to_t(&self, idx: usize) -> Option<u32> {
        if 0 < idx && idx < self.columns.len() {
            Some(self.sigma_to_char[self.columns.rank(idx)? - 1])
        } else {
            None
        }
    }

    /// sa_range_for returns the lower and upper bounds on indices in the suffix array that begin
    /// with the provided character.  For example a return value of (lower, upper) means that all
    /// indices idx, lower <= idx < upper will have SA[idx] be a prefix beginning with T.
    pub fn sa_range_for(&self, t: u32) -> Result<(usize, usize), Error> {
        match self.char_to_sigma(t) {
            Some(idx) => self.sa_range_for_sigma(idx),
            None => Ok((1, 0)),
        }
    }

    /// sa_range_for_sigma returns the lower and upper bounds on indices in the suffix array that
    /// begin with the provided sigma.  This variant takes sigma, a value on [1..K).
    pub fn sa_range_for_sigma(&self, t: u32) -> Result<(usize, usize), Error> {
        let idx = t as usize;
        Ok((
            self.columns.select(idx).ok_or(Error::BadSelect(idx))?,
            self.columns
                .select(idx + 1)
                .ok_or(Error::BadSelect(idx + 1))? - 1,
        ))
    }
}

impl<'a> std::fmt::Debug for Sigma<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.debug_struct("Sigma")
            .field("char_to_sigma", &self.char_to_sigma)
            .field("sigma_to_char", &self.sigma_to_char)
            .finish()
    }
}

impl<'a> Unpackable<'a> for Sigma<'a> {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let (stub, buf) = SigmaStub::unpack(buf).map_err(|_| Error::Unparseable)?;
        let columns = BitVector::new(stub.columns).ok_or(Error::Unparseable)?;
        let sigma_to_char = stub.sigma_to_char;
        let mut char_to_sigma = HashMap::with_capacity(sigma_to_char.len());
        for (idx, t) in sigma_to_char.iter().enumerate() {
            char_to_sigma.insert(*t, idx + 1);
        }
        Ok((
            Self {
                sigma_to_char,
                char_to_sigma,
                columns,
            },
            buf,
        ))
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use crate::test_util::{assert_eq_with_ctx, test_cases_for, TestCase};

    use super::*;

    fn check_sigma_from(t: &TestCase) {
        let sigma = t.sigma();
        let sigma = Sigma::unpack(&sigma).expect("test should unpack").0;
        // check the sigma2text
        for i in 0..t.sigma2text.len() {
            assert_eq_with_ctx!(t.sigma2text[i] as u32, sigma.sigma_to_char[i], i);
        }
        assert_eq_with_ctx!(t.sigma2text.len(), sigma.sigma_to_char.len());
        // check the boundaries
        assert_eq_with_ctx!(0, sigma.columns.rank(0).expect("rank should succeed"));
        for i in 0..t.boundaries.len() {
            assert_eq_with_ctx!(
                Some(i + 1),
                sigma.columns.rank(t.boundaries[i]),
                i,
                t.boundaries[i],
                sigma.columns.len()
            );
        }
    }

    test_cases_for!(sigma_from, super::check_sigma_from);

    #[allow(non_snake_case)]
    fn check_K(t: &TestCase) {
        let sigma = t.sigma();
        let sigma = Sigma::unpack(&sigma).expect("test should unpack").0;
        assert_eq_with_ctx!(t.boundaries.len(), sigma.K());
    }

    test_cases_for!(k, super::check_K);

    fn check_char_to_sigma(t: &TestCase) {
        let sigma = t.sigma();
        let sigma = Sigma::unpack(&sigma).expect("test should unpack").0;
        // characters that should be there
        for i in 0..t.sigma2text.len() {
            assert_eq_with_ctx!(
                Some((i + 1) as u32),
                sigma.char_to_sigma(t.sigma2text[i] as u32),
                i,
                t.sigma2text[i]
            );
        }
        // characters that should not be there
        for i in 0..t.not_in_str.len() {
            assert_eq_with_ctx!(
                None,
                sigma.char_to_sigma(t.not_in_str[i] as u32),
                t.not_in_str[i]
            );
        }
    }

    test_cases_for!(char_to_sigma, super::check_char_to_sigma);

    fn check_bucket_starts(t: &TestCase) {
        let sigma = t.sigma();
        let sigma = Sigma::unpack(&sigma).expect("test should unpack").0;
        let mut buckets = vec![0; sigma.K()];
        sigma.bucket_starts(&mut buckets).unwrap();
        let bucket_starts: &[usize] = &buckets;
        assert_eq_with_ctx!(t.bucket_starts, bucket_starts);
    }

    test_cases_for!(bucket_starts, super::check_bucket_starts);

    fn check_bucket_limits(t: &TestCase) {
        let sigma = t.sigma();
        let sigma = Sigma::unpack(&sigma).expect("test should unpack").0;
        let mut buckets = vec![0; sigma.K()];
        sigma.bucket_limits(&mut buckets).unwrap();
        let bucket_limits: &[usize] = &buckets;
        assert_eq_with_ctx!(t.bucket_limits, bucket_limits);
    }

    test_cases_for!(bucket_limits, super::check_bucket_limits);

    fn check_sa_index_to_sigma(t: &TestCase) {
        let sigma = t.sigma();
        let sigma = Sigma::unpack(&sigma).expect("test should unpack").0;
        // check the test case for a little sanity
        assert_eq_with_ctx!(t.S.len(), t.SA.len());
        assert_eq_with_ctx!(t.SA.len(), t.deref_SA.len());
        // evaluate
        for i in 0..t.deref_SA.len() {
            assert_eq_with_ctx!(Some(t.deref_SA[i]), sigma.sa_index_to_sigma(i), i);
        }
        assert_eq_with_ctx!(
            None,
            sigma.sa_index_to_sigma(t.deref_SA.len()),
            t.deref_SA.len()
        );
    }

    test_cases_for!(sa_index_to_sigma, super::check_sa_index_to_sigma);

    fn check_sa_index_to_t(t: &TestCase) {
        let sigma = t.sigma();
        let sigma = Sigma::unpack(&sigma).expect("test should unpack").0;
        // check the test case for a little sanity
        assert_eq_with_ctx!(t.S.len(), t.SA.len());
        assert_eq_with_ctx!(t.SA.len(), t.deref_SA.len());
        // evaluate
        assert_eq_with_ctx!(0, t.deref_SA[0]);
        assert_eq_with_ctx!(None, sigma.sa_index_to_t(0));
        for i in 1..t.deref_SA.len() {
            // sanity check
            assert!(t.deref_SA[i] > 0);
            assert!(t.deref_SA[i] as usize - 1 < t.sigma2text.len());
            // evaluate
            assert_eq_with_ctx!(
                Some(t.sigma2text[t.deref_SA[i] as usize - 1] as u32),
                sigma.sa_index_to_t(i),
                i
            );
        }
        assert_eq_with_ctx!(
            None,
            sigma.sa_index_to_t(t.deref_SA.len()),
            t.deref_SA.len()
        );
    }

    test_cases_for!(sa_index_to_t, super::check_sa_index_to_t);

    fn check_sa_range_for(t: &TestCase) {
        let sigma = t.sigma();
        let sigma = Sigma::unpack(&sigma).expect("test should unpack").0;
        // existing characters
        for i in 0..t.sigma2text.len() {
            assert_eq_with_ctx!(
                (t.bucket_starts[i + 1], t.bucket_limits[i + 1] - 1),
                sigma.sa_range_for(t.sigma2text[i] as u32).unwrap()
            );
        }
        // non-existent characters
        for i in 0..t.not_in_str.len() {
            assert_eq_with_ctx!(
                (1, 0),
                sigma.sa_range_for(t.not_in_str[i] as u32).unwrap(),
                t.not_in_str[i]
            );
        }
    }

    test_cases_for!(sa_range_for, super::check_sa_range_for);
}
