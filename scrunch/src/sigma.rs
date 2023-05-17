//! Sigma is the greek character often used to represent an alphabet when dealing with languages.
//! The sigma module provides a data structure for describing the alphabet used in a piece of text.

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::Hash;
use std::iter::FromIterator;

use crate::bit_vector::OldBitVector;

/// Sigma represents an alphabet.
pub struct Sigma<T, B>
where
    B: OldBitVector,
{
    sigma_to_char: Vec<T>,
    char_to_sigma: HashMap<T, usize>,
    columns: B,
}

impl<T, B> From<&[T]> for Sigma<T, B>
where
    T: Copy + Eq + Hash + Ord,
    B: OldBitVector,
{
    fn from(text: &[T]) -> Self {
        text.iter().map(|&x| x).collect()
    }
}

impl<T, B> FromIterator<T> for Sigma<T, B>
where
    T: Copy + Eq + Hash + Ord,
    B: OldBitVector,
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        // count each character
        let mut counts: HashMap<T, usize> = HashMap::new();
        for t in iter {
            *counts.entry(t).or_insert(0) += 1;
        }
        // create an array of characters in ascending order
        let mut sigma_to_char: Vec<T> = Vec::with_capacity(counts.len());
        for (t, _) in counts.iter() {
            sigma_to_char.push(*t);
        }
        sigma_to_char.sort();
        // use the counts and the ascending order to create buckets
        let mut buckets: Vec<usize> = Vec::with_capacity(sigma_to_char.len() + 1);
        buckets.push(0);
        let mut total = 0;
        for t in sigma_to_char.iter() {
            total += match counts.get(t) {
                Some(x) => x,
                None => {
                    panic!("there is a durability bug or the text changed underneath us");
                }
            };
            buckets.push(total);
        }
        // repurpose counts map to map T->sigma
        let mut char_to_sigma = counts;
        for (idx, t) in sigma_to_char.iter().enumerate() {
            match char_to_sigma.entry(*t) {
                Entry::Occupied(mut entry) => {
                    *entry.get_mut() = idx + 1;
                }
                Entry::Vacant(_) => {
                    panic!("there is a durability bug or the text changed underneath us");
                }
            }
        }
        // sanity checks on the way out the door.
        assert_eq!(sigma_to_char.len(), char_to_sigma.len());
        assert_eq!(sigma_to_char.len() + 1, buckets.len());
        Self {
            sigma_to_char,
            char_to_sigma,
            columns: B::sparse(&buckets),
        }
    }
}

impl<B> Sigma<usize, B>
where
    B: OldBitVector,
{
    /// from_subproblem creates a Sigma suitable for representing the provided subproblem of the
    /// suffix array-induced sort algorithm (see module sais).
    ///
    /// # Panics
    /// - text must be non-empty
    /// - the last element of text must be 0
    pub fn from_subproblem(text: &[usize]) -> Self {
        assert!(text.len() > 0);
        assert_eq!(0, text[text.len() - 1]);
        // TODO(rescrv):  cloned()?
        text[..text.len() - 1].iter().cloned().collect()
    }
}

impl<T, B> Sigma<T, B>
where
    T: Copy + Clone + Eq + Hash + Ord,
    B: OldBitVector,
{
    /// K is the number of unique characters (including sentinels) in this alphabet.
    #[allow(non_snake_case)]
    pub fn K(&self) -> usize {
        self.sigma_to_char.len() + 1
    }

    /// char_to_sigma takes a character from its original domain to the range [0, K).
    pub fn char_to_sigma(&self, t: T) -> Option<usize> {
        // TODO(rescrv): A better way to remove the ref than the map?
        self.char_to_sigma.get(&t).map(|x| *x)
    }

    /// text_to_sigma takes a text from its original domain to a sequence of characters on the
    /// range [0, K).
    pub fn text_to_sigma(&self, text: &[T]) -> Option<Vec<usize>> {
        let mut sigma = Vec::with_capacity(text.len() + 1);
        for t in text.iter() {
            match self.char_to_sigma(*t) {
                Some(x) => {
                    sigma.push(x);
                }
                None => {
                    return None;
                }
            };
        }
        sigma.push(0);
        Some(sigma)
    }

    /// bucket_starts fills in the provided slice of buckets (with length K) with the indices at
    /// which each of the K symbols will first appear in the suffix array.
    pub fn bucket_starts(&self, buckets: &mut [usize]) {
        assert_eq!(self.K(), buckets.len());
        for i in 0..buckets.len() {
            buckets[i] = self.columns.select(i);
        }
    }

    /// bucket_limits fills in the provided slice of buckets (with length K) with the indices one
    /// past the last occurrence of each of the K symbols in the suffix array.
    pub fn bucket_limits(&self, buckets: &mut [usize]) {
        assert_eq!(self.K(), buckets.len());
        for i in 0..buckets.len() {
            buckets[i] = self.columns.select(i + 1);
        }
    }

    /// sa_index_to_sigma returns the first character of the prefix at position idx in the suffix
    /// array.  The returned value is on the interval [0, K).
    pub fn sa_index_to_sigma(&self, idx: usize) -> Option<usize> {
        if idx < self.columns.len() {
            Some(self.columns.rank(idx))
        } else {
            None
        }
    }

    /// sa_index_to_t returns the first character of the prefix at position idx in the suffix
    /// array.  The returned value is in the original domain of the text.
    pub fn sa_index_to_t(&self, idx: usize) -> Option<T> {
        if 0 < idx && idx < self.columns.len() {
            Some(self.sigma_to_char[self.columns.rank(idx) - 1])
        } else {
            None
        }
    }

    /// sa_range_for returns the lower and upper bounds on indices in the suffix array that begin
    /// with the provided character.  For example a return value of (lower, upper) means that all
    /// indices idx, lower <= idx < upper will have SA[idx] be a prefix beginning with T.
    pub fn sa_range_for(&self, t: T) -> (usize, usize) {
        match self.char_to_sigma(t) {
            Some(idx) => (self.columns.select(idx), self.columns.select(idx + 1)),
            None => (0, 0),
        }
    }

    /// columns is an escape hatch to return the raw bit vector over characters of the suffix array.
    pub fn columns(&self) -> &B {
        &self.columns
    }

    #[cfg(test)]
    pub fn test_hack(sigma: &[T], text: &[usize]) -> Self {
        let mut alt: Vec<T> = Vec::with_capacity(text.len() - 1);
        for i in 0..text.len() - 1 {
            alt.push(sigma[text[i] - 1]);
        }
        let alt: &[T] = &alt;
        Sigma::from(alt)
    }
}

// Translate the arbitrary-alphabet string s into a set of contiguous characters [0, sigma_sz] and
// return the string and sigma.
//
// The resulting output string is a Vec<usize> that has one element for each element in the input
// plus a trailing 0 sentinel.  It is guaranteed that no other element has a value of zero.  Every
// other element maps to the range [1, sigma.len()] such that a value c translates to sigma[c - 1].
// // TODO(rescrv): remove this
pub fn to_compact_alphabet<T>(s: &[T]) -> (Vec<usize>, Vec<T>)
where
    T: Copy + Clone + Eq + Hash + Ord,
{
    let mut alphabet: HashSet<T> = HashSet::new();
    for c in s.iter() {
        alphabet.insert(*c);
    }
    let mut ordered: Vec<T> = Vec::with_capacity(alphabet.len());
    for a in alphabet.iter() {
        ordered.push(*a);
    }
    ordered.sort_unstable();
    let mut alphabet = HashMap::with_capacity(ordered.len());
    for (i, c) in ordered.iter().cloned().enumerate() {
        alphabet.insert(c, i + 1);
    }
    let mut translated: Vec<usize> = Vec::with_capacity(s.len() + 1);
    for c in s.iter() {
        translated.push(*alphabet.get(&c).unwrap());
    }
    translated.push(0);
    (translated, ordered)
}

#[cfg(test)]
mod tests {
    use crate::bit_vector::ReferenceOldBitVector;
    use crate::test_cases_for;
    use crate::testutil::TestCase;

    use super::*;

    fn check_sigma_from(t: &TestCase) {
        let sigma = t.sigma();
        // check the sigma2text
        for i in 0..t.sigma2text.len() {
            assert_eq!(t.sigma2text[i], sigma.sigma_to_char[i], "i={}", i);
        }
        assert_eq!(t.sigma2text.len(), sigma.sigma_to_char.len());
        // check the boundaries
        assert_eq!(0, sigma.columns.rank(0));
        for i in 0..t.boundaries.len() {
            assert_eq!(
                i + 1,
                sigma.columns.rank(t.boundaries[i]),
                "sigma.columns.rank(t.boundaries[i={}] = {}) = {}",
                i,
                t.boundaries[i],
                sigma.columns.rank(t.boundaries[i])
            );
        }
    }

    test_cases_for!(sigma_from, crate::sigma::tests::check_sigma_from);

    fn check_test_hack(t: &TestCase) {
        let sigma_new: Sigma<char, ReferenceOldBitVector> = Sigma::test_hack(t.sigma2text, t.S);
        let sigma_from = t.sigma();
        for i in 0..std::cmp::min(sigma_from.columns.len(), sigma_new.columns.len()) {
            assert_eq!(
                sigma_new.columns.rank(i),
                sigma_from.columns.rank(i),
                "i={}",
                i
            );
        }
        assert_eq!(sigma_new.columns.len(), sigma_from.columns.len());
    }

    test_cases_for!(new_matches_from_XXX, crate::sigma::tests::check_test_hack);

    #[allow(non_snake_case)]
    fn check_K(t: &TestCase) {
        let sigma = t.sigma();
        assert_eq!(t.boundaries.len(), sigma.K());
    }

    #[allow(non_snake_case)]
    test_cases_for!(k, crate::sigma::tests::check_K);

    fn check_char_to_sigma(t: &TestCase) {
        let sigma = t.sigma();
        // characters that should be there
        for i in 0..t.sigma2text.len() {
            assert_eq!(
                Some(i + 1),
                sigma.char_to_sigma(t.sigma2text[i]),
                "i={} == char_to_sigma[{}]",
                i,
                t.sigma2text[i]
            );
        }
        // characters that should not be there
        for i in 0..t.not_in_str.len() {
            assert_eq!(
                None,
                sigma.char_to_sigma(t.not_in_str[i]),
                "None == char_to_sigma[{}]",
                t.not_in_str[i]
            );
        }
    }

    test_cases_for!(char_to_sigma, crate::sigma::tests::check_char_to_sigma);

    fn check_text_to_sigma(t: &TestCase) {
        let chars: Vec<char> = t.text.chars().collect();
        let sigma = t.sigma();
        let translated: &[usize] = &sigma.text_to_sigma(&chars).unwrap();
        assert_eq!(t.S, translated);
    }

    test_cases_for!(text_to_sigma, crate::sigma::tests::check_text_to_sigma);

    fn check_bucket_starts(t: &TestCase) {
        let sigma = t.sigma();
        let mut buckets: &mut [usize] = &mut vec![0; sigma.K()];
        sigma.bucket_starts(&mut buckets);
        let bucket_starts: &[usize] = &buckets;
        assert_eq!(t.bucket_starts, bucket_starts);
    }

    test_cases_for!(bucket_starts, crate::sigma::tests::check_bucket_starts);

    fn check_bucket_limits(t: &TestCase) {
        let sigma = t.sigma();
        let mut buckets: &mut [usize] = &mut vec![0; sigma.K()];
        sigma.bucket_limits(&mut buckets);
        let bucket_limits: &[usize] = &buckets;
        assert_eq!(t.bucket_limits, bucket_limits);
    }

    test_cases_for!(bucket_limits, crate::sigma::tests::check_bucket_limits);

    fn check_sa_index_to_sigma(t: &TestCase) {
        let sigma = t.sigma();
        // check the test case for a little sanity
        assert_eq!(t.S.len(), t.SA.len());
        assert_eq!(t.SA.len(), t.deref_SA.len());
        // evaluate
        for i in 0..t.deref_SA.len() {
            assert_eq!(Some(t.deref_SA[i]), sigma.sa_index_to_sigma(i), "i={}", i);
        }
        assert_eq!(
            None,
            sigma.sa_index_to_sigma(t.deref_SA.len()),
            "i={}",
            t.deref_SA.len()
        );
    }

    test_cases_for!(
        sa_index_to_sigma,
        crate::sigma::tests::check_sa_index_to_sigma
    );

    fn check_sa_index_to_t(t: &TestCase) {
        let sigma = t.sigma();
        // check the test case for a little sanity
        assert_eq!(t.S.len(), t.SA.len());
        assert_eq!(t.SA.len(), t.deref_SA.len());
        // evaluate
        assert_eq!(0, t.deref_SA[0]);
        assert_eq!(None, sigma.sa_index_to_t(0), "i=0");
        for i in 1..t.deref_SA.len() {
            // sanity check
            assert!(t.deref_SA[i] > 0);
            assert!(t.deref_SA[i] - 1 < t.sigma2text.len());
            // evaluate
            assert_eq!(
                Some(t.sigma2text[t.deref_SA[i] - 1]),
                sigma.sa_index_to_t(i),
                "i={}",
                i
            );
        }
        assert_eq!(
            None,
            sigma.sa_index_to_t(t.deref_SA.len()),
            "i={}",
            t.deref_SA.len()
        );
    }

    test_cases_for!(sa_index_to_t, crate::sigma::tests::check_sa_index_to_t);

    fn check_sa_range_for(t: &TestCase) {
        let sigma = t.sigma();
        // existing characters
        for i in 0..t.sigma2text.len() {
            assert_eq!(
                (t.bucket_starts[i + 1], t.bucket_limits[i + 1]),
                sigma.sa_range_for(t.sigma2text[i])
            );
        }
        // non-existent characters
        for i in 0..t.not_in_str.len() {
            assert_eq!(
                (0, 0),
                sigma.sa_range_for(t.not_in_str[i]),
                "(0, 0) == sa_range_for[{}]",
                t.not_in_str[i]
            );
        }
    }

    test_cases_for!(sa_range_for, crate::sigma::tests::check_sa_range_for);
}
