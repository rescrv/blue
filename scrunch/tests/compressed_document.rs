extern crate scrunch;

mod common;

gutenberg_tests! {
    compressed_document,
    ::scrunch::CompressedDocument<'b>
}
