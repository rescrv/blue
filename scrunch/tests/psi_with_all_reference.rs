extern crate scrunch;

mod common;

gutenberg_tests! {
    psi_with_all_reference,
    ::scrunch::PsiDocument::<'b,
        ::scrunch::sa::ReferenceSuffixArray,
        ::scrunch::isa::ReferenceInverseSuffixArray,
        ::scrunch::psi::ReferencePsi
    >
}
