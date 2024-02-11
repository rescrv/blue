extern crate scrunch;

mod common;

gutenberg_tests! {
    psi_with_wavelet_psi,
    ::scrunch::PsiDocument::<'b,
        ::scrunch::sa::ReferenceSuffixArray,
        ::scrunch::isa::ReferenceInverseSuffixArray,
        ::scrunch::psi::wavelet_tree::WaveletTreePsi<'b, ::scrunch::wavelet_tree::ReferenceWaveletTree>,
    >
}
