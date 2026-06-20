extern crate prototk;
#[macro_use]
extern crate prototk_derive;

use buffertk::stack_pack;

use prototk::SError;

#[derive(Clone, Debug, Default, Eq, Message, PartialEq)]
struct WrappedError {
    #[prototk(1, message)]
    err: SError,
}

fn test_helper(err: SError, code: &str) {
    assert_eq!(Some(code), prototk::error_code(&err));
    let we = WrappedError { err };

    let buf: Vec<u8> = stack_pack(&we).to_vec();
    let mut up = buffertk::Unpacker::new(&buf);
    let got: WrappedError = up.unpack().unwrap();
    assert_eq!(we, got, "unpacker failed");
    assert!(up.is_empty(), "unpack should not have remaining buffer");
}

#[test]
fn success() {
    test_helper(prototk::success(), prototk::CODE_SUCCESS);
}

#[test]
fn buffer_too_short() {
    test_helper(
        prototk::buffer_too_short(42, 24),
        prototk::CODE_BUFFER_TOO_SHORT,
    );
}

#[test]
fn invalid_field_number() {
    test_helper(
        prototk::invalid_field_number(13, "13"),
        prototk::CODE_INVALID_FIELD_NUMBER,
    );
}

#[test]
fn unhandled_wire_type() {
    test_helper(
        prototk::unhandled_wire_type(42),
        prototk::CODE_UNHANDLED_WIRE_TYPE,
    );
}

#[test]
fn tag_too_large() {
    test_helper(
        prototk::tag_too_large(8589934592u64),
        prototk::CODE_TAG_TOO_LARGE,
    );
}

#[test]
fn varint_overflow() {
    test_helper(prototk::varint_overflow(11), prototk::CODE_VARINT_OVERFLOW);
}

#[test]
fn unsigned_overflow() {
    test_helper(
        prototk::unsigned_overflow(1u64 << 32),
        prototk::CODE_UNSIGNED_OVERFLOW,
    );
}

#[test]
fn signed_overflow() {
    test_helper(
        prototk::signed_overflow(1i64 << 32),
        prototk::CODE_SIGNED_OVERFLOW,
    );
}

#[test]
fn wrong_length() {
    test_helper(prototk::wrong_length(32, 16), prototk::CODE_WRONG_LENGTH);
}

#[test]
fn string_encoding() {
    test_helper(prototk::string_encoding(), prototk::CODE_STRING_ENCODING);
}

#[test]
fn unknown_discriminant() {
    test_helper(
        prototk::unknown_discriminant(42),
        prototk::CODE_UNKNOWN_DISCRIMINANT,
    );
}
