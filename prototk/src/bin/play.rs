#![allow(unused_imports)]
#![allow(non_camel_case_types)]

use buffertk::{stack_pack, v64, Buffer, Packable, StackPacker, Unpackable, Unpacker};
use prototk::{Error, FieldType, FieldHelper, Tag, WireType};

use prototk::field_types::*;
/*

////////////////////////////////////////////// bytesX //////////////////////////////////////////////

impl<'a> FieldType<'a> for bytesX {
    const WIRE_TYPE: WireType = WireType::LengthDelimited;
    const LENGTH_PREFIXED: bool = true;

    type NativeType = &'a [u8];

    fn from_native(_: Self::NativeType) -> Self {
        todo!();
    }
}

impl<'a> FieldHelper<'a, bytesX> for &'a [u8] {
    fn prototk_pack_sz(tag: &Tag, field: &Self) -> usize {
        stack_pack(tag).pack(field).pack_sz()
    }

    fn prototk_pack(tag: &Tag, field: &Self, out: &mut [u8]) {
        stack_pack(tag).pack(field).into_slice(out);
    }

    /*
    fn prototk_assign(proto: &'a [u8], out: &mut Self) {
        *out = proto;
    }
    */
}

impl<'a> FieldHelper<'a, bytesX> for Vec<u8> {
    fn prototk_pack_sz(tag: &Tag, field: &Self) -> usize {
        let field: &[u8] = field;
        stack_pack(tag).pack(field).pack_sz()
    }

    fn prototk_pack(tag: &Tag, field: &Self, out: &mut [u8]) {
        let field: &[u8] = field;
        stack_pack(tag).pack(field).into_slice(out);
    }

    /*
    fn prototk_assign(proto: &'a [u8], out: &mut Self) {
        out.truncate(0);
        out.extend_from_slice(proto);
    }
    */
}

impl<'a> FieldHelper<'a, bytesX> for Buffer {
    fn prototk_pack_sz(tag: &Tag, field: &Self) -> usize {
        let field: &[u8] = field.as_bytes();
        stack_pack(tag).pack(field).pack_sz()
    }

    fn prototk_pack(tag: &Tag, field: &Self, out: &mut [u8]) {
        let field: &[u8] = field.as_bytes();
        stack_pack(tag).pack(field).into_slice(out);
    }

    /*
    fn prototk_assign(proto: &'a [u8], out: &mut Self) {
        if out.len() != proto.len() {
            *out = Buffer::new(proto.len());
        }
        out.as_bytes_mut().copy_from_slice(proto);
    }
    */
}

/*

impl<'a> FieldHelper<'a, stringref<'a>> for String {
    fn prototk_pack_sz(tag: Tag, field: &Self) -> usize {
        let field: &[u8] = field.as_bytes();
        stack_pack(tag).pack(field).pack_sz()
    }

    fn prototk_pack(tag: Tag, field: &Self, out: &mut [u8]) {
        let field: &[u8] = field.as_bytes();
        stack_pack(tag).pack(field).into_slice(out);
    }

    fn prototk_assign(proto: &'a str, out: &mut Self) {
        *out = proto.to_owned();
    }
}
*/

pub struct bytesX {}

impl Packable for bytesX {
    fn pack_sz(&self) -> usize { todo!(); }
    fn pack(&self, _: &mut [u8]) { todo!() }
}

impl<'a> Unpackable<'a> for bytesX {
    type Error = ();

    fn unpack<'b: 'a>(_: &'b [u8]) -> Result<(Self, &'b [u8]), Self::Error> { todo!(); }
}

#[derive(Debug)]
struct FooBar<'a> {
    bytes1: &'a [u8],
    bytes2: Vec<u8>,
    bytes3: Buffer,
    string1: &'a str,
    string2: String,
    vecstr1: Vec<&'a str>,
    vecstr2: Vec<String>,
}

fn foobar<'a>(buf: &'a [u8], string: &'a str) -> FooBar<'a> {
    let mut f = FooBar {
        bytes1: &[],
        bytes2: Vec::new(),
        bytes3: Buffer::new(0),
        string1: "",
        string2: String::default(),
        vecstr1: Vec::new(),
        vecstr2: Vec::new(),
    };
    /*
    FieldHelper::<bytesX>::prototk_assign(buf, &mut f.bytes1);
    FieldHelper::<bytesX>::prototk_assign(buf, &mut f.bytes2);
    FieldHelper::<bytesX>::prototk_assign(buf, &mut f.bytes3);

    let s = stringref(string);
    FieldHelper::<stringref>::prototk_assign(&s.0, &mut f.string1);
    FieldHelper::<stringref>::prototk_assign(&s.0, &mut f.string2);

    FieldHelper::<stringref>::prototk_assign(&s.0, &mut f.vecstr1);
    FieldHelper::<stringref>::prototk_assign(&s.0, &mut f.vecstr1);
    FieldHelper::<stringref>::prototk_assign(&s.0, &mut f.vecstr2);
    FieldHelper::<stringref>::prototk_assign(&s.0, &mut f.vecstr2);
    */

    println!("{:?}", f);
    f
}

fn main() {
    let buf = &[0, 1, 2, 3, 4, 5, 6, 7];
    let f = foobar(buf, "hello world");
    println!("{:?}", f);
}
*/
fn main() {}
