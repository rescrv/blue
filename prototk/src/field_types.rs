#![allow(non_camel_case_types)]

// We allow non-CamelCase types here because we want the struct names to appear as close to they do
// in the proto documentation and official implementation.  Thus, `uint64` is how we represent the
// type of `u64`.  The primary use of these field types is to pull the token from a field annotated
// with e.g. #[prototk(7, uint64)], where the uint64 token is used verbatim.

use std::convert::TryInto;

use super::*;

/////////////////////////////////////////////// int32 //////////////////////////////////////////////

pub struct int32 {
    x: i32,
}

impl<'a> FieldType<'a> for int32 {
    const WIRE_TYPE: WireType = WireType::Varint;
    const LENGTH_PREFIXED: bool = false;

    type NativeType = i32;

    fn into_native(self) -> Self::NativeType {
        self.x
    }
}

impl Packable for int32 {
    fn pack_sz(&self) -> usize {
        let v: v64 = self.x.into();
        v.pack_sz()
    }

    fn pack<'a>(&self, buf: &'a mut[u8]) {
        let v: v64 = self.x.into();
        v.pack(buf)
    }
}

impl<'a> Unpackable<'a> for int32 {
    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let (v, buf) = v64::unpack(buf)?;
        let x: i32 = v.try_into()?;
        Ok((x.into(), buf))
    }
}

impl From<i32> for int32 {
    fn from(x: i32) -> Self {
        Self {
            x,
        }
    }
}

impl From<&i32> for int32 {
    fn from(x: &i32) -> Self {
        Self {
            x: *x,
        }
    }
}

/////////////////////////////////////////////// int64 //////////////////////////////////////////////

pub struct int64 {
    x: i64,
}

impl<'a> FieldType<'a> for int64 {
    const WIRE_TYPE: WireType = WireType::Varint;
    const LENGTH_PREFIXED: bool = false;

    type NativeType = i64;

    fn into_native(self) -> Self::NativeType {
        self.x
    }
}

impl Packable for int64 {
    fn pack_sz(&self) -> usize {
        let v: v64 = self.x.into();
        v.pack_sz()
    }

    fn pack<'a>(&self, buf: &'a mut[u8]) {
        let v: v64 = self.x.into();
        v.pack(buf)
    }
}

impl<'a> Unpackable<'a> for int64 {
    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let (v, buf) = v64::unpack(buf)?;
        let x: i64 = v.into();
        Ok((x.into(), buf))
    }
}

impl From<i64> for int64 {
    fn from(x: i64) -> Self {
        Self {
            x,
        }
    }
}

impl From<&i64> for int64 {
    fn from(x: &i64) -> Self {
        Self {
            x: *x,
        }
    }
}

////////////////////////////////////////////// uint32 //////////////////////////////////////////////

pub struct uint32 {
    x: u32,
}

impl<'a> FieldType<'a> for uint32 {
    const WIRE_TYPE: WireType = WireType::Varint;
    const LENGTH_PREFIXED: bool = false;

    type NativeType = u32;

    fn into_native(self) -> Self::NativeType {
        self.x
    }
}

impl Packable for uint32 {
    fn pack_sz(&self) -> usize {
        let v: v64 = self.x.into();
        v.pack_sz()
    }

    fn pack<'a>(&self, buf: &'a mut[u8]) {
        let v: v64 = self.x.into();
        v.pack(buf)
    }
}

impl<'a> Unpackable<'a> for uint32 {
    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let (v, buf) = v64::unpack(buf)?;
        let x: u32 = v.try_into()?;
        Ok((x.into(), buf))
    }
}

impl From<u32> for uint32 {
    fn from(x: u32) -> Self {
        Self {
            x,
        }
    }
}

impl From<&u32> for uint32 {
    fn from(x: &u32) -> Self {
        Self {
            x: *x,
        }
    }
}

////////////////////////////////////////////// uint64 //////////////////////////////////////////////

pub struct uint64 {
    x: u64,
}

impl<'a> FieldType<'a> for uint64 {
    const WIRE_TYPE: WireType = WireType::Varint;
    const LENGTH_PREFIXED: bool = false;

    type NativeType = u64;

    fn into_native(self) -> Self::NativeType {
        self.x
    }
}

impl Packable for uint64 {
    fn pack_sz(&self) -> usize {
        let v: v64 = self.x.into();
        v.pack_sz()
    }

    fn pack<'a>(&self, buf: &'a mut[u8]) {
        let v: v64 = self.x.into();
        v.pack(buf)
    }
}

impl<'a> Unpackable<'a> for uint64 {
    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let (v, buf) = v64::unpack(buf)?;
        let x: u64 = v.into();
        Ok((x.into(), buf))
    }
}

impl From<u64> for uint64 {
    fn from(x: u64) -> Self {
        Self {
            x,
        }
    }
}

impl From<&u64> for uint64 {
    fn from(x: &u64) -> Self {
        Self {
            x: *x,
        }
    }
}

////////////////////////////////////////////// sint32 //////////////////////////////////////////////

pub struct sint32 {
    x: i32,
}

impl<'a> FieldType<'a> for sint32 {
    const WIRE_TYPE: WireType = WireType::Varint;
    const LENGTH_PREFIXED: bool = false;

    type NativeType = i32;

    fn into_native(self) -> Self::NativeType {
        self.x
    }
}

impl Packable for sint32 {
    fn pack_sz(&self) -> usize {
        let v: v64 = self.into();
        v.pack_sz()
    }

    fn pack<'a>(&self, buf: &'a mut[u8]) {
        let v: v64 = self.into();
        v.pack(buf)
    }
}

impl<'a> Unpackable<'a> for sint32 {
    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let (v, buf) = v64::unpack(buf)?;
        let x: i64 = unzigzag(v.into());
        let x: i32 = match x.try_into() {
            Ok(x) => { x },
            Err(_) => {
                return Err(Error::SignedOverflow{value:x});
            },
        };
        Ok((x.into(), buf))
    }
}

impl From<i32> for sint32 {
    fn from(x: i32) -> Self {
        Self {
            x,
        }
    }
}

impl From<&i32> for sint32 {
    fn from(x: &i32) -> Self {
        Self {
            x: *x,
        }
    }
}

impl From<&sint32> for v64 {
    fn from(x: &sint32) -> v64 {
        zigzag(x.x as i64).into()
    }
}

impl From<sint32> for v64 {
    fn from(x: sint32) -> v64 {
        zigzag(x.x as i64).into()
    }
}

////////////////////////////////////////////// sint64 //////////////////////////////////////////////

pub struct sint64 {
    x: i64,
}

impl<'a> FieldType<'a> for sint64 {
    const WIRE_TYPE: WireType = WireType::Varint;
    const LENGTH_PREFIXED: bool = false;

    type NativeType = i64;

    fn into_native(self) -> Self::NativeType {
        self.x
    }
}

impl Packable for sint64 {
    fn pack_sz(&self) -> usize {
        let v: v64 = self.into();
        v.pack_sz()
    }

    fn pack<'a>(&self, buf: &'a mut[u8]) {
        let v: v64 = self.into();
        v.pack(buf)
    }
}

impl<'a> Unpackable<'a> for sint64 {
    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let (v, buf) = v64::unpack(buf)?;
        let x: i64 = unzigzag(v.into());
        Ok((x.into(), buf))
    }
}

impl From<i64> for sint64 {
    fn from(x: i64) -> Self {
        Self {
            x,
        }
    }
}

impl From<&i64> for sint64 {
    fn from(x: &i64) -> Self {
        Self {
            x: *x,
        }
    }
}

impl From<&sint64> for v64 {
    fn from(x: &sint64) -> v64 {
        zigzag(x.x as i64).into()
    }
}

impl From<sint64> for v64 {
    fn from(x: sint64) -> v64 {
        zigzag(x.x as i64).into()
    }
}

/////////////////////////////////////////////// Bool ///////////////////////////////////////////////

pub struct Bool {
    b: bool,
}

impl<'a> FieldType<'a> for Bool {
    const WIRE_TYPE: WireType = WireType::Varint;
    const LENGTH_PREFIXED: bool = false;

    type NativeType = bool;

    fn into_native(self) -> Self::NativeType {
        self.b
    }
}

impl Packable for Bool {
    fn pack_sz(&self) -> usize {
        let v: v64 = self.into();
        v.pack_sz()
    }

    fn pack<'a>(&self, buf: &'a mut[u8]) {
        let v: v64 = self.into();
        v.pack(buf)
    }
}

impl<'a> Unpackable<'a> for Bool {
    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let (v, buf) = v64::unpack(buf)?;
        let x: u64 = v.into();
        let b = if x == 0 {
            false
        } else {
            true
        };
        Ok((b.into(), buf))
    }
}

impl From<bool> for Bool {
    fn from(b: bool) -> Self {
        Self {
            b,
        }
    }
}

impl From<&bool> for Bool {
    fn from(b: &bool) -> Self {
        Self {
            b: *b,
        }
    }
}

impl From<&Bool> for v64 {
    fn from(b: &Bool) -> v64 {
        if b.b { 1 } else { 0 }.into()
    }
}

impl From<Bool> for v64 {
    fn from(b: Bool) -> v64 {
        if b.b { 1 } else { 0 }.into()
    }
}

////////////////////////////////////////////// fixed32 //////////////////////////////////////////////

pub struct fixed32 {
    x: u32,
}

impl<'a> FieldType<'a> for fixed32 {
    const WIRE_TYPE: WireType = WireType::ThirtyTwo;
    const LENGTH_PREFIXED: bool = false;

    type NativeType = u32;

    fn into_native(self) -> Self::NativeType {
        self.x
    }
}

impl Packable for fixed32 {
    fn pack_sz(&self) -> usize {
        self.x.pack_sz()
    }

    fn pack<'a>(&self, buf: &'a mut[u8]) {
        self.x.pack(buf)
    }
}

impl<'a> Unpackable<'a> for fixed32 {
    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let (x, buf) = u32::unpack(buf)?;
        Ok((x.into(), buf))
    }
}

impl From<u32> for fixed32 {
    fn from(x: u32) -> Self {
        Self {
            x,
        }
    }
}

impl From<&u32> for fixed32 {
    fn from(x: &u32) -> Self {
        Self {
            x: *x,
        }
    }
}

////////////////////////////////////////////// fixed64 //////////////////////////////////////////////

pub struct fixed64 {
    x: u64,
}

impl<'a> FieldType<'a> for fixed64 {
    const WIRE_TYPE: WireType = WireType::SixtyFour;
    const LENGTH_PREFIXED: bool = false;

    type NativeType = u64;

    fn into_native(self) -> Self::NativeType {
        self.x
    }
}

impl Packable for fixed64 {
    fn pack_sz(&self) -> usize {
        self.x.pack_sz()
    }

    fn pack<'a>(&self, buf: &'a mut[u8]) {
        self.x.pack(buf)
    }
}

impl<'a> Unpackable<'a> for fixed64 {
    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let (x, buf) = u64::unpack(buf)?;
        Ok((x.into(), buf))
    }
}

impl From<u64> for fixed64 {
    fn from(x: u64) -> Self {
        Self {
            x,
        }
    }
}

impl From<&u64> for fixed64 {
    fn from(x: &u64) -> Self {
        Self {
            x: *x,
        }
    }
}

///////////////////////////////////////////// sfixed32 //////////////////////////////////////////////

pub struct sfixed32 {
    x: i32,
}

impl<'a> FieldType<'a> for sfixed32 {
    const WIRE_TYPE: WireType = WireType::ThirtyTwo;
    const LENGTH_PREFIXED: bool = false;

    type NativeType = i32;

    fn into_native(self) -> Self::NativeType {
        self.x
    }
}

impl Packable for sfixed32 {
    fn pack_sz(&self) -> usize {
        self.x.pack_sz()
    }

    fn pack<'a>(&self, buf: &'a mut[u8]) {
        self.x.pack(buf)
    }
}

impl<'a> Unpackable<'a> for sfixed32 {
    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let (x, buf) = i32::unpack(buf)?;
        Ok((x.into(), buf))
    }
}

impl From<i32> for sfixed32 {
    fn from(x: i32) -> Self {
        Self {
            x,
        }
    }
}

impl From<&i32> for sfixed32 {
    fn from(x: &i32) -> Self {
        Self {
            x: *x,
        }
    }
}

///////////////////////////////////////////// sfixed64 //////////////////////////////////////////////

pub struct sfixed64 {
    x: i64,
}

impl<'a> FieldType<'a> for sfixed64 {
    const WIRE_TYPE: WireType = WireType::SixtyFour;
    const LENGTH_PREFIXED: bool = false;

    type NativeType = i64;

    fn into_native(self) -> Self::NativeType {
        self.x
    }
}

impl Packable for sfixed64 {
    fn pack_sz(&self) -> usize {
        self.x.pack_sz()
    }

    fn pack<'a>(&self, buf: &'a mut[u8]) {
        self.x.pack(buf)
    }
}

impl<'a> Unpackable<'a> for sfixed64 {
    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let (x, buf) = i64::unpack(buf)?;
        Ok((x.into(), buf))
    }
}

impl From<i64> for sfixed64 {
    fn from(x: i64) -> Self {
        Self {
            x,
        }
    }
}

impl From<&i64> for sfixed64 {
    fn from(x: &i64) -> Self {
        Self {
            x: *x,
        }
    }
}

/////////////////////////////////////////////// float //////////////////////////////////////////////

pub struct float {
    x: f32,
}

impl<'a> FieldType<'a> for float {
    const WIRE_TYPE: WireType = WireType::SixtyFour;
    const LENGTH_PREFIXED: bool = false;

    type NativeType = f32;

    fn into_native(self) -> Self::NativeType {
        self.x
    }
}

impl Packable for float {
    fn pack_sz(&self) -> usize {
        self.x.pack_sz()
    }

    fn pack<'a>(&self, buf: &'a mut[u8]) {
        self.x.pack(buf)
    }
}

impl<'a> Unpackable<'a> for float {
    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let (x, buf) = f32::unpack(buf)?;
        Ok((x.into(), buf))
    }
}

impl From<f32> for float {
    fn from(x: f32) -> Self {
        Self {
            x,
        }
    }
}

impl From<&f32> for float {
    fn from(x: &f32) -> Self {
        Self {
            x: *x,
        }
    }
}

////////////////////////////////////////////// double //////////////////////////////////////////////

pub struct double {
    x: f64,
}

impl<'a> FieldType<'a> for double {
    const WIRE_TYPE: WireType = WireType::SixtyFour;
    const LENGTH_PREFIXED: bool = false;

    type NativeType = f64;

    fn into_native(self) -> Self::NativeType {
        self.x
    }
}

impl Packable for double {
    fn pack_sz(&self) -> usize {
        self.x.pack_sz()
    }

    fn pack<'a>(&self, buf: &'a mut[u8]) {
        self.x.pack(buf)
    }
}

impl<'a> Unpackable<'a> for double {
    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let (x, buf) = f64::unpack(buf)?;
        Ok((x.into(), buf))
    }
}

impl From<f64> for double {
    fn from(x: f64) -> Self {
        Self {
            x,
        }
    }
}

impl From<&f64> for double {
    fn from(x: &f64) -> Self {
        Self {
            x: *x,
        }
    }
}

/////////////////////////////////////////////// bytes //////////////////////////////////////////////

pub struct bytes<'a>(&'a [u8]);

impl<'a> FieldType<'a> for bytes<'a> {
    const WIRE_TYPE: WireType = WireType::LengthDelimited;
    const LENGTH_PREFIXED: bool = true;

    type NativeType = &'a [u8];

    fn into_native(self) -> Self::NativeType {
        self.0
    }
}

impl<'a> Packable for bytes<'a> {
    fn pack_sz(&self) -> usize {
        self.0.pack_sz()
    }

    fn pack<'b>(&self, buf: &'b mut [u8]) {
        self.0.pack(buf)
    }
}

impl<'a> Unpackable<'a> for bytes<'a> {
    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let mut up = Unpacker::new(buf);
        let v: v64 = up.unpack()?;
        let v: usize = v.into();
        let rem = up.remain();
        if rem.len() < v {
            return Err(Error::BufferTooShort{ required: v, had: rem.len() });
        }
        Ok((Self(&rem[..v]), &rem[v..]))
    }
}

impl<'a> From<&'a [u8]> for bytes<'a> {
    fn from(x: &'a [u8]) -> Self {
        Self(x)
    }
}

impl<'a> From<&'a &'a [u8]> for bytes<'a> {
    fn from(x: &'a &'a [u8]) -> Self {
        Self(*x)
    }
}

////////////////////////////////////////////// bytes32 /////////////////////////////////////////////

pub struct bytes32([u8; 32]);

impl<'a> FieldType<'a> for bytes32 {
    const WIRE_TYPE: WireType = WireType::LengthDelimited;
    const LENGTH_PREFIXED: bool = true;

    type NativeType = [u8; 32];

    fn into_native(self) -> Self::NativeType {
        self.0
    }
}

impl Packable for bytes32 {
    fn pack_sz(&self) -> usize {
        let x: &[u8] = &self.0;
        <&[u8]>::pack_sz(&x)
    }

    fn pack<'b>(&self, buf: &'b mut [u8]) {
        let x: &[u8] = &self.0;
        <&[u8]>::pack(&x, buf)
    }
}

impl<'a> Unpackable<'a> for bytes32 {
    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let (arr, rem) = <[u8; 32]>::unpack(buf)?;
        Ok((bytes32(arr), rem))
    }
}

impl<'a> From<&[u8; 32]> for bytes32 {
    fn from(x: &[u8; 32]) -> Self {
        Self(*x)
    }
}

////////////////////////////////////////////// buffer //////////////////////////////////////////////

pub struct buffer(Buffer);

impl FieldType<'_> for buffer {
    const WIRE_TYPE: WireType = WireType::LengthDelimited;
    const LENGTH_PREFIXED: bool = true;

    type NativeType = Buffer;

    fn into_native(self) -> Self::NativeType {
        self.0
    }
}

impl Packable for buffer {
    fn pack_sz(&self) -> usize {
        let slice: &[u8] = &self.0.buf;
        slice.pack_sz()
    }

    fn pack<'b>(&self, buf: &'b mut [u8]) {
        let slice: &[u8] = &self.0.buf;
        slice.pack(buf)
    }
}

impl<'a> Unpackable<'a> for buffer {
    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let mut up = Unpacker::new(buf);
        let v: v64 = up.unpack()?;
        let v: usize = v.into();
        let rem = up.remain();
        if rem.len() < v {
            return Err(Error::BufferTooShort{ required: v, had: rem.len() });
        }
        let buf = Buffer {
            buf: rem[..v].into(),
        };
        Ok((Self(buf), &rem[v..]))
    }
}

impl<'a> From<&'a Buffer> for buffer {
    fn from(x: &'a Buffer) -> Self {
        Self(x.clone())
    }
}

impl<'a> From<&'a [u8]> for buffer {
    fn from(x: &'a [u8]) -> Self {
        Self(Buffer {
             buf: x.to_vec(),
        })
    }
}

///////////////////////////////////////////// string ////////////////////////////////////////////

pub struct string {
    s: String,
}

impl<'a> FieldType<'a> for string {
    const WIRE_TYPE: WireType = WireType::LengthDelimited;
    const LENGTH_PREFIXED: bool = true;

    type NativeType = String;

    fn into_native(self) -> Self::NativeType {
        self.s
    }
}

impl<'a> Packable for string {
    fn pack_sz(&self) -> usize {
        self.s.as_bytes().pack_sz()
    }

    fn pack<'b>(&self, buf: &'b mut [u8]) {
        self.s.as_bytes().pack(buf)
    }
}

impl<'a> Unpackable<'a> for string {
    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let mut up = Unpacker::new(buf);
        let v: v64 = up.unpack()?;
        let v: usize = v.into();
        let rem = up.remain();
        if rem.len() < v {
            return Err(Error::BufferTooShort{ required: v, had: rem.len() });
        }
        let x: &'a [u8] = &rem[..v];
        let s: &'a str = match std::str::from_utf8(x) {
            Ok(s) => { s },
            Err(_) => { unimplemented!(); /* TODO(rescriva) */ },
        };
        let s: String = String::from(s);
        Ok((Self{s}, &rem[v..]))
    }
}

impl From<&String> for string {
    fn from(s: &String) -> string {
        string{s: s.to_string()}
    }
}

///////////////////////////////////////////// stringref ////////////////////////////////////////////

pub struct stringref<'a>(&'a str);

impl<'a> FieldType<'a> for stringref<'a> {
    const WIRE_TYPE: WireType = WireType::LengthDelimited;
    const LENGTH_PREFIXED: bool = true;

    type NativeType = &'a str;

    fn into_native(self) -> Self::NativeType {
        self.0
    }
}

impl<'a> Packable for stringref<'a> {
    fn pack_sz(&self) -> usize {
        self.0.as_bytes().pack_sz()
    }

    fn pack<'b>(&self, buf: &'b mut [u8]) {
        self.0.as_bytes().pack(buf)
    }
}

impl<'a> Unpackable<'a> for stringref<'a> {
    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let mut up = Unpacker::new(buf);
        let v: v64 = up.unpack()?;
        let v: usize = v.into();
        let rem = up.remain();
        if rem.len() < v {
            return Err(Error::BufferTooShort{ required: v, had: rem.len() });
        }
        let x: &'a [u8] = &rem[..v];
        let s: &'a str = match std::str::from_utf8(x) {
            Ok(s) => { s },
            Err(_) => { unimplemented!(); /* TODO(rescriva) */ },
        };
        Ok((Self(s), &rem[v..]))
    }
}

impl<'a> From<&'a str> for stringref<'a> {
    fn from(s: &'a str) -> stringref<'a> {
        stringref(s)
    }
}

impl<'a> From<&&'a str> for stringref<'a> {
    fn from(s: &&'a str) -> stringref<'a> {
        stringref(*s)
    }
}

////////////////////////////////////////////// message /////////////////////////////////////////////

pub struct message<M> {
    msg: M,
}

impl<'a, M: Message<'a>> FieldType<'a> for message<M> {
    const WIRE_TYPE: WireType = WireType::LengthDelimited;
    const LENGTH_PREFIXED: bool = true;

    type NativeType = M;

    fn into_native(self) -> Self::NativeType {
        self.msg
    }
}

impl<'a, M: Message<'a>> Packable for message<M> {
    fn pack_sz(&self) -> usize {
        stack_pack(&self.msg).length_prefixed().pack_sz()
    }

    fn pack<'b>(&self, buf: &'b mut [u8]) {
        stack_pack(&self.msg).length_prefixed().pack(buf)
    }
}

impl<'a, M: Message<'a>> Unpackable<'a> for message<M> {
    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let mut up = Unpacker::new(buf);
        let v: v64 = up.unpack()?;
        let v: usize = v.into();
        let rem = up.remain();
        // TODO(rescrv): this pattern multiple times; try to move to Unpacker.
        if rem.len() < v {
            return Err(Error::BufferTooShort{ required: v, had: rem.len() });
        }
        let buf: &'b [u8] = &rem[..v];
        let rem: &'b [u8] = &rem[v..];
        let (m, empty): (M, &'a [u8]) = <M as Unpackable>::unpack(buf)?;
        // TODO(rescrv): assert is nasty
        assert_eq!(0, empty.len());
        Ok((Self::from(m), rem))
    }
}

impl<'a, M: Message<'a>> From<M> for message<M> {
    fn from(m: M) -> message<M> {
        Self {
            msg: m,
        }
    }
}

impl<'a, M: Message<'a>> From<&M> for message<M>
{
    fn from(m: &M) -> message<M> {
        Self {
            msg: m.clone(),
        }
    }
}

/////////////////////////////////////////// Vec<message> ///////////////////////////////////////////

impl<'a, M: Message<'a>> FieldType<'a> for message<Vec<M>> {
    const WIRE_TYPE: WireType = WireType::LengthDelimited;
    const LENGTH_PREFIXED: bool = true;

    type NativeType = M;

    fn into_native(self) -> Self::NativeType {
        assert_eq!(1, self.msg.len(), "assume vec always has exactly one M");
        let mut x = self;
        x.msg.pop().unwrap()
    }
}

impl<'a, M: Message<'a>> Packable for message<Vec<M>> {
    fn pack_sz(&self) -> usize {
        assert_eq!(1, self.msg.len(), "assume vec always has exactly one M");
        self.msg[0].pack_sz()
    }

    fn pack<'b>(&self, buf: &'b mut [u8]) {
        assert_eq!(1, self.msg.len(), "assume vec always has exactly one M");
        self.msg[0].pack(buf);
    }
}

impl<'a, M: Message<'a>> Unpackable<'a> for message<Vec<M>> {
    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let mut up = Unpacker::new(buf);
        let px: message<M> = up.unpack()?;
        Ok((Self {
            msg: vec![px.msg],
        }, up.remain()))
    }
}

impl<'a, M: Message<'a>> From<M> for message<Vec<M>> {
    fn from(m: M) -> message<Vec<M>> {
        Self {
            msg: vec![m],
        }
    }
}

impl<'a, M: Message<'a>> From<&M> for message<Vec<M>>
{
    fn from(m: &M) -> message<Vec<M>> {
        Self {
            msg: vec![m.clone()],
        }
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use crate::*;
    use crate::field_types::*;

    #[test]
    fn int32() {
        let mut buf: [u8; 64] = [0u8; 64];
        let min = int32::from(i32::min_value());
        let neg = int32::from(-1);
        let zero = int32::from(0);
        let one = int32::from(1);
        let max = int32::from(i32::max_value());
        let exp = &[
            0x80, 0x80, 0x80, 0x80, 0xf8, 0xff, 0xff, 0xff, 0xff, 1,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 1,
            0,
            1,
            0xff, 0xff, 0xff, 0xff, 0x07,
        ];
        let buf = stack_pack(min)
            .pack(neg)
            .pack(zero)
            .pack(one)
            .pack(max)
            .into_slice(&mut buf);
        assert_eq!(exp, buf, "packing int32 field failed");
        let mut up = Unpacker::new(buf);
        let min: int32 = up.unpack().unwrap();
        let neg: int32 = up.unpack().unwrap();
        let zero: int32 = up.unpack().unwrap();
        let one: int32 = up.unpack().unwrap();
        let max: int32 = up.unpack().unwrap();
        assert_eq!(i32::min_value(), min.into_native());
        assert_eq!(-1i32, neg.into_native());
        assert_eq!(0i32, zero.into_native());
        assert_eq!(1i32, one.into_native());
        assert_eq!(i32::max_value(), max.into_native());
    }

    #[test]
    fn int64() {
        let mut buf: [u8; 64] = [0u8; 64];
        let min = int64::from(i64::min_value());
        let neg = int64::from(-1);
        let zero = int64::from(0);
        let one = int64::from(1);
        let max = int64::from(i64::max_value());
        let exp = &[
            0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 1,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 1,
            0,
            1,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f,
        ];
        let buf = stack_pack(min)
            .pack(neg)
            .pack(zero)
            .pack(one)
            .pack(max)
            .into_slice(&mut buf);
        assert_eq!(exp, buf, "packing int64 field failed");
        let mut up = Unpacker::new(buf);
        let min: int64 = up.unpack().unwrap();
        let neg: int64 = up.unpack().unwrap();
        let zero: int64 = up.unpack().unwrap();
        let one: int64 = up.unpack().unwrap();
        let max: int64 = up.unpack().unwrap();
        assert_eq!(i64::min_value(), min.into_native());
        assert_eq!(-1i64, neg.into_native());
        assert_eq!(0i64, zero.into_native());
        assert_eq!(1i64, one.into_native());
        assert_eq!(i64::max_value(), max.into_native());
    }

    #[test]
    fn uint32() {
        let mut buf: [u8; 64] = [0u8; 64];
        let zero = uint32::from(0);
        let one = uint32::from(1);
        let max = uint32::from(u32::max_value());
        let exp = &[
            0,
            1,
            0xff, 0xff, 0xff, 0xff, 0x0f,
        ];
        let buf = stack_pack(zero)
            .pack(one)
            .pack(max)
            .into_slice(&mut buf);
        assert_eq!(exp, buf, "packing uint32 field failed");
        let mut up = Unpacker::new(buf);
        let zero: uint32 = up.unpack().unwrap();
        let one: uint32 = up.unpack().unwrap();
        let max: uint32 = up.unpack().unwrap();
        assert_eq!(0u32, zero.into_native());
        assert_eq!(1u32, one.into_native());
        assert_eq!(u32::max_value(), max.into_native());
    }

    #[test]
    fn uint64() {
        let mut buf: [u8; 64] = [0u8; 64];
        let zero = uint64::from(0);
        let one = uint64::from(1);
        let max = uint64::from(u64::max_value());
        let exp = &[
            0,
            1,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 1,
        ];
        let buf = stack_pack(zero)
            .pack(one)
            .pack(max)
            .into_slice(&mut buf);
        assert_eq!(exp, buf, "packing uint64 field failed");
        let mut up = Unpacker::new(buf);
        let zero: uint64 = up.unpack().unwrap();
        let one: uint64 = up.unpack().unwrap();
        let max: uint64 = up.unpack().unwrap();
        assert_eq!(0u64, zero.into_native());
        assert_eq!(1u64, one.into_native());
        assert_eq!(u64::max_value(), max.into_native());
    }

    #[test]
    fn sint32() {
        let mut buf: [u8; 64] = [0u8; 64];
        let min = sint32::from(i32::min_value());
        let neg = sint32::from(-1);
        let zero = sint32::from(0);
        let one = sint32::from(1);
        let max = sint32::from(i32::max_value());
        let exp = &[
            0xff, 0xff, 0xff, 0xff, 0x0f,
            1,
            0,
            2,
            0xfe, 0xff, 0xff, 0xff, 0x0f,
        ];
        let buf = stack_pack(min)
            .pack(neg)
            .pack(zero)
            .pack(one)
            .pack(max)
            .into_slice(&mut buf);
        assert_eq!(exp, buf, "packing sint32 field failed");
        let mut up = Unpacker::new(buf);
        let min: sint32 = up.unpack().unwrap();
        let neg: sint32 = up.unpack().unwrap();
        let zero: sint32 = up.unpack().unwrap();
        let one: sint32 = up.unpack().unwrap();
        let max: sint32 = up.unpack().unwrap();
        assert_eq!(i32::min_value(), min.into_native());
        assert_eq!(-1i32, neg.into_native());
        assert_eq!(0i32, zero.into_native());
        assert_eq!(1i32, one.into_native());
        assert_eq!(i32::max_value(), max.into_native());
    }

    #[test]
    fn sint64() {
        let mut buf: [u8; 64] = [0u8; 64];
        let min = sint64::from(i64::min_value());
        let neg = sint64::from(-1);
        let zero = sint64::from(0);
        let one = sint64::from(1);
        let max = sint64::from(i64::max_value());
        let exp = &[
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 1,
            1,
            0,
            2,
            0xfe, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 1,
        ];
        let buf = stack_pack(min)
            .pack(neg)
            .pack(zero)
            .pack(one)
            .pack(max)
            .into_slice(&mut buf);
        assert_eq!(exp, buf, "packing sint64 field failed");
        let mut up = Unpacker::new(buf);
        let min: sint64 = up.unpack().unwrap();
        let neg: sint64 = up.unpack().unwrap();
        let zero: sint64 = up.unpack().unwrap();
        let one: sint64 = up.unpack().unwrap();
        let max: sint64 = up.unpack().unwrap();
        assert_eq!(i64::min_value(), min.into_native());
        assert_eq!(-1i64, neg.into_native());
        assert_eq!(0i64, zero.into_native());
        assert_eq!(1i64, one.into_native());
        assert_eq!(i64::max_value(), max.into_native());
    }

    #[test]
    #[allow(non_snake_case)]
    fn Bool() {
        let mut buf: [u8; 64] = [0u8; 64];
        let False = Bool::from(false);
        let True = Bool::from(true);
        let exp = &[ 0, 1, ];
        let buf = stack_pack(False)
            .pack(True)
            .into_slice(&mut buf);
        assert_eq!(exp, buf, "packing Bool field failed");
        let mut up = Unpacker::new(buf);
        let False: Bool = up.unpack().unwrap();
        let True: Bool = up.unpack().unwrap();
        assert_eq!(false, False.into_native());
        assert_eq!(true, True.into_native());
    }

    #[test]
    fn fixed64() {
        let mut buf: [u8; 64] = [0u8; 64];
        let zero = fixed64::from(0);
        let one = fixed64::from(1);
        let max = fixed64::from(u64::max_value());
        let exp = &[
            0, 0, 0, 0, 0, 0, 0, 0,
            1, 0, 0, 0, 0, 0, 0, 0,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        ];
        let buf = stack_pack(zero)
            .pack(one)
            .pack(max)
            .into_slice(&mut buf);
        assert_eq!(exp, buf, "packing fixed64 field failed");
        let mut up = Unpacker::new(buf);
        let zero: fixed64 = up.unpack().unwrap();
        let one: fixed64 = up.unpack().unwrap();
        let max: fixed64 = up.unpack().unwrap();
        assert_eq!(0u64, zero.into_native());
        assert_eq!(1u64, one.into_native());
        assert_eq!(u64::max_value(), max.into_native());
    }

    #[test]
    fn sfixed64() {
        let mut buf: [u8; 64] = [0u8; 64];
        let min = sfixed64::from(i64::min_value());
        let neg = sfixed64::from(-1);
        let zero = sfixed64::from(0);
        let one = sfixed64::from(1);
        let max = sfixed64::from(i64::max_value());
        let exp: &[u8] = &[
            0, 0, 0, 0, 0, 0, 0, 0x80,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0, 0, 0, 0, 0, 0, 0, 0,
            1, 0, 0, 0, 0, 0, 0, 0,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f,
        ];
        let buf = stack_pack(min)
            .pack(neg)
            .pack(zero)
            .pack(one)
            .pack(max)
            .into_slice(&mut buf);
        assert_eq!(exp, buf, "packing sfixed64 field failed");
        let mut up = Unpacker::new(buf);
        let min: sfixed64 = up.unpack().unwrap();
        let neg: sfixed64 = up.unpack().unwrap();
        let zero: sfixed64 = up.unpack().unwrap();
        let one: sfixed64 = up.unpack().unwrap();
        let max: sfixed64 = up.unpack().unwrap();
        assert_eq!(i64::min_value(), min.into_native());
        assert_eq!(-1i64, neg.into_native());
        assert_eq!(0i64, zero.into_native());
        assert_eq!(1i64, one.into_native());
        assert_eq!(i64::max_value(), max.into_native());
    }

    #[test]
    fn double() {
        let mut buf: [u8; 64] = [0u8; 64];
        let pi = double::from(3.14159);
        let exp: &[u8] = &[
            0x6e, 0x86, 0x1b, 0xf0, 0xf9, 0x21, 0x09, 0x40,
        ];
        let buf = stack_pack(pi).into_slice(&mut buf);
        assert_eq!(exp, buf, "packing double field failed");
        let mut up = Unpacker::new(buf);
        let pi: double = up.unpack().unwrap();
        assert_eq!(3.14159, pi.into_native());
    }

    #[test]
    fn fixed32() {
        let mut buf: [u8; 64] = [0u8; 64];
        let zero = fixed32::from(0);
        let one = fixed32::from(1);
        let max = fixed32::from(u32::max_value());
        let exp = &[
            0, 0, 0, 0,
            1, 0, 0, 0,
            0xff, 0xff, 0xff, 0xff,
        ];
        let buf = stack_pack(zero)
            .pack(one)
            .pack(max)
            .into_slice(&mut buf);
        assert_eq!(exp, buf, "packing fixed32 field failed");
        let mut up = Unpacker::new(buf);
        let zero: fixed32 = up.unpack().unwrap();
        let one: fixed32 = up.unpack().unwrap();
        let max: fixed32 = up.unpack().unwrap();
        assert_eq!(0u32, zero.into_native());
        assert_eq!(1u32, one.into_native());
        assert_eq!(u32::max_value(), max.into_native());
    }

    #[test]
    fn sfixed32() {
        let mut buf: [u8; 64] = [0u8; 64];
        let min = sfixed32::from(i32::min_value());
        let neg = sfixed32::from(-1);
        let zero = sfixed32::from(0);
        let one = sfixed32::from(1);
        let max = sfixed32::from(i32::max_value());
        let exp: &[u8] = &[
            0, 0, 0, 0x80,
            0xff, 0xff, 0xff, 0xff,
            0, 0, 0, 0,
            1, 0, 0, 0,
            0xff, 0xff, 0xff, 0x7f,
        ];
        let buf = stack_pack(min)
            .pack(neg)
            .pack(zero)
            .pack(one)
            .pack(max)
            .into_slice(&mut buf);
        assert_eq!(exp, buf, "packing sfixed32 field failed");
        let mut up = Unpacker::new(buf);
        let min: sfixed32 = up.unpack().unwrap();
        let neg: sfixed32 = up.unpack().unwrap();
        let zero: sfixed32 = up.unpack().unwrap();
        let one: sfixed32 = up.unpack().unwrap();
        let max: sfixed32 = up.unpack().unwrap();
        assert_eq!(i32::min_value(), min.into_native());
        assert_eq!(-1i32, neg.into_native());
        assert_eq!(0i32, zero.into_native());
        assert_eq!(1i32, one.into_native());
        assert_eq!(i32::max_value(), max.into_native());
    }

    #[test]
    fn float() {
        let mut buf: [u8; 64] = [0u8; 64];
        let pi = float::from(3.14159);
        let exp: &[u8] = &[
            0xd0, 0x0f, 0x49, 0x40,
        ];
        let buf = stack_pack(pi).into_slice(&mut buf);
        assert_eq!(exp, buf, "packing float field failed");
        let mut up = Unpacker::new(buf);
        let pi: float = up.unpack().unwrap();
        assert_eq!(3.14159f32, pi.into_native());
    }

    #[test]
    fn bytes() {
        const BYTES: &[u8] = &[0xff, 0x00];
        let mut buf: [u8; 64] = [0u8; 64];
        let msg = bytes::from(BYTES);
        let exp: &[u8] = &[0x2, 0xff, 0x00];
        let buf = stack_pack(msg).into_slice(&mut buf);
        assert_eq!(exp, buf, "packing bytes field failed");
        let mut up = Unpacker::new(buf);
        let msg: bytes = up.unpack().unwrap();
        let msg: &[u8] = msg.into_native();
        assert_eq!(BYTES, msg);
    }

    #[test]
    fn stringref() {
        const STRING: &str = "string \u{1F600}";
        let mut buf: [u8; 64] = [0u8; 64];
        let msg = stringref::from(STRING);
        let exp: &[u8] = &[0xb, 0x73, 0x74, 0x72, 0x69, 0x6e, 0x67, 0x20, 0xf0, 0x9f, 0x98, 0x80];
        let buf = stack_pack(msg).into_slice(&mut buf);
        assert_eq!(exp, buf, "packing stringref field failed");
        let mut up = Unpacker::new(buf);
        let msg: stringref = up.unpack().unwrap();
        let msg: &str = msg.into_native();
        assert_eq!(STRING, msg);
    }
}
