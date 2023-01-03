use std::fs::File;

use prototk::field_types::*;

use zerror::{ZErrorTrait};

#[test]
fn dev_zero() {
    let dev_zero = File::open("/dev/zero")
        .with_context::<fixed32>("something", 1, 42u32)
        .with_context::<fixed64>("else", 2, 42u64);
    dev_zero.expect("I expect /dev/zero should exist");
}

#[test]
#[should_panic]
fn noexist() {
    let noexist = File::open("noexist")
        .with_context::<fixed32>("something", 1, 42u32)
        .with_context::<fixed64>("else", 2, 42u64);
    noexist.expect("I expect this to fail");
}
