extern crate prototk;
#[macro_use]
extern crate prototk_derive;

////////////////////////////////////// Stuff we want to write //////////////////////////////////////

/// Details of an X,Y point that might be relevant for an error.
#[derive(Debug, Default, Eq, Message, PartialEq)]
pub struct Details {
    #[prototk(1, uint64)]
    x: u64,
    #[prototk(2, uint64)]
    y: u64,
}

/// An [Error] demonstrating three different ways of auto-generating enums.
#[derive(Debug, Eq, Message, PartialEq)]
pub enum Error {
    #[prototk(1, message)]
    Success,
    #[prototk(2, message)]
    BlockTooSmall {
        #[prototk(1, uint64)]
        length: usize,
        #[prototk(2, uint64)]
        required: usize,
    },
    #[prototk(3, message)]
    DetailedError(Details),
}

impl Default for Error {
    fn default() -> Error {
        Error::Success
    }
}

/////////////////////////////////// What we want to see generated //////////////////////////////////

#[test]
fn three_kinds_of_enum() {
    let exp1 = Error::Success;
    let exp2 = Error::BlockTooSmall { length: 5, required: 10 };
    let exp3 = Error::DetailedError(Details { x: 42, y: 99 });
    // test packing
    let buf: Vec<u8> = buffertk::stack_pack((&exp1, &exp2, &exp3)).to_vec();
    let exp: &[u8] = &[
        10, 0,
        18, 4, 8, 5, 16, 10,
        26, 4, 8, 42, 16, 99,
    ];
    let got: &[u8] = &buf;
    assert_eq!(exp, got, "buffer did not match expectations");

    // test unpacking
    let mut up = buffertk::Unpacker::new(&[10, 0]);
    let got1 = up.unpack().unwrap();
    assert_eq!(exp1, got1, "unpacker failed");

    let mut up = buffertk::Unpacker::new(&[18, 4, 8, 5, 16, 10]);
    let got2 = up.unpack().unwrap();
    assert_eq!(exp2, got2, "unpacker failed");

    let mut up = buffertk::Unpacker::new(&[26, 4, 8, 42, 16, 99]);
    let got3 = up.unpack().unwrap();
    assert_eq!(exp3, got3, "unpacker failed");

    // test remainder
    let exp: &[u8] = &[];
    let rem: &[u8] = up.remain();
    assert_eq!(exp, rem, "unpack should not have remaining buffer");
}
