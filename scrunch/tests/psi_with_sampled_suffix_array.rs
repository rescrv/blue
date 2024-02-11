extern crate scrunch;

mod common;

gutenberg_tests! {
    psi_with_sampled_suffix_array,
    ::scrunch::PsiDocument::<'b,
        ::scrunch::sa::SampledSuffixArray<'b>,
        ::scrunch::isa::ReferenceInverseSuffixArray,
        ::scrunch::psi::ReferencePsi
    >
}
