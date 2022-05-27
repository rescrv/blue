extern crate scrunch;

mod common;

// TODO(rescrv): do better
fn simple_new(
    text: &[char],
) -> scrunch::SearchIndex<
    char,
    scrunch::UncompressedSuffixArray,
    scrunch::UncompressedInverseSuffixArray,
    scrunch::bit_vector::ReferenceBitVector,
    scrunch::psi::ReferencePsi,
> {
    scrunch::SearchIndex::new(text)
}

gutenberg_tests! {
    simple: crate::simple_new,
}
