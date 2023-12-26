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
#[derive(Debug, Default, Eq, Message, PartialEq)]
pub enum Error {
    #[prototk(1, message)]
    #[default]
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

/////////////////////////////////// What we want to see generated //////////////////////////////////

#[test]
fn three_kinds_of_enum() {
    let exp1 = Error::Success;
    let exp2 = Error::BlockTooSmall {
        length: 5,
        required: 10,
    };
    let exp3 = Error::DetailedError(Details { x: 42, y: 99 });
    // test packing
    let buf: Vec<u8> = buffertk::stack_pack((&exp1, &exp2, &exp3)).to_vec();
    let exp: &[u8] = &[10, 0, 18, 4, 8, 5, 16, 10, 26, 4, 8, 42, 16, 99];
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

////////////////////////////////////////// EnumWithOption //////////////////////////////////////////

#[derive(Debug, Default, Eq, Message, PartialEq)]
enum EnumWithOptionAndVectorMessages {
    #[prototk(1, message)]
    #[default]
    Nop,
    #[prototk(2, message)]
    VariantWithOption {
        #[prototk(1, message)]
        value: Option<Details>,
    },
    #[prototk(3, message)]
    VariantWithVector {
        #[prototk(1, message)]
        value: Vec<Details>,
    },
}

#[test]
fn enum_embed_option() {
    let value = EnumWithOptionAndVectorMessages::VariantWithOption {
        value: Some(Details { x: 42, y: 99 }),
    };
    // test packing
    let buf: Vec<u8> = buffertk::stack_pack(&value).to_vec();
    let exp: &[u8] = &[18, 6, 10, 4, 8, 42, 16, 99];
    let got: &[u8] = &buf;
    assert_eq!(exp, got, "buffer did not match expectations");

    // test unpacking
    let mut up = buffertk::Unpacker::new(exp);
    let got: EnumWithOptionAndVectorMessages = up.unpack().unwrap();
    assert_eq!(value, got, "unpacker failed");

    // test remainder
    let exp: &[u8] = &[];
    let rem: &[u8] = up.remain();
    assert_eq!(exp, rem, "unpack should not have remaining buffer");
}

#[test]
fn enum_embed_vector() {
    let value = EnumWithOptionAndVectorMessages::VariantWithVector {
        value: vec![Details { x: 42, y: 99 }, Details { x: 1, y: 1 }],
    };
    // test packing
    let buf: Vec<u8> = buffertk::stack_pack(&value).to_vec();
    let exp: &[u8] = &[26, 12, 10, 4, 8, 42, 16, 99, 10, 4, 8, 1, 16, 1];
    let got: &[u8] = &buf;
    assert_eq!(exp, got, "buffer did not match expectations");

    // test unpacking
    let mut up = buffertk::Unpacker::new(exp);
    let got: EnumWithOptionAndVectorMessages = up.unpack().unwrap();
    assert_eq!(value, got, "unpacker failed");

    // test remainder
    let exp: &[u8] = &[];
    let rem: &[u8] = up.remain();
    assert_eq!(exp, rem, "unpack should not have remaining buffer");
}

/////////////////////////////////////////// EnumWithArray //////////////////////////////////////////

#[derive(Debug, Default, Eq, Message, PartialEq)]
enum EnumWithArray {
    #[prototk(1, message)]
    #[default]
    Nop,
    #[prototk(2, message)]
    Variant32 {
        #[prototk(1, bytes32)]
        value: [u8; 32],
    },
    #[prototk(3, message)]
    Variant64 {
        #[prototk(1, bytes64)]
        value: [u8; 64],
    },
}

#[test]
fn enum_with_array() {
    let value = EnumWithArray::Variant64 {
        value: [
            0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45,
            46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63,
        ],
    };
    // test packing
    let buf: Vec<u8> = buffertk::stack_pack(&value).to_vec();
    let exp: &[u8] = &[
        26, 66, 10, 64, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
        21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43,
        44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63,
    ];
    let got: &[u8] = &buf;
    assert_eq!(exp, got, "buffer did not match expectations");

    // test unpacking
    let mut up = buffertk::Unpacker::new(exp);
    let got: EnumWithArray = up.unpack().unwrap();
    assert_eq!(value, got, "unpacker failed");

    // test remainder
    let exp: &[u8] = &[];
    let rem: &[u8] = up.remain();
    assert_eq!(exp, rem, "unpack should not have remaining buffer");
}
