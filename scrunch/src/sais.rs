//! Suffix Array-Induced Sort (sais, for short) is an algorithm to construct the suffix array of a
//! string in linear time in the length of the string.  A suffix array contains all possible
//! suffixes of a string in sorted order such that `SA[i]` indicates that `S[SA[i]..]` would be the
//! i'th suffix in sorted order.
#![allow(non_snake_case)]

use crate::Error;
use crate::sigma::Sigma;

/// LSType is an indication of how a character relates to those charcters that follow it.  An
/// L-type character indicates the character is larger than the character that follows it.  An
/// S-type character is smaller than the character that follows it.  For purposes of comparison,
/// ties take on the type of the next character.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[cfg(test)]
enum LSType {
    L,
    S,
}

struct TypeBits {
    bits: Vec<u64>,
    lms_bits: Vec<u64>,
}

struct LmsPositions<'a> {
    bits: &'a [u64],
    word_idx: usize,
    word: u64,
}

struct LmsPositionsRev<'a> {
    bits: &'a [u64],
    word_idx: usize,
    word: u64,
}

impl Iterator for LmsPositions<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.word != 0 {
                let bit = self.word.trailing_zeros() as usize;
                self.word &= self.word - 1;
                return Some((self.word_idx - 1) * 64 + bit);
            }
            self.word = *self.bits.get(self.word_idx)?;
            self.word_idx += 1;
        }
    }
}

impl Iterator for LmsPositionsRev<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.word != 0 {
                let bit = 63 - self.word.leading_zeros() as usize;
                self.word &= !(1u64 << bit);
                return Some((self.word_idx << 6) + bit);
            }
            if self.word_idx == 0 {
                return None;
            }
            self.word_idx -= 1;
            // SAFETY(codex): word_idx starts at bits.len() and is decremented only after the
            // zero check, so the index is in-bounds; the immutable slice contains initialized
            // u64 values and this copy creates no aliasing or lifetime violation under the
            // Rustonomicon's unchecked-indexing rules.
            self.word = unsafe { *self.bits.get_unchecked(self.word_idx) };
        }
    }
}

impl TypeBits {
    fn new(len: usize) -> Self {
        Self {
            bits: vec![0u64; len.div_ceil(64)],
            lms_bits: vec![0u64; len.div_ceil(64)],
        }
    }

    #[inline(always)]
    fn is_s(&self, idx: usize) -> bool {
        // SAFETY(codex): TypeBits is allocated with ceil(input_len / 64) words and every caller
        // passes an index into the original input, so idx >> 6 is in-bounds; the read is immutable,
        // aligned, and initialized, satisfying the Rustonomicon requirements for unchecked access.
        unsafe { ((*self.bits.get_unchecked(idx >> 6) >> (idx & 63)) & 1) != 0 }
    }

    #[inline(always)]
    fn is_l(&self, idx: usize) -> bool {
        !self.is_s(idx)
    }

    #[inline(always)]
    fn is_lms(&self, idx: usize) -> bool {
        // SAFETY(codex): lms_bits has the same word layout as bits, and callers only query input
        // offsets; therefore idx >> 6 is in-bounds and the immutable initialized u64 read does not
        // violate aliasing or provenance rules described by the Rustonomicon.
        unsafe { ((*self.lms_bits.get_unchecked(idx >> 6) >> (idx & 63)) & 1) != 0 }
    }

    fn lms_positions(&self) -> LmsPositions<'_> {
        LmsPositions {
            bits: &self.lms_bits,
            word_idx: 0,
            word: 0,
        }
    }

    fn lms_positions_rev(&self) -> LmsPositionsRev<'_> {
        LmsPositionsRev {
            bits: &self.lms_bits,
            word_idx: self.lms_bits.len(),
            word: 0,
        }
    }
}

trait Symbol: Copy + Eq {
    fn to_usize(self) -> usize;
}

impl Symbol for u8 {
    #[inline(always)]
    fn to_usize(self) -> usize {
        self as usize
    }
}

impl Symbol for u16 {
    #[inline(always)]
    fn to_usize(self) -> usize {
        self as usize
    }
}

impl Symbol for u32 {
    #[inline(always)]
    fn to_usize(self) -> usize {
        self as usize
    }
}

impl Symbol for usize {
    #[inline(always)]
    fn to_usize(self) -> usize {
        self
    }
}

trait Index: Symbol + Default {
    const MAX: Self;

    fn from_usize(x: usize) -> Self;
}

impl Index for usize {
    const MAX: Self = usize::MAX;

    #[inline(always)]
    fn from_usize(x: usize) -> Self {
        x
    }
}

impl Index for u32 {
    const MAX: Self = u32::MAX;

    #[inline(always)]
    fn from_usize(x: usize) -> Self {
        debug_assert!(x < u32::MAX as usize);
        x as u32
    }
}

/// get_types creates an LSType-string from a usize-string.
#[cfg(test)]
fn get_types<Sym: Symbol>(S: &[Sym]) -> Vec<LSType> {
    let mut prev = (LSType::S, 0usize);
    let mut types = vec![LSType::S; S.len()];
    for i in (0..S.len()).rev() {
        let s = S[i].to_usize();
        prev = if s < prev.1 || (s == prev.1 && prev.0 == LSType::S) {
            (LSType::S, s)
        } else {
            (LSType::L, s)
        };
        types[i] = prev.0;
    }
    types
}

/// get_type_bits creates a packed LSType-string from a string.
fn get_type_bits<Sym: Symbol>(S: &[Sym]) -> TypeBits {
    let mut prev_is_s = true;
    let mut prev_symbol = 0usize;
    let mut types = TypeBits::new(S.len());
    if S.is_empty() {
        return types;
    }
    let mut word_idx = (S.len() - 1) >> 6;
    let mut word = 0u64;
    let mut lms_word_idx = usize::MAX;
    let mut lms_word = 0u64;
    for i in (0..S.len()).rev() {
        let next_word_idx = i >> 6;
        if next_word_idx != word_idx {
            types.bits[word_idx] = word;
            word_idx = next_word_idx;
            word = 0;
        }
        let symbol = S[i].to_usize();
        let is_s = symbol < prev_symbol || (symbol == prev_symbol && prev_is_s);
        if is_s {
            word |= 1u64 << (i & 63);
        }
        if !is_s && prev_is_s {
            let lms_idx = i + 1;
            let next_lms_word_idx = lms_idx >> 6;
            if next_lms_word_idx != lms_word_idx {
                if lms_word_idx != usize::MAX {
                    types.lms_bits[lms_word_idx] = lms_word;
                }
                lms_word_idx = next_lms_word_idx;
                lms_word = 0;
            }
            lms_word |= 1u64 << (lms_idx & 63);
        }
        prev_is_s = is_s;
        prev_symbol = symbol;
    }
    types.bits[word_idx] = word;
    if lms_word_idx != usize::MAX {
        types.lms_bits[lms_word_idx] = lms_word;
    }
    types
}

/// is_lms returns whether a particular index in the LSType-string is the left-most-S.
#[cfg(test)]
#[inline(always)]
fn is_lms(T: &[LSType], i: usize) -> bool {
    // SAFETY(codex): the left conjunct proves i > 0, and test callers enumerate i from T indices,
    // so i - 1 and i are both in-bounds; immutable initialized enum reads preserve Rust's aliasing
    // and lifetime rules as required by the Rustonomicon.
    i > 0 && unsafe { *T.get_unchecked(i - 1) == LSType::L && *T.get_unchecked(i) == LSType::S }
}

/// induce_L does a left-to-right induced sort to fill in L-type symbols.  By scanning
/// left-to-right we guarantee that we go in a strictly increasing order of suffixes.  The sort is
/// called "induced" because it scans a partially-filled suffix array and uses the partially-sorted
/// suffixes to select the next suffix of the string to sort.
///
/// And because we because we only fill in L-type suffixes, they are by definition larger than (to
/// the right of) the index used to construct the suffix.
fn induce_L<Sym: Symbol, Idx: Index, Bucket: Index>(
    bucket_starts: &[Bucket],
    S: &[Sym],
    SA: &mut [Idx],
    T: &TypeBits,
    buckets: &mut Vec<Bucket>,
) -> Result<(), Error> {
    buckets.copy_from_slice(bucket_starts);
    for i in 0..S.len() {
        // SAFETY(codex): i is produced by 0..S.len() and SA.len() == S.len(), so the immutable SA
        // access is in-bounds and reads an initialized Index without aliasing mutation, matching the
        // Rustonomicon's unchecked indexing preconditions.
        let j = unsafe { SA.get_unchecked(i).to_usize().wrapping_sub(1) };
        if j < S.len() && T.is_l(j) {
            // SAFETY(codex): j < S.len() was checked above; S is immutable and initialized, so this
            // unchecked read observes a valid symbol without violating aliasing or lifetime rules.
            let symbol = unsafe { S.get_unchecked(j).to_usize() };
            // SAFETY(codex): sais_impl validates every input symbol is less than the bucket count,
            // and recursive buckets are built from that same alphabet; this is the only mutable
            // borrow of this bucket element, so Rustonomicon bounds and aliasing rules are upheld.
            let bucket = unsafe { buckets.get_unchecked_mut(symbol) };
            let dest = bucket.to_usize();
            // SAFETY(codex): induced sorting maintains bucket cursors inside SA ranges derived from
            // bucket_starts/bucket_limits, so dest < SA.len(); this write uses the unique mutable SA
            // borrow and stores an initialized Index, satisfying Rustonomicon requirements.
            unsafe {
                *SA.get_unchecked_mut(dest) = Idx::from_usize(j);
            }
            *bucket = Bucket::from_usize(dest + 1);
        }
    }
    Ok(())
}

/// induce_S does a right-to-left induced sort to fill in S-type symbols.  The reasoning for what
/// this does is similar to how induce_L works, except it works right-to-left, is monotonically
/// decreasing, and fills buckets from the right.
fn induce_S<Sym: Symbol, Idx: Index, Bucket: Index>(
    bucket_limits: &[Bucket],
    S: &[Sym],
    SA: &mut [Idx],
    T: &TypeBits,
    buckets: &mut Vec<Bucket>,
) -> Result<(), Error> {
    buckets.copy_from_slice(bucket_limits);
    for i in (0..S.len()).rev() {
        // SAFETY(codex): i is produced by a range bounded by S.len(), and SA.len() == S.len(), so
        // this initialized immutable read is in-bounds and does not conflict with mutation.
        let j = unsafe { SA.get_unchecked(i).to_usize().wrapping_sub(1) };
        if j < S.len() && T.is_s(j) {
            // SAFETY(codex): the branch checked j < S.len(); reading S[j] immutably is in-bounds,
            // initialized, and follows the Rustonomicon aliasing rules for shared references.
            let symbol = unsafe { S.get_unchecked(j).to_usize() };
            // SAFETY(codex): symbols are validated against the bucket count before sorting, and the
            // S-pass cursor is initialized from bucket_limits; mutating one bucket slot through the
            // only active mutable borrow satisfies Rustonomicon bounds and aliasing requirements.
            let dest = unsafe {
                let bucket = buckets.get_unchecked_mut(symbol);
                let dest = bucket.to_usize() - 1;
                *bucket = Bucket::from_usize(dest);
                dest
            };
            // SAFETY(codex): dest is the decremented bucket cursor, which stays within the suffix
            // array bucket range by the induced-sort invariant; the unique mutable SA borrow makes
            // this initialized write valid under the Rustonomicon.
            unsafe {
                *SA.get_unchecked_mut(dest) = Idx::from_usize(j);
            }
        }
    }
    Ok(())
}

fn bucket_starts_and_limits<Sym: Symbol, Idx: Index>(
    k: usize,
    s: &[Sym],
) -> Result<(Vec<Idx>, Vec<Idx>), Error> {
    let mut bucket_limits = vec![Idx::from_usize(0); k];
    for &symbol in s {
        let symbol = symbol.to_usize();
        if symbol >= k {
            return Err(Error::InvalidSigma);
        }
        bucket_limits[symbol] = Idx::from_usize(bucket_limits[symbol].to_usize() + 1);
    }
    let mut sum = 0usize;
    for limit in bucket_limits.iter_mut() {
        sum += limit.to_usize();
        *limit = Idx::from_usize(sum);
    }
    let mut bucket_starts = vec![Idx::from_usize(0); k];
    let mut prev = 0usize;
    for (idx, limit) in bucket_limits.iter().copied().enumerate() {
        bucket_starts[idx] = Idx::from_usize(prev);
        prev = limit.to_usize();
    }
    Ok((bucket_starts, bucket_limits))
}

fn sais_impl<Sym: Symbol, Idx: Index, Bucket: Index>(
    bucket_starts: &[Bucket],
    bucket_limits: &[Bucket],
    S: &[Sym],
    SA: &mut [Idx],
) -> Result<(), Error> {
    assert_eq!(bucket_starts.len(), bucket_limits.len());
    assert!(
        bucket_limits
            .last()
            .copied()
            .map(|x| x.to_usize())
            .unwrap_or(0)
            == S.len()
    );
    assert!(bucket_starts.len() <= S.len());
    assert!(S.len() == SA.len());
    assert!(!S.is_empty());
    assert_eq!(S[S.len() - 1].to_usize(), 0);
    if S.iter()
        .any(|symbol| symbol.to_usize() >= bucket_starts.len())
    {
        return Err(Error::InvalidSigma);
    }

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
    let T = get_type_bits(S);

    // 2.  Compute the "buckets" for the string and place the LMS characters at the end of their
    //     respective buckets.
    //
    // In our example, we would divide the suffix array like this:
    // SA     = [11, 1, 9, 2, 4, 6, 10, 0, 8, 3, 5, 7]
    // S[SA]  = [ $, a, a, a, a, a,  b, b, b, b, b, b]
    // Bucket = [ 0, 1, 1, 1, 1, 1,  2, 2, 2, 2, 2, 2]
    let mut buckets = bucket_limits.to_vec();
    buckets.copy_from_slice(bucket_limits);
    // We will use usize::MAX as a sentinel to mean "empty" throughout the computation.
    // Place the LMSes at the end of their respective buckets.
    // After this loop, SA will look like this:
    // SA = [11, _, 9, 6, 4, 1, _, _, _, _, _, _]
    SA.fill(Index::MAX);
    for i in T.lms_positions() {
        // SAFETY(codex): lms_positions only yields bits that were set from valid input offsets, so
        // i < S.len(); this immutable initialized symbol read is in-bounds and alias-safe.
        let symbol = unsafe { S.get_unchecked(i).to_usize() };
        // SAFETY(codex): sais_impl validated symbols are bucket indices, and placing LMS entries
        // decrements cursors inside bucket_limits; the single mutable bucket borrow satisfies the
        // Rustonomicon's exclusivity rule.
        let dest = unsafe {
            let bucket = buckets.get_unchecked_mut(symbol);
            let dest = bucket.to_usize() - 1;
            *bucket = Bucket::from_usize(dest);
            dest
        };
        // SAFETY(codex): dest comes from a valid bucket cursor for the suffix array and therefore
        // is less than SA.len(); writing through the unique mutable SA borrow preserves aliasing and
        // initialization invariants required by the Rustonomicon.
        unsafe {
            *SA.get_unchecked_mut(dest) = Idx::from_usize(i);
        }
    }

    // 3.  Induce sort: left-to-right for L-type suffixes then right-to-left for S-type suffixes.
    //
    // Do a left-to-right induced-L pass:
    // SA = [11, _, 9, 6, 4, 1, 10, 8, 5, 3, 0, 7]
    induce_L(bucket_starts, S, SA, &T, &mut buckets)?;

    // And then an induce-S pass:
    // SA = [11, 1, 9, 4, 2, 6, 10, 8, 5, 3, 0, 7]
    // Note how this differs from the expected SA:
    // SA = [11, 1, 9, 2, 4, 6, 10, 0, 8, 3, 5, 7]
    induce_S(bucket_limits, S, SA, &T, &mut buckets)?;

    // 3.  Construct a subproblem using the LMS suffixes.
    let mut substrings = 0usize;

    // First, collect the LMS suffixes in the order they appear in the almost-sorted SA.  After
    // this loop:
    // SA[..substrings] = [11, 1, 9, 4, 6]
    for i in 0..S.len() {
        // SAFETY(codex): i ranges over 0..S.len() and SA.len() == S.len(); SA has been filled with
        // initialized sentinels or offsets, so this immutable read is in-bounds and alias-safe.
        let pos = unsafe { SA.get_unchecked(i).to_usize() };
        if T.is_lms(pos) {
            // SAFETY(codex): the same range proof as above makes SA[i] in-bounds and initialized;
            // copying the value does not create aliasing or lifetime issues.
            let value = unsafe { *SA.get_unchecked(i) };
            // SAFETY(codex): substrings counts LMS entries already observed in an S.len() scan, so
            // substrings <= i < SA.len(); writing the compacted prefix uses the unique mutable SA
            // borrow and stores an initialized value.
            unsafe {
                *SA.get_unchecked_mut(substrings) = value;
            }
            substrings += 1;
        }
    }

    // Clear the rest of the SA so that we have scratch space.
    SA[substrings..].fill(Index::MAX);

    let mut previous_lms = usize::MAX;
    for i in T.lms_positions() {
        if previous_lms != usize::MAX {
            // SAFETY(codex): LMS positions are at least two apart, so previous_lms / 2 indexes the
            // scratch region reserved at SA[substrings..]; substrings + previous_lms / 2 < SA.len()
            // by the SA-IS layout invariant, and the write is via the unique mutable SA borrow.
            unsafe {
                *SA.get_unchecked_mut(substrings + previous_lms / 2) =
                    Idx::from_usize(i - previous_lms + 1);
            }
        }
        previous_lms = i;
    }
    if previous_lms != usize::MAX {
        // SAFETY(codex): previous_lms is a valid LMS position and the same scratch-region layout as
        // above ensures substrings + previous_lms / 2 is in-bounds; this initialized write has no
        // competing mutable or shared alias.
        unsafe {
            *SA.get_unchecked_mut(substrings + previous_lms / 2) =
                Idx::from_usize(S.len() - previous_lms);
        }
    }

    // sentinel used on first pass
    let mut name = 0usize;
    let mut prev = usize::MAX;
    let mut prev_len = 0usize;

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
        // SAFETY(codex): i < substrings and substrings <= SA.len(); this prefix contains collected
        // LMS positions, all initialized, so the immutable read is in-bounds and alias-safe.
        let pos: usize = unsafe { SA.get_unchecked(i).to_usize() };
        // SAFETY(codex): pos is an LMS position and the substring-length table was written at
        // substrings + pos / 2 using the SA-IS scratch layout; that index is in-bounds and contains
        // an initialized length.
        let pos_len = unsafe { SA.get_unchecked(substrings + pos / 2).to_usize() };
        // Does it differ from the previous?
        let mut diff = prev == usize::MAX;
        if !diff {
            for d in 0..pos_len.min(prev_len) {
                let pos_d = pos + d;
                let prev_d = prev + d;
                // If the strings differ by symbol or differ by LSType.
                // SAFETY(codex): d is bounded by the precomputed LMS substring lengths, so pos_d
                // and prev_d remain within S and TypeBits; only immutable initialized reads occur,
                // which satisfies the Rustonomicon's bounds and aliasing requirements.
                if unsafe {
                    S.get_unchecked(pos_d) != S.get_unchecked(prev_d)
                        || T.is_s(pos_d) != T.is_s(prev_d)
                } {
                    diff = true;
                    break;
                }
            }
        }
        // Advance the char used to represent the string when the strings differ.
        if diff {
            name += 1;
            prev = pos;
            prev_len = pos_len;
        }
        // We can safely write to substrings + pos / 2 because we know substrings < SA.len() / 2
        // and we also know that the closest two LMS indices can be placed is 2 (you need an L-type
        // between two S-type characters).
        let pos = pos / 2;
        SA[substrings + pos] = Idx::from_usize(name - 1);
    }

    // Coalesce the subproblem to one contiguous range at the end of the array.
    //
    // SA = [11, 1, 9, 4, 6, 1, _, 3, 4, 2, 0, _]
    //        subproblem -> |-------------------|
    //
    //      [11, 1, 9, 4, 6, 1, _, 1, 3, 4, 2, 0]
    //               coalesced -> !-------------|
    let mut coalesce = S.len();
    for i in T.lms_positions_rev() {
        coalesce -= 1;
        // SAFETY(codex): lms_positions_rev yields valid LMS offsets, so substrings + i / 2 indexes
        // the initialized subproblem name in the scratch region; coalesce moves within the final
        // substrings slots at the end of SA. The source read and destination write are in-bounds,
        // and the unique mutable SA borrow upholds Rustonomicon aliasing rules.
        unsafe {
            *SA.get_unchecked_mut(coalesce) = *SA.get_unchecked(substrings + i / 2);
        }
    }
    debug_assert_eq!(coalesce, S.len() - substrings);

    // That coalesced form is our subproblem!  Let's call it S1:
    //
    // S1 = [1, 3, 4, 2, 0]
    let (SA1, S1) = SA.split_at_mut(substrings);
    let (_, S1) = S1.split_at_mut(S1.len() - substrings);
    if name < substrings {
        let (bucket_starts1, bucket_limits1) = bucket_starts_and_limits::<_, Bucket>(name, S1)?;
        sais_impl(&bucket_starts1, &bucket_limits1, S1, SA1)?;
    } else {
        // Else we can trivially fill in the SA1 because we know each name was used exactly once
        // and they were created sequentially.  If S[i] = X, it means the X'th suffix in the suffix
        // array begins at i, or SA[X] = i.
        //
        // SA1 = [4, 0, 3, 1, 2]
        for i in 0..S1.len() {
            SA1[S1[i].to_usize()] = Idx::from_usize(i);
        }
    }

    // When we created the subproblem we mapped the LMS suffixes into the contiguous range
    // [0, name), name<substrings.  Our solution will be indices into this subproblem in the range
    // [0, substrings), but the original solution must be over the original input text.
    //
    // Repurpose S1 such that S1[i] is the index of the i'th LMS character in the parent text.
    //
    // S1 = [1, 4, 6, 9, 11]
    let mut j = 0;
    for i in T.lms_positions() {
        // SAFETY(codex): T has exactly substrings LMS positions and S1.len() == substrings, so
        // j is in-bounds for each iteration; S1 is uniquely borrowed as mutable and receives an
        // initialized Index value.
        unsafe {
            *S1.get_unchecked_mut(j) = Idx::from_usize(i);
        }
        j += 1;
    }

    // Translate SA1 using S1 so that it is now over the original text, preserving the relative
    // ordering of the elements.
    //
    // before: [4, 0, 3, 1, 2]
    // after:  [11, 1, 9, 4, 6]
    for i in 0..substrings {
        // SAFETY(codex): i < SA1.len(), and recursive SA1 values are valid indices into S1; both
        // slices are initialized and the immutable reads obey Rustonomicon bounds and aliasing
        // rules.
        let value = unsafe { *S1.get_unchecked(SA1.get_unchecked(i).to_usize()) };
        // SAFETY(codex): i < SA1.len() and SA1 is uniquely borrowed mutably, so writing the
        // translated initialized value is in-bounds with no aliasing violation.
        unsafe {
            *SA1.get_unchecked_mut(i) = value;
        }
    }

    // Here we pulled a dirty trick with references.  SA1 above was partitioned off as the prefix
    // of SA[..substrings].  From this point forward we don't refer to SA1 and assume it magics its
    // way to the prefix of SA.
    //
    // We used other parts of SA, too, so wipe those parts and make them empty:
    //
    // SA = [11, 1, 9, 4, 6, _, _, _, _, _, _, _]
    SA[substrings..].fill(Index::MAX);
    buckets.copy_from_slice(bucket_limits);

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
    for i in (0..substrings).rev() {
        // SAFETY(codex): i < substrings <= SA.len() and the prefix contains initialized LMS
        // positions after translating SA1, so this immutable read is in-bounds and alias-safe.
        j = unsafe { SA.get_unchecked(i).to_usize() };
        // SAFETY(codex): i is in-bounds and SA is uniquely borrowed mutably; replacing this prefix
        // slot with the initialized sentinel preserves Rustonomicon aliasing and initialization
        // rules.
        unsafe {
            *SA.get_unchecked_mut(i) = Index::MAX;
        }
        // SAFETY(codex): j is an LMS offset from the original text, hence j < S.len(); S is an
        // immutable initialized slice and this read is in-bounds.
        let symbol = unsafe { S.get_unchecked(j).to_usize() };
        // SAFETY(codex): the symbol domain was validated, and reverse LMS placement decrements
        // bucket cursors within valid suffix-array bucket ranges; the single mutable bucket borrow
        // satisfies Rustonomicon exclusivity requirements.
        let dest = unsafe {
            let bucket = buckets.get_unchecked_mut(symbol);
            let dest = bucket.to_usize() - 1;
            *bucket = Bucket::from_usize(dest);
            dest
        };
        // SAFETY(codex): dest is a valid bucket position in SA by the bucket cursor invariant; this
        // initialized write uses the unique mutable SA borrow and therefore respects aliasing rules.
        unsafe {
            *SA.get_unchecked_mut(dest) = Idx::from_usize(j);
        }
    }

    // Now that we know the LMS characters are correct, the induced sort will be correct as well.
    //
    // Here's what it looks like after the L-type pass:
    // SA = [11, _, 1, 9, 4, 6, 10, 0, 8, 3, 5, 7]
    induce_L(bucket_starts, S, SA, &T, &mut buckets)?;
    // Here's what it looks like after the S-type pass:
    // SA = [11, 1, 9, 2, 4, 6, 10, 0, 8, 3, 5, 7]
    induce_S(bucket_limits, S, SA, &T, &mut buckets)?;
    Ok(())
}

/// sais is the Suffix-Array Induced-Sort.  This algorithm uses induced-sort (similar technique is
/// used in radix sort) to produce a suffix array in time linear in the length of the input.
///
/// # Panics:
/// - K > S.len()
/// - S.len() != SA.len()
/// - S.len() == 0
/// - S is not termintated by a zero
pub fn sais(sigma: &Sigma, S: &[u32], SA: &mut [usize]) -> Result<(), Error> {
    // We need some space for sentinels in the lang.
    assert!(sigma.K() <= S.len());
    // The input and "output" must be aligned.
    assert!(S.len() == SA.len());
    // The last character should be zero, which also requires there to be a last character.
    assert!(!S.is_empty());
    assert_eq!(S[S.len() - 1], 0);

    let mut bucket_starts = vec![0usize; sigma.K()];
    sigma.bucket_starts(&mut bucket_starts)?;
    let mut bucket_limits = vec![0usize; sigma.K()];
    sigma.bucket_limits(&mut bucket_limits)?;
    sais_impl(&bucket_starts, &bucket_limits, S, SA)
}

fn sais_u32_index<Sym: Symbol>(sigma: &Sigma, S: &[Sym], SA: &mut [u32]) -> Result<(), Error> {
    assert!(S.len() < u32::MAX as usize);
    // We need some space for sentinels in the lang.
    assert!(sigma.K() <= S.len());
    // The input and "output" must be aligned.
    assert!(S.len() == SA.len());
    // The last character should be zero, which also requires there to be a last character.
    assert!(!S.is_empty());
    assert_eq!(S[S.len() - 1].to_usize(), 0);

    let mut bucket_starts_usize = vec![0usize; sigma.K()];
    sigma.bucket_starts(&mut bucket_starts_usize)?;
    let bucket_starts: Vec<u32> = bucket_starts_usize
        .into_iter()
        .map(u32::try_from)
        .collect::<Result<_, _>>()?;
    let mut bucket_limits_usize = vec![0usize; sigma.K()];
    sigma.bucket_limits(&mut bucket_limits_usize)?;
    let bucket_limits: Vec<u32> = bucket_limits_usize
        .into_iter()
        .map(u32::try_from)
        .collect::<Result<_, _>>()?;
    sais_impl(&bucket_starts, &bucket_limits, S, SA)
}

/// sais_u8_u32 constructs a suffix array using u32 offsets and u8 symbols.
pub fn sais_u8_u32(sigma: &Sigma, S: &[u8], SA: &mut [u32]) -> Result<(), Error> {
    sais_u32_index(sigma, S, SA)
}

/// sais_u16_u32 constructs a suffix array using u32 offsets and u16 symbols.
pub fn sais_u16_u32(sigma: &Sigma, S: &[u16], SA: &mut [u32]) -> Result<(), Error> {
    sais_u32_index(sigma, S, SA)
}

/// sais_u32 constructs a suffix array using u32 offsets for inputs shorter than u32::MAX.
pub fn sais_u32(sigma: &Sigma, S: &[u32], SA: &mut [u32]) -> Result<(), Error> {
    sais_u32_index(sigma, S, SA)
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use buffertk::Unpackable;

    use super::super::test_cases_for;
    use super::super::test_util::TestCase;

    use super::*;

    fn sigma_for_text(text: &[u32]) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut builder = crate::builder::Builder::new(&mut buf);
        Sigma::construct(text.iter().copied(), &mut builder).expect("sigma should construct");
        drop(builder);
        buf
    }

    fn naive_suffix_array<Sym: Ord>(S: &[Sym]) -> Vec<usize> {
        let mut SA: Vec<usize> = (0..S.len()).collect();
        SA.sort_by(|&lhs, &rhs| S[lhs..].cmp(&S[rhs..]));
        SA
    }

    fn check_sais_u8_u32(t: &TestCase) {
        let sigma = t.sigma();
        let sigma = Sigma::unpack(&sigma).expect("test should unpack").0;
        let S: Vec<u8> = t.S.iter().map(|x| u8::try_from(*x).unwrap()).collect();
        let mut SA = vec![0u32; t.S.len()];
        sais_u8_u32(&sigma, &S, &mut SA).unwrap();
        let expected: Vec<u32> = t.SA.iter().map(|x| *x as u32).collect();
        assert_eq!(expected, SA);
    }

    test_cases_for!(sais_u8_u32, super::check_sais_u8_u32);

    fn check_sais_u16_u32(t: &TestCase) {
        let sigma = t.sigma();
        let sigma = Sigma::unpack(&sigma).expect("test should unpack").0;
        let S: Vec<u16> = t.S.iter().map(|x| u16::try_from(*x).unwrap()).collect();
        let mut SA = vec![0u32; t.S.len()];
        sais_u16_u32(&sigma, &S, &mut SA).unwrap();
        let expected: Vec<u32> = t.SA.iter().map(|x| *x as u32).collect();
        assert_eq!(expected, SA);
    }

    test_cases_for!(sais_u16_u32, super::check_sais_u16_u32);

    fn check_sais_u32(t: &TestCase) {
        let sigma = t.sigma();
        let sigma = Sigma::unpack(&sigma).expect("test should unpack").0;
        let mut SA = vec![0u32; t.S.len()];
        sais_u32(&sigma, t.S, &mut SA).unwrap();
        let expected: Vec<u32> = t.SA.iter().map(|x| *x as u32).collect();
        assert_eq!(expected, SA);
    }

    test_cases_for!(sais_u32, super::check_sais_u32);

    #[test]
    fn invalid_symbol_rejected() {
        let sigma = sigma_for_text(&[1, 2]);
        let sigma = Sigma::unpack(&sigma).expect("test should unpack").0;
        let mut SA = vec![0usize; 3];
        assert_eq!(Err(Error::InvalidSigma), sais(&sigma, &[1, 3, 0], &mut SA));
        let mut SA = vec![0u32; 3];
        assert_eq!(
            Err(Error::InvalidSigma),
            sais_u8_u32(&sigma, &[1, 3, 0], &mut SA)
        );
    }

    #[test]
    fn u16_symbols_above_u8() {
        let text: Vec<u32> = (0..600).map(|idx| idx * 37 % 300 + 1).collect();
        let sigma = sigma_for_text(&text);
        let sigma = Sigma::unpack(&sigma).expect("test should unpack").0;
        let mut S: Vec<u16> = text
            .iter()
            .map(|t| sigma.char_to_sigma(*t).unwrap() as u16)
            .collect();
        S.push(0);
        let mut SA = vec![0u32; S.len()];
        sais_u16_u32(&sigma, &S, &mut SA).unwrap();
        let expected: Vec<u32> = naive_suffix_array(&S)
            .into_iter()
            .map(|x| x as u32)
            .collect();
        assert_eq!(expected, SA);
    }

    fn check_get_types(t: &TestCase) {
        let types: Vec<LSType> = t
            .lstype
            .chars()
            .map(|c| if c == 'L' { LSType::L } else { LSType::S })
            .collect();
        let returned = get_types(t.S);
        assert_eq!(&types, &returned);
    }

    test_cases_for!(get_types, super::check_get_types);

    fn check_is_lms(t: &TestCase) {
        let types = get_types(t.S);
        let returned: String = types
            .iter()
            .enumerate()
            .map(|(i, _)| if is_lms(&types, i) { '*' } else { ' ' })
            .collect();
        assert_eq!(t.lmspos, returned);
    }

    test_cases_for!(is_lms, super::check_is_lms);

    fn check_sais(t: &TestCase) {
        let sigma = t.sigma();
        let sigma = Sigma::unpack(&sigma).expect("test should unpack").0;
        let mut SA = vec![0usize; t.S.len()];
        sais(&sigma, t.S, &mut SA).unwrap();
        assert_eq!(t.SA, SA);
    }

    test_cases_for!(sais, super::check_sais);
}
