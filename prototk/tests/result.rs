
extern crate prototk;
#[macro_use]
extern crate prototk_derive;

use buffertk::stack_pack;

use prototk::Error;

#[derive(Clone, Debug, Default, Eq, Message, PartialEq)]
struct Foo {
    #[prototk(1, uint64)]
    x: u64,
    #[prototk(2, uint64)]
    y: u64,
}

#[derive(Clone, Debug, Eq, Message, PartialEq)]
struct Bar {
    #[prototk(1, message)]
    res: Result<Foo, Error>,
}

impl Default for Bar {
    fn default() -> Self {
        Self {
            res: Err(Error::default()),
        }
    }
}

// TODO(rescrv): de-dupe this
fn test_helper(res: Result<Foo, Error>, exp: &[u8]) {
    // test packing
    let buf: Vec<u8> = stack_pack(&res).to_vec();
    let got: &[u8] = &buf;
    assert_eq!(exp, got, "buffer did not match expectations");

    // test unpacking
    let mut up = buffertk::Unpacker::new(exp);
    let got: Result<Foo, Error> = up.unpack().unwrap();
    assert_eq!(res, got, "unpacker failed");

    // test remainder
    let exp: &[u8] = &[];
    let rem: &[u8] = up.remain();
    assert_eq!(exp, rem, "unpack should not have remaining buffer");
}

#[test]
fn result_ok() {
    test_helper(Ok(Foo {
        x: 42,
        y: 99,
    }), &[10, 4, 8, 42, 16, 99]);
}

#[test]
fn result_err() {
    test_helper(Err(Error::UnknownDiscriminant {
        discriminant: 33,
    }), &[18, 7, 210, 128, 128, 1, 2, 8, 33]);
}
