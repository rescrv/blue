use std::hash::Hash;

pub mod bit_array;
pub mod bit_vector;
pub mod dictionary;
pub mod psi;
pub mod reference;
pub mod sais;
pub mod sigma;
pub mod wavelet_tree;

pub trait Index {
    type Item;

    fn length(&self) -> usize;
    fn extract<'a>(
        &'a self,
        idx: usize,
        count: usize,
    ) -> Option<Box<dyn Iterator<Item = Self::Item> + 'a>>;
    fn search<'a>(&'a self, needle: &'a [Self::Item]) -> Box<dyn Iterator<Item = usize> + 'a>;
    // Count the number of times needle appears in the original text.
    //
    // In a simple implementation this could be implemented as self.search(needle).count(), but it
    // is explicitly part of the API as certain structures can perform count much more efficiently
    // than search.
    fn count(&self, needle: &[Self::Item]) -> usize;
}

pub struct SearchIndex<T, SA, ISA, C, P>
where
    T: Copy + Clone + Eq + Hash + Ord,
    SA: SuffixArray,
    ISA: InverseSuffixArray,
    C: bit_vector::BitVector,
    P: psi::Psi,
{
    // Translate indices to characters.
    sigma: Sigma<T, C>,

    // Sorted array of all suffixes of the original text.
    //
    // Each entry sa[i] in the suffix array indicates the offset in the original text at which the
    // i'th prefix begins.  Compactly: i<j => text[sa[i]..] < text[sa[j]..].
    sa: SA,

    // Inverse suffix array.
    //
    // Each entry isa[i] tells the position of text[i..] in the suffix array.
    isa: ISA,

    // Psi is the successor function applied to the suffix array.
    // Using psi, you can traverse the suffix array because sa, isa, psi are all related:
    //
    // psi[idx] = isa[sa[idx] + 1]
    //
    // See [DGA] for a more thorough understanding.
    psi: P,
}

// XXX document
pub trait SuffixArray {
    fn new(sa: &[usize]) -> Self;
    fn lookup(&self, idx: usize) -> usize;
}

// XXX document
pub trait InverseSuffixArray {
    fn new(isa: &[usize]) -> Self;
    fn lookup(&self, idx: usize) -> usize;
}

pub struct UncompressedSuffixArray {
    sa: Vec<usize>,
}

impl SuffixArray for UncompressedSuffixArray {
    fn new(sa: &[usize]) -> Self {
        Self { sa: sa.to_vec() }
    }

    fn lookup(&self, idx: usize) -> usize {
        self.sa[idx]
    }
}

pub struct UncompressedInverseSuffixArray {
    isa: Vec<usize>,
}

impl InverseSuffixArray for UncompressedInverseSuffixArray {
    fn new(isa: &[usize]) -> Self {
        Self { isa: isa.to_vec() }
    }

    fn lookup(&self, idx: usize) -> usize {
        self.isa[idx]
    }
}

impl<T, SA, ISA, SIG, PSI> SearchIndex<T, SA, ISA, SIG, PSI>
where
    T: Copy + Clone + Eq + Hash + Ord,
    SA: SuffixArray,
    ISA: InverseSuffixArray,
    SIG: bit_vector::BitVector,
    PSI: psi::Psi,
{
    pub fn new(input: &[T]) -> SearchIndex<T, SA, ISA, SIG, PSI> {
        let sigma: Sigma<T, SIG> = Sigma::from(input);

        // convert the text into a compacted alphabet
        let (text, _) = sigma::to_compact_alphabet(input);
        let text: &[usize] = &text;
        // compute the suffix array
        let mut sa: Vec<usize> = Vec::with_capacity(text.len());
        sa.resize(text.len(), 0);
        sais::sais(&sigma, &text, &mut sa);
        let sa: &[usize] = &sa;
        // compute the inverse suffix array
        let isa: &[usize] = &inverse(sa);
        // compute the successor array, psi
        let mut psi: Vec<usize> = Vec::with_capacity(text.len());
        psi.resize(text.len(), 0);
        psi[isa[isa.len() - 1]] = isa[0];
        for i in 1..isa.len() {
            psi[isa[i - 1]] = isa[i];
        }
        let psi: &[usize] = &psi;
        // create the overall search index
        let sa = SA::new(sa);
        let isa = ISA::new(isa);
        let psi = PSI::new(&sigma, psi);
        SearchIndex {
            sigma,
            sa,
            isa,
            psi,
        }
    }

    fn backwards_search(&self, needle: &[T]) -> (usize, usize) {
        // If there's no needle, we should return everything except the artificial end marker.
        if needle.len() == 0 {
            return (1, self.psi.len());
        }
        let mut range = (0, self.psi.len());
        // This performs backwards search as described in [AB].  It's not immediately obviously
        // that this will actually search for what we want to find, but :shrug:.
        // TODO(rescrv): shrug?
        for i in (0..needle.len()).rev() {
            range = self
                .psi
                .constrain(&self.sigma, self.sigma.sa_range_for(needle[i]), range);
        }
        range
    }
}

impl<T, SA, ISA, C, P> Index for SearchIndex<T, SA, ISA, C, P>
where
    T: Copy + Clone + Eq + Hash + Ord,
    SA: SuffixArray,
    ISA: InverseSuffixArray,
    C: bit_vector::BitVector,
    P: psi::Psi,
{
    type Item = T;

    fn length(&self) -> usize {
        // psi always includes an implicit end character that we strip
        self.psi.len() - 1
    }

    fn extract<'a>(
        &'a self,
        idx: usize,
        count: usize,
    ) -> Option<Box<dyn Iterator<Item = Self::Item> + 'a>> {
        if idx > self.length() || idx + count > self.length() {
            return None;
        }
        let mut idx = self.isa.lookup(idx);
        let mut result = Vec::with_capacity(count);
        for _ in 0..count {
            match self.sigma.sa_index_to_t(idx) {
                Some(x) => result.push(x),
                None => panic!("XXX"),
            }
            idx = self.psi.lookup(&self.sigma, idx);
        }
        // XXX make it lazy
        Some(Box::new(result.into_iter()))
    }

    fn search<'a>(&'a self, needle: &'a [Self::Item]) -> Box<dyn Iterator<Item = usize> + 'a> {
        let range = self.backwards_search(needle);
        let mut result = Vec::with_capacity(range.1 - range.0);
        for offset in range.0..range.1 {
            result.push(self.sa.lookup(offset));
        }
        result.sort_unstable();
        Box::new(result.into_iter())
    }

    fn count(&self, needle: &[Self::Item]) -> usize {
        let range = self.backwards_search(needle);
        range.1 - range.0
    }
}

fn inverse(x: &[usize]) -> Vec<usize> {
    let mut ix: Vec<usize> = Vec::with_capacity(x.len());
    ix.resize(x.len(), 0);
    for i in 0..x.len() {
        ix[x[i]] = i
    }
    ix
}

pub type Sigma<T, B> = crate::sigma::Sigma<T, B>;

#[cfg(test)]
pub mod testutil {
    use crate::bit_vector::ReferenceBitVector;

    use super::*;

    // NOTE(rescrv):  Verbose, possibly duplicating information, so that there's a canonical place
    // to look for the examples of different representations.  Also serves as a human cross-check.
    // Static, possibly to a point of inconvenience, to make the most easy thing to do to create
    // top-level constants.  These are both done to serve human readers well and force human
    // writers to think about the test case and generate it by hand.
    #[allow(non_snake_case)]
    pub struct TestCase {
        pub text: &'static str,
        pub sigma2text: &'static [char],
        pub boundaries: &'static [usize],
        pub not_in_str: &'static [char],
        pub S: &'static [usize],
        pub SA: &'static [usize],
        pub bucket_starts: &'static [usize],
        pub bucket_limits: &'static [usize],
        pub deref_SA: &'static [usize],
        pub lstype: &'static str,
        pub lmspos: &'static str,
    }

    impl TestCase {
        pub fn sigma(&self) -> Sigma<char, ReferenceBitVector> {
            self.text.chars().collect()
        }
    }

    pub const BANANA: &TestCase = &TestCase {
        text: "BANANA",
        sigma2text: &['A', 'B', 'N'],
        boundaries: &[1, 4, 5, 7],
        not_in_str: &['C', 'D', 'E'],
        S: &[2, 1, 3, 1, 3, 1, 0],
        SA: &[6, 5, 3, 1, 0, 4, 2],
        bucket_starts: &[0, 1, 4, 5],
        bucket_limits: &[1, 4, 5, 7],
        deref_SA: &[0, 1, 1, 1, 2, 3, 3],
        lstype: "LSLSLLS",
        lmspos: " * *  *",
    };

    pub const MISSISSIPPI: &TestCase = &TestCase {
        text: "MISSISSIPPI",
        sigma2text: &['I', 'M', 'P', 'S'],
        boundaries: &[1, 5, 6, 8, 12],
        not_in_str: &['A', 'B', 'N'],
        S: &[2, 1, 4, 4, 1, 4, 4, 1, 3, 3, 1, 0],
        SA: &[11, 10, 7, 4, 1, 0, 9, 8, 6, 3, 5, 2],
        bucket_starts: &[0, 1, 5, 6, 8],
        bucket_limits: &[1, 5, 6, 8, 12],
        deref_SA: &[0, 1, 1, 1, 1, 2, 3, 3, 4, 4, 4, 4],
        lstype: "LSLLSLLSLLLS",
        lmspos: " *  *  *   *",
    };

    pub const MISSISSIPPI_BANANA: &TestCase = &TestCase {
        text: "MISSISSIPPIBANANA",
        sigma2text: &['A', 'B', 'I', 'M', 'N', 'P', 'S'],
        boundaries: &[1, 4, 5, 9, 10, 12, 14, 18],
        not_in_str: &['C', 'D', 'E'],
        S: &[4, 3, 7, 7, 3, 7, 7, 3, 6, 6, 3, 2, 1, 5, 1, 5, 1, 0],
        SA: &[17, 16, 14, 12, 11, 10, 7, 4, 1, 0, 15, 13, 9, 8, 6, 3, 5, 2],
        bucket_starts: &[0, 1, 4, 5, 9, 10, 12, 14],
        bucket_limits: &[1, 4, 5, 9, 10, 12, 14, 18],
        deref_SA: &[0, 1, 1, 1, 2, 3, 3, 3, 3, 4, 5, 5, 6, 6, 7, 7, 7, 7],
        lstype: "LSLLSLLSLLLLSLSLLS",
        lmspos: " *  *  *    * *  *",
    };

    pub const MIISSISSISSIPPI: &TestCase = &TestCase {
        text: "MIISSISSISSIPPI",
        sigma2text: &['I', 'M', 'P', 'S'],
        boundaries: &[1, 7, 8, 10, 16],
        not_in_str: &['A', 'B', 'N'],
        S: &[2, 1, 1, 4, 4, 1, 4, 4, 1, 4, 4, 1, 3, 3, 1, 0],
        SA: &[15, 14, 1, 11, 8, 5, 2, 0, 13, 12, 10, 7, 4, 9, 6, 3],
        bucket_starts: &[0, 1, 7, 8, 10],
        bucket_limits: &[1, 7, 8, 10, 16],
        deref_SA: &[0, 1, 1, 1, 1, 1, 1, 2, 3, 3, 4, 4, 4, 4, 4, 4],
        lstype: "LSSLLSLLSLLSLLLS",
        lmspos: " *   *  *  *   *",
    };

    // Thist test case was found in early prototype of sais.  It has survived a language rewrite
    // and really only exists because it makes me laugh.  I'd love to replace it with a minimal
    // test case demonstrating all of the following:
    // - identical S+LS blocks ordered in SA the way they are ordered in the input
    // - identical S+LS blocks reversed in SA from the way they are ordered in the input
    // - S+LS block that whose S+L prefix is shared by multiple S+L strings, also forward/reverse
    pub const MUTANT_BANANA: &TestCase = &TestCase {
        text: "ANBNANANCANBNCNANANCNA",
        sigma2text: &['A', 'B', 'C', 'N'],
        boundaries: &[1, 8, 10, 13, 23],
        not_in_str: &['D', 'E', 'F'],
        S: &[
            1, 4, 2, 4, 1, 4, 1, 4, 3, 1, 4, 2, 4, 3, 4, 1, 4, 1, 4, 3, 4, 1, 0,
        ],
        SA: &[
            22, 21, 4, 15, 0, 9, 6, 17, 2, 11, 8, 19, 13, 20, 3, 14, 5, 16, 1, 10, 7, 18, 12,
        ],
        bucket_starts: &[0, 1, 8, 10, 13],
        bucket_limits: &[1, 8, 10, 13, 23],
        deref_SA: &[
            0, 1, 1, 1, 1, 1, 1, 1, 2, 2, 3, 3, 3, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        ],
        lstype: "SLSLSLSLLSLSLSLSLSLSLLS",
        lmspos: "  * * *  * * * * * *  *",
    };

    #[macro_export]
    macro_rules! test_cases_for {
        ($name:ident, $check:path) => {
            pub mod $name {
                #[test]
                fn banana() {
                    $check($crate::testutil::BANANA);
                }

                #[test]
                fn mississippi() {
                    $check($crate::testutil::MISSISSIPPI);
                }

                #[test]
                fn mississippi_banana() {
                    $check($crate::testutil::MISSISSIPPI_BANANA);
                }

                #[test]
                fn miissississippi() {
                    $check($crate::testutil::MIISSISSISSIPPI);
                }

                #[test]
                fn mutant_banana() {
                    $check($crate::testutil::MUTANT_BANANA);
                }
            }
        };
    }

    pub const TODO_BANANA: &str = "banana";
    pub const TODO_MISSISSIPPI: &str = "mississippi";
    pub const TODO_MISSISSIPPIBANANA: &str = "mississippibanana";
    pub const TODO_MIISSISSISSIPPI: &str = "miissississippi";
    pub const TODO_MUTANT_BANANA: &str = "anbnanancanbncnanancna";

    pub struct SearchResult<'a> {
        pub input: &'a str,
        pub needle: &'a str,
        pub offsets: &'a [usize],
    }

    impl<'a> SearchResult<'a> {
        pub fn check_length<I>(&self, f: fn(&[char]) -> I)
        where
            I: Index<Item = char>,
        {
            let s = self.input();
            let index = f(&s);
            assert_eq!(s.len(), index.length());
        }

        pub fn check_extract<I>(&self, f: fn(&[char]) -> I)
        where
            I: Index<Item = char>,
        {
            let s = self.input();
            let index = f(&s);
            for i in 0..s.len() {
                for j in i..s.len() {
                    let expected: Vec<char> = s[i..j].iter().cloned().collect();
                    let returned: Vec<char> = index.extract(i, j - i).unwrap().collect();
                    assert_eq!(expected, returned);
                }
            }
        }

        pub fn check_empty_search<I>(&self, f: fn(&[char]) -> I)
        where
            I: Index<Item = char>,
        {
            let s = self.input();
            let index = f(&s);
            let expected: Vec<usize> = (0..s.len()).collect();
            let mut returned: Vec<usize> = index.search(&[]).collect();
            returned.sort_unstable();
            assert_eq!(&expected, &returned);
        }

        pub fn check_search<I>(&self, f: fn(&[char]) -> I)
        where
            I: Index<Item = char>,
        {
            let s = self.input();
            let n: Vec<char> = self.needle();
            let index = f(&s);
            let mut returned: Vec<usize> = index.search(&n).collect();
            returned.sort_unstable();
            assert_eq!(self.offsets, returned.as_slice());
        }

        pub fn check_count<I>(&self, f: fn(&[char]) -> I)
        where
            I: Index<Item = char>,
        {
            let s = self.input();
            let n: Vec<char> = self.needle();
            let index = f(&s);
            assert_eq!(self.offsets.len(), index.count(&n));
        }

        fn input(&self) -> Vec<char> {
            self.input.chars().into_iter().collect()
        }

        fn needle(&self) -> Vec<char> {
            self.needle.chars().into_iter().collect()
        }
    }

    pub const BANANA_AN: SearchResult = SearchResult {
        input: TODO_BANANA,
        needle: "an",
        offsets: &[1, 3],
    };

    pub const BANANA_NA: SearchResult = SearchResult {
        input: TODO_BANANA,
        needle: "na",
        offsets: &[2, 4],
    };

    pub const MISSISSIPPI_ISS: SearchResult = SearchResult {
        input: TODO_MISSISSIPPI,
        needle: "iss",
        offsets: &[1, 4],
    };

    pub const MISSISSIPPIBANANA_ISS: SearchResult = SearchResult {
        input: TODO_MISSISSIPPIBANANA,
        needle: "iss",
        offsets: &[1, 4],
    };

    pub const MIISSISSISSIPPI_ISS: SearchResult = SearchResult {
        input: TODO_MIISSISSISSIPPI,
        needle: "iss",
        offsets: &[2, 5, 8],
    };

    pub const MUTANT_BANANA_AN: SearchResult = SearchResult {
        input: TODO_MUTANT_BANANA,
        needle: "an",
        offsets: &[0, 4, 6, 9, 15, 17],
    };

    pub const SEARCHES: &[SearchResult] = &[
        BANANA_AN,
        BANANA_NA,
        MISSISSIPPI_ISS,
        MISSISSIPPIBANANA_ISS,
        MIISSISSISSIPPI_ISS,
        MUTANT_BANANA_AN,
    ];
}

#[cfg(test)]
mod tests {
    use super::testutil::*;
    use super::*;

    #[test]
    fn inverse() {
        let x = &[8, 6, 9, 5, 0, 3, 1, 2, 7, 4];
        let ix = &[4, 6, 7, 5, 9, 3, 1, 8, 0, 2];
        let returned: &[usize] = &super::inverse(x);
        assert_eq!(ix, returned);
        let returned: &[usize] = &super::inverse(returned);
        assert_eq!(x, returned);
    }

    macro_rules! searches_tests {
        ($($name:ident: $value:expr,)*) => {
        $(
            mod $name {
                #[allow(unused_imports)]
                use super::*;

                #[test]
                fn length() {
                    for search in super::SEARCHES {
                        search.check_length($value);
                    }
                }

                #[test]
                fn extract() {
                    for search in super::SEARCHES {
                        search.check_extract($value);
                    }
                }

                #[test]
                fn search() {
                    for search in super::SEARCHES {
                        search.check_search($value);
                        search.check_empty_search($value);
                    }
                }

                #[test]
                fn count() {
                    for search in super::SEARCHES {
                        search.check_count($value);
                    }
                }
            }
        )*
        }
    }

    fn simple_new(
        text: &[char],
    ) -> SearchIndex<
        char,
        UncompressedSuffixArray,
        UncompressedInverseSuffixArray,
        bit_vector::ReferenceBitVector,
        psi::ReferencePsi,
    > {
        SearchIndex::new(text)
    }

    searches_tests! {
        reference: crate::reference::ReferenceIndex::new,
        simple_new: simple_new,
    }
}
