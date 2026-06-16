extern crate prototk;
#[macro_use]
extern crate prototk_derive;

use buffertk::stack_pack;

use prototk::SError;

#[derive(Clone, Debug, Default, Eq, Message, PartialEq)]
struct Foo {
    #[prototk(1, uint64)]
    x: u64,
    #[prototk(2, uint64)]
    y: u64,
}

#[derive(Clone, Debug, Eq, Message, PartialEq)]
#[allow(dead_code)]
struct Bar {
    #[prototk(1, message)]
    res: Result<Foo, SError>,
}

impl Default for Bar {
    fn default() -> Self {
        Self {
            res: Err(prototk::success()),
        }
    }
}

// TODO(rescrv): de-dupe this
fn test_helper(res: Result<Foo, SError>, exp: &[u8]) {
    // test packing
    let buf: Vec<u8> = stack_pack(&res).to_vec();
    let got: &[u8] = &buf;
    assert_eq!(exp, got, "buffer did not match expectations");

    // test unpacking
    let mut up = buffertk::Unpacker::new(exp);
    let got: Result<Foo, SError> = up.unpack().unwrap();
    assert_eq!(res, got, "unpacker failed");

    // test remainder
    let exp: &[u8] = &[];
    let rem: &[u8] = up.remain();
    assert_eq!(exp, rem, "unpack should not have remaining buffer");
}

#[test]
fn result_ok() {
    test_helper(Ok(Foo { x: 42, y: 99 }), &[10, 4, 8, 42, 16, 99]);
}

#[test]
fn result_err() {
    test_helper(
        Err(prototk::unknown_discriminant(33)),
        &[
            18, 103, 102, 40, 101, 114, 114, 111, 114, 32, 40, 112, 104, 97, 115, 101, 32, 112,
            114, 111, 116, 111, 116, 107, 41, 32, 40, 99, 111, 100, 101, 32, 117, 110, 107, 110,
            111, 119, 110, 45, 100, 105, 115, 99, 114, 105, 109, 105, 110, 97, 110, 116, 41, 32,
            40, 109, 101, 115, 115, 97, 103, 101, 32, 34, 117, 110, 107, 110, 111, 119, 110, 32,
            100, 105, 115, 99, 114, 105, 109, 105, 110, 97, 110, 116, 34, 41, 32, 40, 100, 105,
            115, 99, 114, 105, 109, 105, 110, 97, 110, 116, 32, 51, 51, 41, 41,
        ],
    );
}
