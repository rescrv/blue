extern crate prototk;
#[macro_use]
extern crate prototk_derive;

use buffertk::stack_pack;

use prototk::Error;

#[derive(Clone, Debug, Default, Eq, Message, PartialEq)]
struct WrappedError {
    #[prototk(1, message)]
    err: Error,
}

fn test_helper(err: Error, s: &str, exp: &[u8]) {
    assert_eq!(s, format!("{:?}", err));
    let we = WrappedError { err };

    // test packing
    let buf: Vec<u8> = stack_pack(&we).to_vec();
    let got: &[u8] = &buf;
    assert_eq!(exp, got, "buffer did not match expectations");

    // test unpacking
    let mut up = buffertk::Unpacker::new(exp);
    let got: WrappedError = up.unpack().unwrap();
    assert_eq!(we, got, "unpacker failed");

    // test remainder
    let exp: &[u8] = &[];
    let rem: &[u8] = up.remain();
    assert_eq!(exp, rem, "unpack should not have remaining buffer");
}

#[test]
fn success() {
    test_helper(Error::Success, "Success", &[10u8, 5, 130, 128, 128, 1, 0]);
}

#[test]
fn buffer_too_short() {
    test_helper(
        Error::BufferTooShort {
            required: 42,
            had: 24,
        },
        "BufferTooShort { required: 42, had: 24 }",
        &[10u8, 9, 138, 128, 128, 1, 4, 8, 42, 16, 24],
    );
}

#[test]
fn invalid_field_number() {
    test_helper(
        Error::InvalidFieldNumber {
            field_number: 13,
            what: "13".to_string(),
        },
        "InvalidFieldNumber { field_number: 13, what: \"13\" }",
        &[10u8, 11, 146, 128, 128, 1, 6, 8, 13, 18, 2, 49, 51],
    );
}

#[test]
fn unhandled_wire_type() {
    test_helper(
        Error::UnhandledWireType { wire_type: 42 },
        "UnhandledWireType { wire_type: 42 }",
        &[10u8, 7, 154, 128, 128, 1, 2, 8, 42],
    );
}

#[test]
fn tag_too_large() {
    test_helper(
        Error::TagTooLarge { tag: 8589934592u64 },
        "TagTooLarge { tag: 8589934592 }",
        &[10u8, 11, 162, 128, 128, 1, 6, 8, 128, 128, 128, 128, 32],
    );
}

#[test]
fn varint_overflow() {
    test_helper(
        Error::VarintOverflow { bytes: 11 },
        "VarintOverflow { bytes: 11 }",
        &[10u8, 7, 170, 128, 128, 1, 2, 8, 11],
    )
}

#[test]
fn unsigned_overflow() {
    test_helper(
        Error::UnsignedOverflow { value: 1u64 << 32 },
        "UnsignedOverflow { value: 4294967296 }",
        &[10u8, 11, 178, 128, 128, 1, 6, 8, 128, 128, 128, 128, 16],
    )
}

#[test]
fn signed_overflow() {
    test_helper(
        Error::SignedOverflow { value: 1i64 << 32 },
        "SignedOverflow { value: 4294967296 }",
        &[10u8, 11, 186, 128, 128, 1, 6, 8, 128, 128, 128, 128, 16],
    )
}

#[test]
fn wrong_length() {
    test_helper(
        Error::WrongLength {
            required: 32,
            had: 16,
        },
        "WrongLength { required: 32, had: 16 }",
        &[10u8, 9, 194, 128, 128, 1, 4, 8, 32, 16, 16],
    )
}

#[test]
fn string_encoding() {
    test_helper(
        Error::StringEncoding {},
        "StringEncoding",
        &[10u8, 5, 202, 128, 128, 1, 0],
    )
}

#[test]
fn unknown_discriminant() {
    test_helper(
        Error::UnknownDiscriminant { discriminant: 42 },
        "UnknownDiscriminant { discriminant: 42 }",
        &[10u8, 7, 210, 128, 128, 1, 2, 8, 42],
    )
}
