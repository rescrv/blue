//! Suffix Array-Induced Sort (sais, for short) is an algorithm to construct the suffix array of a
//! string in linear time in the length of the string.  A suffix array contains all possible
//! suffixes of a string in sorted order such that SA[i] indicates that S[i..] would be the i'th
//! suffix in sorted order.

#![allow(non_snake_case)]

use std::hash::Hash;

use crate::bit_vector::BitVector;
use crate::sigma::Sigma;

/// LSType is an indication of how a character relates to those charcters that follow it.  An
/// L-type character indicates the character is larger than the character that follows it.  An
/// S-type character is smaller than the character that follows it.  For purposes of comparison,
/// ties take on the type of the next character.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum LSType {
    L,
    S,
}

/// get_types creates an LSType-string from a usize-string.
fn get_types(S: &[usize]) -> Vec<LSType> {
    let mut prev = (LSType::S, 0);
    let mut types = Vec::with_capacity(S.len());
    for &s in S.iter().rev() {
        prev = if s < prev.1 {
            (LSType::S, s)
        } else if s == prev.1 && prev.0 == LSType::S {
            (LSType::S, s)
        } else {
            (LSType::L, s)
        };
        types.push(prev.0);
    }
    types.reverse();
    types
}

/// is_lms returns whether a particular index in the LSType-string is the left-most-S.
fn is_lms(T: &[LSType], i: usize) -> bool {
    i > 0 && T[i - 1] == LSType::L && T[i] == LSType::S
}

/// induce_L does a left-to-right induced sort to fill in L-type symbols.  By scanning
/// left-to-right we guarantee that we go in a strictly increasing order of suffixes.  The sort is
/// called "induced" because it scans a partially-filled suffix array and uses the partially-sorted
/// suffixes to select the next suffix of the string to sort.
///
/// And because we because we only fill in L-type suffixes, they are by definition larger than (to
/// the right of) the index used to construct the suffix.
fn induce_L<T, B>(sigma: &Sigma<T, B>, S: &[usize], SA: &mut [usize], T: &[LSType], buckets: &mut [usize])
where
    T: Copy + Eq + Hash + Ord,
    B: BitVector,
{
    sigma.bucket_starts(buckets);
    for i in 0..S.len() {
        let j = SA[i].wrapping_sub(1);
        if j < usize::max_value() - 1 && T[j] == LSType::L {
            SA[buckets[S[j]]] = j;
            buckets[S[j]] += 1;
        }
    }
}

/// induce_S does a right-to-left induced sort to fill in S-type symbols.  The reasoning for what
/// this does is similar to how induce_L works, except it works right-to-left, is monotonically
/// decreasing, and fills buckets from the right.
fn induce_S<T, B>(sigma: &Sigma<T, B>, S: &[usize], SA: &mut [usize], T: &[LSType], buckets: &mut [usize])
where
    T: Copy + Eq + Hash + Ord,
    B: BitVector,
{
    sigma.bucket_limits(buckets);
    for i in (0..S.len()).rev() {
        let j = SA[i].wrapping_sub(1);
        if j < usize::max_value() - 1 && T[j] == LSType::S {
            buckets[S[j]] -= 1;
            SA[buckets[S[j]]] = j;
        }
    }
}

/// sais is the Suffix-Array Induced-Sort.  This algorithm uses induced-sort (similar technique is
/// used in radix sort) to produce a suffix array in time linear in the length of the input.
///
/// # Panics:
/// - K > S.len()
/// - S.len() != SA.len()
/// - S.len() == 0
/// - S is not termintated by a zero
pub fn sais<T, B>(sigma: &Sigma<T, B>, S: &[usize], SA: &mut [usize])
    where 
    T: Copy + Eq + Hash + Ord,
    B: BitVector,
{
    // We need some space for sentinels in the lang.
    assert!(sigma.K() <= S.len());
    assert!(sigma.K() < usize::max_value()); // should never fire
                                     // The input and "output" must be aligned.
    assert!(S.len() == SA.len());
    // The last character should be zero, which also requires there to be a last character.
    assert!(S.len() > 0);
    assert_eq!(S[S.len() - 1], 0);

    ////////////////////////////////////////////////////////////////////////////////////////////////
    // Imagine we wanted to compute the suffix array of the string "baabababbab" that looks
    // something like this ($ is an end-of-string marker and the zero-character):
    //
    //   11 $
    //    1 aabababbab$
    //    9 ab$
    //    2 abababbab$
    //    4 ababbab$
    //    6 abbab$
    //   10 b$
    //    0 baabababbab$
    //    8 bab$
    //    3 bababbab$
    //    5 babbab$
    //    7 bbab$
    //
    // We could create each of these suffixes and sort, but there's a more efficient way to do it
    // known as the Suffix Array, Induced Sort algorithm[1,2].
    //
    // The key idea of the algorithm is to operate on "buckets" which are ranges of the suffix
    // array that correspond to suffixes sharing the same first character.  Then:
    //
    // 1.  Assign an L- or S-type to all characters in S.  These types do not change.  Of special
    //     note are the left-most-S (LMS) characters s.t. is_lms(i) iff T[i-1]=L and T[i]=S.
    // 2.  Place the LMS characters at the ends of their respective buckets.
    // 3.  Induce sort as a separate pass for each character type.  At the end of this pass we know
    //     the position of all L-type suffixes and use them to determine a partial order over the
    //     LMS characters.  The partial order will create a relation between any two suffixes of S
    //     whose maximally-long prefixes's types match the regular expression S+L* differ.
    // 4.  Use the LMS characters to construct a subproblem of at most size 1/2 the input.  This
    //     subproblem will produce a total-order across the left-most-S characters that will induce
    //     the proper output.
    // 5.  Place the newly-sorted LMS characters at the end of their buckets (but not their proper
    //     but not positions).
    // 6.  Repeat the induce sort for each type to place all strings.
    //
    // What follows is an expanded description of this algorithm inline with the code.

    // 1.  Assign a type to each character in the string.  The types are:
    //     L-Type:  S[i] is L-Type iff S[i] > S[j] or (S[i] == S[j] and S[j] is L-Type).
    //     S-Type:  S[i] is S-Type iff S[i] < S[j] or (S[i] == S[j] and S[j] is S-Type).
    //
    // In our running example, we find:
    // S = [b, a, a, b, a, b, a, b, b, a, b, $]
    // T = [L, S, S, L, S, L, S, L, L, S, L, S]
    let T = get_types(S);
    let T: &[LSType] = &T;
    // 2.  Compute the "buckets" for the string and place the LMS characters at the end of their
    //     respective buckets.
    //
    // In our example, we would divide the suffix array like this:
    // SA     = [11, 1, 9, 2, 4, 6, 10, 0, 8, 3, 5, 7]
    // S[SA]  = [ $, a, a, a, a, a,  b, b, b, b, b, b]
    // Bucket = [ 0, 1, 1, 1, 1, 1,  2, 2, 2, 2, 2, 2]
    let buckets: &mut [usize] = &mut vec![0; sigma.K()];
    sigma.bucket_limits(buckets);
    // We will use usize::max_value() as a sentinel to mean "empty" throughout the computation.
    for i in 0..S.len() {
        SA[i] = usize::max_value();
    }
    // Place the LMSes at the end of their respective buckets.
    // After this loop, SA will look like this:
    // SA = [11, _, 9, 6, 4, 1, _, _, _, _, _, _]
    for i in 0..S.len() {
        if is_lms(&T, i) {
            buckets[S[i]] -= 1;
            SA[buckets[S[i]]] = i;
        }
    }
    // 3.  Induce sort: left-to-right for L-type suffixes then right-to-left for S-type suffixes.
    //
    // Do a left-to-right induced-L pass:
    // SA = [11, _, 9, 6, 4, 1, 10, 8, 5, 3, 0, 7]
    induce_L(sigma, S, SA, &T, buckets);
    // And then an induce-S pass:
    // SA = [11, 1, 9, 4, 2, 6, 10, 8, 5, 3, 0, 7]
    // Note how this differs from the expected SA:
    // SA = [11, 1, 9, 2, 4, 6, 10, 0, 8, 3, 5, 7]
    induce_S(sigma, S, SA, &T, buckets);
    // 3.  Construct a subproblem using the LMS suffixes.
    let mut substrings = 0;
    // First, collect the LMS suffixes in the order they appear in the almost-sorted SA.  After
    // this loop:
    // SA[..substrings] = [11, 1, 9, 4, 6]
    for i in 0..S.len() {
        if is_lms(&T, SA[i]) {
            SA[substrings] = SA[i];
            substrings += 1;
        }
    }
    // Clear the rest of the SA so that we have scratch space.
    for i in substrings..S.len() {
        SA[i] = usize::max_value();
    }
    let mut name = 0;
    let mut prev = usize::max_value(); // sentinel used on first pass
                                       // Translate the LMS strings into lexicographically-sorted names.  At the start of this loop,
                                       // SA[i] = j, i < substrings is the index of each LMS symbol in our original text.
                                       //
                                       // We will iterate over these substrings and assign to each unique LMS-substring a number drawn
                                       // from an increasing sequence.
                                       //
                                       // At the end of this loop, we can expect the SA to be filled in with two-different
                                       // perspectives on the same subproblem:
                                       // SA = [11, 1, 9, 4, 6, 1, _, 3, 4, 2, 0, _]
                                       //      |---------------| <- LMSes from S
                                       //        subproblem -> |-------------------|
    for i in 0..substrings {
        // Position in the original text.
        let pos = SA[i];
        // Does it differ from the previous?
        let mut diff = false;
        for d in 0..S.len() {
            // If it's the first string, or the strings differ by symbol or differ by LSType.
            if prev == usize::max_value() || S[pos + d] != S[prev + d] || T[pos + d] != T[prev + d]
            {
                diff = true;
                break;
            // One of the strings terminated earlier than the other, but their symbols are the
            // same.
            } else if d > 0 && (is_lms(&T, pos + d) || is_lms(&T, prev + d)) {
                break;
            }
        }
        // Advance the char used to represent the string when the strings differ.
        if diff {
            name += 1;
            prev = pos;
        }
        // We can safely write to substrings + pos / 2 because we know substrings < SA.len() / 2
        // and we also know that the closest two LMS indices can be placed is 2 (you need an L-type
        // between two S-type characters).
        let pos = pos / 2;
        SA[substrings + pos] = name - 1
    }
    let mut j = S.len() - 1;
    // Coalesce the subproblem to one contiguous range at the end of the array.
    //
    // SA = [11, 1, 9, 4, 6, 1, _, 3, 4, 2, 0, _]
    //        subproblem -> |-------------------|
    //
    //      [11, 1, 9, 4, 6, 1, _, 1, 3, 4, 2, 0]
    //               coalesced -> !-------------|
    for i in (substrings..S.len()).rev() {
        if SA[i] != usize::max_value() {
            SA[j] = SA[i];
            j -= 1;
        }
    }
    // That coalesced form is our subproblem!  Let's call it S1:
    //
    // S1 = [1, 3, 4, 2, 0]
    let (SA1, S1) = SA.split_at_mut(substrings);
    let (_, S1) = S1.split_at_mut(S1.len() - substrings);
    if name < substrings {
        let sigma1: Sigma<usize, B> = Sigma::from_subproblem(S1);
        // If the next-to-assign name is less than the number of substrings, we know we have
        // duplicates that must be sorted relative to each other.
        sais(&sigma1, S1, SA1);
    } else {
        // Else we can trivially fill in the SA1 because we know each name was used exactly once
        // and they were created sequentially.  If S[i] = X, it means the X'th suffix in the suffix
        // array begins at i, or SA[X] = i.
        //
        // SA1 = [4, 0, 3, 1, 2]
        //
        for i in 0..S1.len() {
            SA1[S1[i]] = i;
        }
    }
    // When we created the subproblem we mapped the LMS suffixes into the contiguous range
    // [0, name), name<substrings.  Our solution will be indices into this subproblem in the range
    // [0, substrings), but the original solution must be over the original input text.
    //
    // Repurpose S1 such that S1[i] is the index of the i'th LMS character in the parent text.
    //
    // S1 = [1, 4, 6, 9, 11]
    let mut j = 0; // enumerate the substrings
    for i in 0..S.len() {
        if is_lms(&T, i) {
            S1[j] = i;
            j += 1;
        }
    }
    // Translate SA1 using S1 so that it is now over the original text, preserving the relative
    // ordering of the elements.
    //
    // before: [4, 0, 3, 1, 2]
    // after:  [11, 1, 9, 4, 6]
    for i in 0..substrings {
        SA1[i] = S1[SA1[i]];
    }
    // Here we pulled a dirty trick with references.  SA1 above was partitioned off as the prefix
    // of SA[..substrings].  From this point forward we don't refer to SA1 and assume it magics its
    // way to the prefix of SA.
    //
    // We used other parts of SA, too, so wipe those parts and make them empty:
    //
    // SA = [11, 1, 9, 4, 6, _, _, _, _, _, _, _]
    for i in substrings..S.len() {
        SA[i] = usize::max_value();
    }
    // Place the LMS strings into SA in reverse order.  We do this in reverse because the
    // subsolution (and thus the number of substrings to place) is guaranteed to be at most half of
    // the input S (you can't have a left-most S without an S and the L to its immediate right).
    // This means that each sub solution can only move to the right, never to the left.  But
    // smaller-indexed substrings could overlap the range we are using to hold the substrings, and
    // we would like to prevent that.
    //
    // After placing our LMS strings, the result looks like this:
    //
    // SA = [11, _, 1, 9, 4, 6, _, _, _, _, _, _]
    //
    // TODO(rescrv):  I do not like this example text because the SA comes out clumped.  Ideally I
    // want something with a pattern closer to [X, _, X, _, _, X, _, _].
    sigma.bucket_limits(buckets);
    for i in (0..substrings).rev() {
        j = SA[i];
        SA[i] = usize::max_value();
        buckets[S[j]] -= 1;
        SA[buckets[S[j]]] = j;
    }
    // Now that we know the LMS characters are correct, the induced sort will be correct as well.
    //
    // Here's what it looks like after the L-type pass:
    // SA = [11, _, 1, 9, 4, 6, 10, 0, 8, 3, 5, 7]
    induce_L(sigma, S, SA, &T, buckets);
    // Here's what it looks like after the S-type pass:
    // SA = [11, 1, 9, 2, 4, 6, 10, 0, 8, 3, 5, 7]
    induce_S(sigma, S, SA, &T, buckets);
}

#[cfg(test)]
mod tests {
    use crate::test_cases_for;
    use crate::testutil::TestCase;

    use super::*;

    fn check_get_types(t: &TestCase) {
        let types: Vec<LSType> = t.lstype
            .chars()
            .map(|c| if c == 'L' { LSType::L } else { LSType::S })
            .collect();
        let returned = get_types(t.S);
        assert_eq!(&types, &returned);
    }

    test_cases_for!(get_types, crate::sais::tests::check_get_types);

    fn check_is_lms(t: &TestCase) {
        let types = get_types(&t.S);
        let returned: String = types
            .iter()
            .enumerate()
            .map(|(i, _)| if is_lms(&types, i) { '*' } else { ' ' })
            .collect();
        assert_eq!(t.lmspos, returned);
    }

    test_cases_for!(is_lms, crate::sais::tests::check_is_lms);

    fn check_sais(t: &TestCase) {
        let sigma = t.sigma();
        let mut SA = vec![0; t.S.len()];
        let SA: &mut [usize] = &mut SA;
        sais(&sigma, &t.S, SA);
        assert_eq!(t.SA, SA);
    }

    test_cases_for!(sais, crate::sais::tests::check_sais);
}
