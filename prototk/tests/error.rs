extern crate prototk;
#[macro_use]
extern crate prototk_derive;

use prototk::Error;

#[derive(Clone, Debug, Default, Eq, Message, PartialEq)]
struct WrappedError {
    #[prototk(1, message)]
    err: Error,
}

#[test]
fn serialize_error() {
    let we = WrappedError {
        err: Error::BufferTooShort {
            required: 10,
            had: 5,
        },
    };
    // test packing
    let buf: Vec<u8> = buffertk::stack_pack(&we).to_vec();
    let exp: &[u8] = &[10, 9, 138, 128, 128, 8, 4, 8, 10, 16, 5];
    let got: &[u8] = &buf;
    assert_eq!(exp, got, "buffer did not match expectations");

    /*
    // test unpacking
    let mut up = buffertk::Unpacker::new(exp);
    let got: WrappedError = up.unpack().unwrap();
    println!("FINDME GOT {:?}", got);
    assert_eq!(we, got, "unpacker failed");

    // test remainder
    let exp: &[u8] = &[];
    let rem: &[u8] = up.remain();
    assert_eq!(exp, rem, "unpack should not have remaining buffer");
    */
}
