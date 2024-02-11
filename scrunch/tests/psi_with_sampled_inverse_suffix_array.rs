extern crate scrunch;

mod common;

gutenberg_tests! {
    psi_with_sampled_inverse_suffix_array,
    ::scrunch::PsiDocument::<'b,
        ::scrunch::sa::ReferenceSuffixArray,
        ::scrunch::isa::SampledInverseSuffixArray<'b>,
        ::scrunch::psi::ReferencePsi
    >
}
