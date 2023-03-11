extern crate prototk;
#[macro_use]
extern crate prototk_derive;

use buffertk::Buffer;

//////////////////////////////////////////// EmptyStruct ///////////////////////////////////////////

#[derive(Clone, Debug, Default, Message, PartialEq)]
struct EmptyStruct {}

#[test]
fn empty_struct() {
    let s = EmptyStruct {};
    // test packing
    let buf = buffertk::stack_pack(s).to_vec();
    let exp: &[u8] = &[];
    let got: &[u8] = &buf;
    assert_eq!(exp, got, "buffer did not match expectations");
    // test unpacking
    let mut up = buffertk::Unpacker::new(exp);
    let exp = EmptyStruct {};
    let got = up.unpack();
    assert_eq!(
        Ok(exp),
        got,
        "unpacker should have returned Ok(EmptyStruct{{}})"
    );
    // test remainder
    let exp: &[u8] = &[];
    let rem: &[u8] = up.remain();
    assert_eq!(exp, rem, "unpack should not have remaining buffer");
}

//////////////////////////////////////////// NamedStruct ///////////////////////////////////////////

#[derive(Clone, Debug, Default, Message, PartialEq)]
struct NamedStruct {
    #[prototk(1, uint64)]
    x: u64,
    #[prototk(2, double)]
    y: f64,
    #[prototk(3, sint32)]
    z: i32,
}

#[test]
fn named_struct() {
    let s = NamedStruct {
        x: 42,
        y: 3.14159,
        z: -1,
    };
    // test packing
    let buf = buffertk::stack_pack(&s).to_vec();
    let exp: &[u8] = &[8, 42, 17, 110, 134, 27, 240, 249, 33, 9, 64, 24, 1];
    let got: &[u8] = &buf;
    assert_eq!(exp, got, "buffer did not match expectations");
    // test unpacking
    let mut up = buffertk::Unpacker::new(exp);
    let exp = s.clone();
    let got = up.unpack();
    assert_eq!(Ok(exp), got, "unpacker should have returned Ok({:?})", s);
    // test remainder
    let exp: &[u8] = &[];
    let rem: &[u8] = up.remain();
    assert_eq!(exp, rem, "unpack should not have remaining buffer");
}

/////////////////////////////////////////// UnnamedStruct //////////////////////////////////////////

#[derive(Clone, Debug, Default, Message, PartialEq)]
struct UnnamedStruct(
    #[prototk(1, uint64)] u64,
    #[prototk(2, double)] f64,
    #[prototk(3, sint32)] i32,
);

#[test]
fn unnamed_struct() {
    let u = UnnamedStruct(42, 3.14159, -1);
    // test packing
    let buf = buffertk::stack_pack(&u).to_vec();
    let exp: &[u8] = &[8, 42, 17, 110, 134, 27, 240, 249, 33, 9, 64, 24, 1];
    let got: &[u8] = &buf;
    assert_eq!(exp, got, "buffer did not match expectations");
    // test unpacking
    let mut up = buffertk::Unpacker::new(exp);
    let exp = u.clone();
    let got = up.unpack();
    assert_eq!(Ok(exp), got, "unpacker should have returned Ok({:?})", u);
    // test remainder
    let exp: &[u8] = &[];
    let rem: &[u8] = up.remain();
    assert_eq!(exp, rem, "unpack should not have remaining buffer");
}

//////////////////////////////////////////// UnitStruct ////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message, PartialEq)]
struct UnitStruct;

#[test]
fn unit_struct() {
    let u = UnitStruct {};
    // test packing
    let buf = buffertk::stack_pack(&u).to_vec();
    let exp: &[u8] = &[];
    let got: &[u8] = &buf;
    assert_eq!(exp, got, "buffer did not match expectations");
    // test unpacking
    let mut up = buffertk::Unpacker::new(exp);
    let exp = u.clone();
    let got = up.unpack();
    assert_eq!(Ok(exp), got, "unpacker should have returned Ok({:?})", u);
    // test remainder
    let exp: &[u8] = &[];
    let rem: &[u8] = up.remain();
    assert_eq!(exp, rem, "unpack should not have remaining buffer");
}

/////////////////////////////////////////// NestedStruct ///////////////////////////////////////////

#[derive(Clone, Debug, Default, Message, PartialEq)]
struct NestedStruct {
    #[prototk(1, message)]
    m: NamedStruct,
}

#[test]
fn nested_struct() {
    let n = NestedStruct {
        m: NamedStruct {
            x: 42,
            y: 3.14159,
            z: -1,
        },
    };
    // test packing
    let buf = buffertk::stack_pack(&n).to_vec();
    let exp: &[u8] = &[10, 13, 8, 42, 17, 110, 134, 27, 240, 249, 33, 9, 64, 24, 1];
    let got: &[u8] = &buf;
    assert_eq!(exp, got, "buffer did not match expectations");
    // test unpacking
    let mut up = buffertk::Unpacker::new(exp);
    let exp = n.clone();
    let got = up.unpack();
    assert_eq!(Ok(exp), got, "unpacker should have returned Ok({:?})", n);
    // test remainder
    let exp: &[u8] = &[];
    let rem: &[u8] = up.remain();
    assert_eq!(exp, rem, "unpack should not have remaining buffer");
}

/////////////////////////////////////////////// Enums //////////////////////////////////////////////

#[derive(Clone, Debug, Message, PartialEq)]
enum EnumOneOf {
    #[prototk(1, sint64)]
    VariantOne(i64),
    #[prototk(2, uint64)]
    VariantTwo(u64),
    #[prototk(3, message)]
    VariantThree(NamedStruct),
}

impl Default for EnumOneOf {
    fn default() -> Self {
        EnumOneOf::VariantOne(0)
    }
}

#[test]
fn enum_one_of() {
    let exp1 = EnumOneOf::VariantOne(-1i64);
    let exp2 = EnumOneOf::VariantTwo(42u64);
    let exp3 = EnumOneOf::VariantThree(NamedStruct {
        x: 42,
        y: 3.14159,
        z: -1,
    });
    // test packing
    let buf: Vec<u8> = buffertk::stack_pack((&exp1, &exp2, &exp3)).to_vec();
    let exp: &[u8] = &[
        8, 1, 16, 42, 26, 13, 8, 42, 17, 110, 134, 27, 240, 249, 33, 9, 64, 24, 1,
    ];
    let got: &[u8] = &buf;
    assert_eq!(exp, got, "buffer did not match expectations");
    // test unpacking
    let mut up = buffertk::Unpacker::new(exp);
    let got1 = up.unpack().unwrap();
    assert_eq!(exp1, got1, "unpacker failed");
    let got2 = up.unpack().unwrap();
    assert_eq!(exp2, got2, "unpacker failed");
    let got3 = up.unpack().unwrap();
    assert_eq!(exp3, got3, "unpacker failed");
    // test remainder
    let exp: &[u8] = &[];
    let rem: &[u8] = up.remain();
    assert_eq!(exp, rem, "unpack should not have remaining buffer");
}

/////////////////////////////////////////// Nested Bytes ///////////////////////////////////////////

#[derive(Clone, Debug, Message, PartialEq)]
struct WithBytes<'a> {
    #[prototk(1, bytes)]
    payload: &'a [u8],
}

impl<'a> Default for WithBytes<'a> {
    fn default() -> Self {
        WithBytes { payload: &[] }
    }
}

#[test]
fn nested_bytes() {
    let wb = WithBytes {
        payload: &[42, 43, 44],
    };
    // test packing
    let buf: Vec<u8> = buffertk::stack_pack(&wb).to_vec();
    let exp: &[u8] = &[10, 3, 42, 43, 44];
    let got: &[u8] = &buf;
    assert_eq!(exp, got, "buffer did not match expectations");
    // test unpacking
    let mut up = buffertk::Unpacker::new(exp);
    let got = up.unpack().unwrap();
    assert_eq!(wb, got, "unpacker failed");
    // test remainder
    let exp: &[u8] = &[];
    let rem: &[u8] = up.remain();
    assert_eq!(exp, rem, "unpack should not have remaining buffer");
}

////////////////////////////////////////////// Vectors /////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message, PartialEq)]
struct WithVectors {
    #[prototk(1, sint64)]
    payload: Vec<i64>,
}

#[test]
fn vector_integers() {
    let wb = WithVectors {
        payload: vec![42, 43, 44],
    };
    // test packing
    let buf: Vec<u8> = buffertk::stack_pack(&wb).to_vec();
    let exp: &[u8] = &[8, 84, 8, 86, 8, 88];
    let got: &[u8] = &buf;
    assert_eq!(exp, got, "buffer did not match expectations");
    // test unpacking
    let mut up = buffertk::Unpacker::new(exp);
    let got = up.unpack().unwrap();
    assert_eq!(wb, got, "unpacker failed");
    // test remainder
    let exp: &[u8] = &[];
    let rem: &[u8] = up.remain();
    assert_eq!(exp, rem, "unpack should not have remaining buffer");
}

/////////////////////////////////////////// VectorOfBytes //////////////////////////////////////////

#[derive(Clone, Debug, Default, Message, PartialEq)]
struct VectorOfBytes {
    #[prototk(15, bytes)]
    value: Vec<u8>,
}

#[test]
fn vector_of_bytes() {
    let vb = VectorOfBytes {
        value: vec![0, 1, 2, 3, 4, 5, 6, 7],
    };
    // test packing
    let buf: Vec<u8> = buffertk::stack_pack(&vb).to_vec();
    let exp: &[u8] = &[122, 8, 0, 1, 2, 3, 4, 5, 6, 7];
    let got: &[u8] = &buf;
    assert_eq!(exp, got, "buffer did not match expectations");
    // test unpacking
    let mut up = buffertk::Unpacker::new(exp);
    let got = up.unpack().unwrap();
    assert_eq!(vb, got, "unpacker failed");
    // test remainder
    let exp: &[u8] = &[];
    let rem: &[u8] = up.remain();
    assert_eq!(exp, rem, "unpack should not have remaining buffer");
}

///////////////////////////////////////// VectorOfMesssages ////////////////////////////////////////

#[derive(Clone, Debug, Default, Message, PartialEq)]
struct VectorOfMessages {
    #[prototk(15, message)]
    messages: Vec<NamedStruct>,
}

#[test]
fn vector_messages() {
    let vm = VectorOfMessages {
        messages: vec![
            NamedStruct {
                x: 42,
                y: 3.14159,
                z: -1,
            },
            NamedStruct {
                x: 42,
                y: 3.14159,
                z: -1,
            },
        ],
    };
    // test packing
    let buf: Vec<u8> = buffertk::stack_pack(&vm).to_vec();
    let exp: &[u8] = &[
        122, 13, 8, 42, 17, 110, 134, 27, 240, 249, 33, 9, 64, 24, 1, 122, 13, 8, 42, 17, 110, 134,
        27, 240, 249, 33, 9, 64, 24, 1,
    ];
    let got: &[u8] = &buf;
    assert_eq!(exp, got, "buffer did not match expectations");
    // test unpacking
    let mut up = buffertk::Unpacker::new(exp);
    let got = up.unpack().unwrap();
    assert_eq!(vm, got, "unpacker failed");
    // test remainder
    let exp: &[u8] = &[];
    let rem: &[u8] = up.remain();
    assert_eq!(exp, rem, "unpack should not have remaining buffer");
}

///////////////////////////////////////// OptionOfMesssages ////////////////////////////////////////

#[derive(Clone, Debug, Default, Message, PartialEq)]
struct OptionOfMessages {
    #[prototk(15, message)]
    messages: Option<NamedStruct>,
}

#[test]
fn option_messages() {
    let vm = OptionOfMessages {
        messages: Some(NamedStruct {
            x: 42,
            y: 3.14159,
            z: -1,
        }),
    };
    // test packing
    let buf: Vec<u8> = buffertk::stack_pack(&vm).to_vec();
    let exp: &[u8] = &[122, 13, 8, 42, 17, 110, 134, 27, 240, 249, 33, 9, 64, 24, 1];
    let got: &[u8] = &buf;
    assert_eq!(exp, got, "buffer did not match expectations");
    // test unpacking
    let mut up = buffertk::Unpacker::new(exp);
    let got = up.unpack().unwrap();
    assert_eq!(vm, got, "unpacker failed");
    // test remainder
    let exp: &[u8] = &[];
    let rem: &[u8] = up.remain();
    assert_eq!(exp, rem, "unpack should not have remaining buffer");
}
////////////////////////////////////////////// String //////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message, PartialEq)]
struct StringInStruct {
    #[prototk(11, string)]
    string: String,
}

#[test]
fn string_in_struct() {
    let sis = StringInStruct {
        string: "hello world".to_string(),
    };
    // test packing
    let buf: Vec<u8> = buffertk::stack_pack(&sis).to_vec();
    let exp: &[u8] = &[90, 11, 104, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100];
    let got: &[u8] = &buf;
    assert_eq!(exp, got, "buffer did not match expectations");
    // test unpacking
    let mut up = buffertk::Unpacker::new(exp);
    let got = up.unpack().unwrap();
    assert_eq!(sis, got, "unpacker failed");
    // test remainder
    let exp: &[u8] = &[];
    let rem: &[u8] = up.remain();
    assert_eq!(exp, rem, "unpack should not have remaining buffer");
}

///////////////////////////////////////////// 32 bytes /////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message, PartialEq)]
struct Bytes32 {
    #[prototk(11, bytes32)]
    buffer: [u8; 32],
}

#[test]
fn bytes32() {
    let b32 = Bytes32 {
        buffer: [
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31,
        ],
    };
    // test packing
    let buf: Vec<u8> = buffertk::stack_pack(&b32).to_vec();
    let exp: &[u8] = &[
        90, 32, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
        23, 24, 25, 26, 27, 28, 29, 30, 31,
    ];
    let got: &[u8] = &buf;
    assert_eq!(exp, got, "buffer did not match expectations");
    // test unpacking
    let mut up = buffertk::Unpacker::new(exp);
    let got = up.unpack().unwrap();
    assert_eq!(b32, got, "unpacker failed");
    // test remainder
    let exp: &[u8] = &[];
    let rem: &[u8] = up.remain();
    assert_eq!(exp, rem, "unpack should not have remaining buffer");
}

////////////////////////////////////////////// Buffer //////////////////////////////////////////////

#[derive(Clone, Debug, Message, PartialEq)]
struct BufferMessage {
    #[prototk(11, bytes)]
    buffer: Buffer,
}

impl Default for BufferMessage {
    fn default() -> Self {
        Self {
            buffer: Buffer::new(0),
        }
    }
}

#[test]
fn buffer_message() {
    let bytes: &[u8] = &[
        0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
        25, 26, 27, 28, 29, 30, 31,
    ];
    let bufmsg = BufferMessage {
        buffer: bytes.into(),
    };
    // test packing
    let buf: Vec<u8> = buffertk::stack_pack(&bufmsg).to_vec();
    let exp: &[u8] = &[
        90, 32, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
        23, 24, 25, 26, 27, 28, 29, 30, 31,
    ];
    let got: &[u8] = &buf;
    assert_eq!(exp, got, "buffer did not match expectations");
    // test unpacking
    let mut up = buffertk::Unpacker::new(exp);
    let got: BufferMessage = up.unpack().unwrap();
    assert_eq!(bufmsg, got, "unpacker failed");
    // test remainder
    let exp: &[u8] = &[];
    let rem: &[u8] = up.remain();
    assert_eq!(exp, rem, "unpack should not have remaining buffer");
}

////////////////////////////////////////////// Option //////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message, PartialEq)]
struct OptionStruct {
    #[prototk(1, uint64)]
    x: Option<u64>,
    #[prototk(2, double)]
    y: Option<f64>,
    #[prototk(3, sint32)]
    z: Option<i32>,
}

#[test]
fn option_struct() {
    let s = OptionStruct {
        x: Some(42),
        y: Some(3.14159),
        z: None,
    };
    // test packing
    let buf = buffertk::stack_pack(&s).to_vec();
    let exp: &[u8] = &[8, 42, 17, 110, 134, 27, 240, 249, 33, 9, 64];
    let got: &[u8] = &buf;
    assert_eq!(exp, got, "buffer did not match expectations");
    // test unpacking
    let mut up = buffertk::Unpacker::new(exp);
    let exp = s.clone();
    let got = up.unpack();
    assert_eq!(Ok(exp), got, "unpacker should have returned Ok({:?})", s);
    // test remainder
    let exp: &[u8] = &[];
    let rem: &[u8] = up.remain();
    assert_eq!(exp, rem, "unpack should not have remaining buffer");
}
