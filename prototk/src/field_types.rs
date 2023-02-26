#![allow(non_camel_case_types)]

// We allow non-CamelCase types here because we want the struct names to appear as close to they do
// in the proto documentation and official implementation.  Thus, `uint64` is how we represent the
// type of `u64`.  The primary use of these field types is to pull the token from a field annotated
// with e.g. #[prototk(7, uint64)], where the uint64 token is used verbatim.

use std::convert::TryInto;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use buffertk::{stack_pack, Buffer, Unpackable, Unpacker};

use super::*;

/////////////////////////////////////////////// int32 //////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct int32(i32);

impl<'a> FieldType<'a> for int32 {
    const WIRE_TYPE: WireType = WireType::Varint;

    type NativeType = i32;

    fn from_native(x: Self::NativeType) -> Self {
        Self(x)
    }
}

impl<'a> FieldHelper<'a, int32> for i32 {
    fn prototk_pack_sz(tag: &Tag, field: &Self) -> usize {
        let v: v64 = v64::from(*field);
        stack_pack(tag).pack(v).pack_sz()
    }

    fn prototk_pack(tag: &Tag, field: &Self, out: &mut [u8]) {
        let v: v64 = v64::from(*field);
        stack_pack(tag).pack(v).into_slice(out);
    }

    fn prototk_convert_field<'b>(proto: int32, out: &'b mut Self) where 'a: 'b, {
        *out = proto.0;
    }

    fn prototk_convert_variant(proto: int32) -> Self {
        proto.0
    }
}

impl<'a> Unpackable<'a> for int32 {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let (v, buf) = v64::unpack(buf)?;
        let x: i32 = v.try_into()?;
        Ok((int32(x), buf))
    }
}

/////////////////////////////////////////////// int64 //////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct int64(i64);

impl<'a> FieldType<'a> for int64 {
    const WIRE_TYPE: WireType = WireType::Varint;

    type NativeType = i64;

    fn from_native(x: Self::NativeType) -> Self {
        Self(x)
    }
}

impl<'a> FieldHelper<'a, int64> for i64 {
    fn prototk_pack_sz(tag: &Tag, field: &Self) -> usize {
        let v: v64 = v64::from(*field);
        stack_pack(tag).pack(v).pack_sz()
    }

    fn prototk_pack(tag: &Tag, field: &Self, out: &mut [u8]) {
        let v: v64 = v64::from(*field);
        stack_pack(tag).pack(v).into_slice(out);
    }

    fn prototk_convert_field<'b>(proto: int64, out: &'b mut Self) where 'a: 'b {
        *out = proto.0;
    }

    fn prototk_convert_variant(proto: int64) -> Self {
        proto.0
    }
}

impl<'a> Unpackable<'a> for int64 {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let (v, buf) = v64::unpack(buf)?;
        let x: i64 = v.into();
        Ok((int64(x), buf))
    }
}

////////////////////////////////////////////// uint32 //////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct uint32(u32);

impl<'a> FieldType<'a> for uint32 {
    const WIRE_TYPE: WireType = WireType::Varint;

    type NativeType = u32;

    fn from_native(x: Self::NativeType) -> Self {
        Self(x)
    }
}

impl<'a> FieldHelper<'a, uint32> for u32 {
    fn prototk_pack_sz(tag: &Tag, field: &Self) -> usize {
        let v: v64 = v64::from(*field);
        stack_pack(tag).pack(v).pack_sz()
    }

    fn prototk_pack(tag: &Tag, field: &Self, out: &mut [u8]) {
        let v: v64 = v64::from(*field);
        stack_pack(tag).pack(v).into_slice(out);
    }

    fn prototk_convert_field<'b>(proto: uint32, out: &'b mut Self) where 'a: 'b {
        *out = proto.0;
    }

    fn prototk_convert_variant(proto: uint32) -> Self {
        proto.0
    }
}

impl<'a> Unpackable<'a> for uint32 {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let (v, buf) = v64::unpack(buf)?;
        let x: u32 = v.try_into()?;
        Ok((uint32(x), buf))
    }
}

////////////////////////////////////////////// uint64 //////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct uint64(u64);

impl<'a> FieldType<'a> for uint64 {
    const WIRE_TYPE: WireType = WireType::Varint;

    type NativeType = u64;

    fn from_native(x: Self::NativeType) -> Self {
        Self(x)
    }
}

impl<'a> FieldHelper<'a, uint64> for u64 {
    fn prototk_pack_sz(tag: &Tag, field: &Self) -> usize {
        let v: v64 = v64::from(*field);
        stack_pack(tag).pack(v).pack_sz()
    }

    fn prototk_pack(tag: &Tag, field: &Self, out: &mut [u8]) {
        let v: v64 = v64::from(*field);
        stack_pack(tag).pack(v).into_slice(out);
    }

    fn prototk_convert_field<'b>(proto: uint64, out: &'b mut Self) where 'a: 'b {
        *out = proto.0;
    }

    fn prototk_convert_variant(proto: uint64) -> Self {
        proto.0
    }
}

impl<'a> Unpackable<'a> for uint64 {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let (v, buf) = v64::unpack(buf)?;
        let x: u64 = v.into();
        Ok((uint64(x), buf))
    }
}

////////////////////////////////////////////// sint32 //////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct sint32(i32);

impl<'a> FieldType<'a> for sint32 {
    const WIRE_TYPE: WireType = WireType::Varint;

    type NativeType = i32;

    fn from_native(x: Self::NativeType) -> Self {
        Self(x)
    }
}

impl<'a> FieldHelper<'a, sint32> for i32 {
    fn prototk_pack_sz(tag: &Tag, field: &Self) -> usize {
        let v: v64 = v64::from(zigzag(*field as i64));
        stack_pack(tag).pack(v).pack_sz()
    }

    fn prototk_pack(tag: &Tag, field: &Self, out: &mut [u8]) {
        let v: v64 = v64::from(zigzag(*field as i64));
        stack_pack(tag).pack(v).into_slice(out);
    }

    fn prototk_convert_field<'b>(proto: sint32, out: &'b mut Self) where 'a: 'b {
        *out = proto.0;
    }

    fn prototk_convert_variant(proto: sint32) -> Self {
        proto.0
    }
}

impl<'a> Unpackable<'a> for sint32 {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let (v, buf) = v64::unpack(buf)?;
        let x: i64 = unzigzag(v.into());
        let x: i32 = match x.try_into() {
            Ok(x) => x,
            Err(_) => {
                return Err(Error::SignedOverflow { value: x });
            }
        };
        Ok((sint32(x), buf))
    }
}

////////////////////////////////////////////// sint64 //////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct sint64(i64);

impl<'a> FieldType<'a> for sint64 {
    const WIRE_TYPE: WireType = WireType::Varint;

    type NativeType = i64;

    fn from_native(x: Self::NativeType) -> Self {
        Self(x)
    }
}

impl<'a> FieldHelper<'a, sint64> for i64 {
    fn prototk_pack_sz(tag: &Tag, field: &Self) -> usize {
        let v: v64 = v64::from(zigzag(*field));
        stack_pack(tag).pack(v).pack_sz()
    }

    fn prototk_pack(tag: &Tag, field: &Self, out: &mut [u8]) {
        let v: v64 = v64::from(zigzag(*field));
        stack_pack(tag).pack(v).into_slice(out);
    }

    fn prototk_convert_field<'b>(proto: sint64, out: &'b mut Self) where 'a: 'b {
        *out = proto.0;
    }

    fn prototk_convert_variant(proto: sint64) -> Self {
        proto.0
    }
}

impl<'a> Unpackable<'a> for sint64 {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let (v, buf) = v64::unpack(buf)?;
        let x: i64 = unzigzag(v.into());
        Ok((sint64(x), buf))
    }
}

////////////////////////////////////////////// fixed32 //////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct fixed32(u32);

impl<'a> FieldType<'a> for fixed32 {
    const WIRE_TYPE: WireType = WireType::ThirtyTwo;

    type NativeType = u32;

    fn from_native(x: Self::NativeType) -> Self {
        Self(x)
    }
}

impl<'a> FieldHelper<'a, fixed32> for u32 {
    fn prototk_pack_sz(tag: &Tag, field: &Self) -> usize {
        stack_pack(tag).pack(field).pack_sz()
    }

    fn prototk_pack(tag: &Tag, field: &Self, out: &mut [u8]) {
        stack_pack(tag).pack(field).into_slice(out);
    }

    fn prototk_convert_field<'b>(proto: fixed32, out: &'b mut Self) where 'a: 'b {
        *out = proto.0;
    }

    fn prototk_convert_variant(proto: fixed32) -> Self {
        proto.0
    }
}

impl<'a> Unpackable<'a> for fixed32 {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let (x, buf) = u32::unpack(buf)?;
        Ok((fixed32(x), buf))
    }
}

////////////////////////////////////////////// fixed64 //////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct fixed64(u64);

impl<'a> FieldType<'a> for fixed64 {
    const WIRE_TYPE: WireType = WireType::SixtyFour;

    type NativeType = u64;

    fn from_native(x: Self::NativeType) -> Self {
        Self(x)
    }
}

impl<'a> FieldHelper<'a, fixed64> for u64 {
    fn prototk_pack_sz(tag: &Tag, field: &Self) -> usize {
        stack_pack(tag).pack(field).pack_sz()
    }

    fn prototk_pack(tag: &Tag, field: &Self, out: &mut [u8]) {
        stack_pack(tag).pack(field).into_slice(out);
    }

    fn prototk_convert_field<'b>(proto: fixed64, out: &'b mut Self) where 'a: 'b {
        *out = proto.0;
    }

    fn prototk_convert_variant(proto: fixed64) -> Self {
        proto.0
    }
}

impl<'a> Unpackable<'a> for fixed64 {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let (x, buf) = u64::unpack(buf)?;
        Ok((fixed64(x), buf))
    }
}

///////////////////////////////////////////// sfixed32 //////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct sfixed32(i32);

impl<'a> FieldType<'a> for sfixed32 {
    const WIRE_TYPE: WireType = WireType::ThirtyTwo;

    type NativeType = i32;

    fn from_native(x: Self::NativeType) -> Self {
        Self(x)
    }
}

impl<'a> FieldHelper<'a, sfixed32> for i32 {
    fn prototk_pack_sz(tag: &Tag, field: &Self) -> usize {
        stack_pack(tag).pack(field).pack_sz()
    }

    fn prototk_pack(tag: &Tag, field: &Self, out: &mut [u8]) {
        stack_pack(tag).pack(field).into_slice(out);
    }

    fn prototk_convert_field<'b>(proto: sfixed32, out: &'b mut Self) where 'a: 'b {
        *out = proto.0;
    }

    fn prototk_convert_variant(proto: sfixed32) -> Self {
        proto.0
    }
}

impl<'a> Unpackable<'a> for sfixed32 {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let (x, buf) = i32::unpack(buf)?;
        Ok((sfixed32(x), buf))
    }
}

///////////////////////////////////////////// sfixed64 //////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct sfixed64(i64);

impl<'a> FieldType<'a> for sfixed64 {
    const WIRE_TYPE: WireType = WireType::SixtyFour;

    type NativeType = i64;

    fn from_native(x: Self::NativeType) -> Self {
        Self(x)
    }
}

impl<'a> FieldHelper<'a, sfixed64> for i64 {
    fn prototk_pack_sz(tag: &Tag, field: &Self) -> usize {
        stack_pack(tag).pack(field).pack_sz()
    }

    fn prototk_pack(tag: &Tag, field: &Self, out: &mut [u8]) {
        stack_pack(tag).pack(field).into_slice(out);
    }

    fn prototk_convert_field<'b>(proto: sfixed64, out: &'b mut Self) where 'a: 'b {
        *out = proto.0;
    }

    fn prototk_convert_variant(proto: sfixed64) -> Self {
        proto.0
    }
}

impl<'a> Unpackable<'a> for sfixed64 {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let (x, buf) = i64::unpack(buf)?;
        Ok((sfixed64(x), buf))
    }
}

/////////////////////////////////////////////// float //////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct float(f32);

impl<'a> FieldType<'a> for float {
    const WIRE_TYPE: WireType = WireType::SixtyFour;

    type NativeType = f32;

    fn from_native(x: Self::NativeType) -> Self {
        Self(x)
    }
}

impl<'a> FieldHelper<'a, float> for f32 {
    fn prototk_pack_sz(tag: &Tag, field: &Self) -> usize {
        stack_pack(tag).pack(field).pack_sz()
    }

    fn prototk_pack(tag: &Tag, field: &Self, out: &mut [u8]) {
        stack_pack(tag).pack(field).into_slice(out);
    }

    fn prototk_convert_field<'b>(proto: float, out: &'b mut Self) where 'a: 'b {
        *out = proto.0;
    }

    fn prototk_convert_variant(proto: float) -> Self {
        proto.0
    }
}

impl<'a> Unpackable<'a> for float {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let (x, buf) = f32::unpack(buf)?;
        Ok((float(x), buf))
    }
}

////////////////////////////////////////////// double //////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct double(f64);

impl<'a> FieldType<'a> for double {
    const WIRE_TYPE: WireType = WireType::SixtyFour;

    type NativeType = f64;

    fn from_native(x: Self::NativeType) -> Self {
        Self(x)
    }
}

impl<'a> FieldHelper<'a, double> for f64 {
    fn prototk_pack_sz(tag: &Tag, field: &Self) -> usize {
        stack_pack(tag).pack(field).pack_sz()
    }

    fn prototk_pack(tag: &Tag, field: &Self, out: &mut [u8]) {
        stack_pack(tag).pack(field).into_slice(out);
    }

    fn prototk_convert_field<'b>(proto: double, out: &'b mut Self) where 'a: 'b {
        *out = proto.0;
    }

    fn prototk_convert_variant(proto: double) -> Self {
        proto.0
    }
}

impl<'a> Unpackable<'a> for double {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let (x, buf) = f64::unpack(buf)?;
        Ok((double(x), buf))
    }
}

/////////////////////////////////////////////// Bool ///////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct Bool(bool);

impl<'a> FieldType<'a> for Bool {
    const WIRE_TYPE: WireType = WireType::Varint;

    type NativeType = bool;

    fn from_native(b: Self::NativeType) -> Self {
        Self(b)
    }
}

impl<'a> FieldHelper<'a, Bool> for bool {
    fn prototk_pack_sz(tag: &Tag, field: &Self) -> usize {
        let v: v64 = v64::from(if *field { 1 } else { 0 });
        stack_pack(tag).pack(v).pack_sz()
    }

    fn prototk_pack(tag: &Tag, field: &Self, out: &mut [u8]) {
        let v: v64 = v64::from(if *field { 1 } else { 0 });
        stack_pack(tag).pack(v).into_slice(out);
    }

    fn prototk_convert_field<'b>(proto: Bool, out: &'b mut Self) where 'a: 'b {
        *out = proto.0;
    }

    fn prototk_convert_variant(proto: Bool) -> Self {
        proto.0
    }
}

impl<'a> Unpackable<'a> for Bool {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let (v, buf) = v64::unpack(buf)?;
        let x: u64 = v.into();
        let b = x != 0;
        Ok((Bool(b), buf))
    }
}

/////////////////////////////////////////////// bytes //////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct bytes<'a>(&'a [u8]);

impl<'a> FieldType<'a> for bytes<'a> {
    const WIRE_TYPE: WireType = WireType::LengthDelimited;

    type NativeType = &'a [u8];

    fn from_native(x: Self::NativeType) -> Self {
        Self(x)
    }
}

impl<'a> FieldHelper<'a, bytes<'a>> for &'a [u8] {
    fn prototk_pack_sz(tag: &Tag, field: &Self) -> usize {
        stack_pack(tag).pack(field).pack_sz()
    }

    fn prototk_pack(tag: &Tag, field: &Self, out: &mut [u8]) {
        stack_pack(tag).pack(field).into_slice(out);
    }

    fn prototk_convert_field<'b>(proto: bytes<'a>, out: &'b mut Self) where 'a: 'b {
        *out = proto.0;
    }

    fn prototk_convert_variant(proto: bytes<'a>) -> Self {
        proto.0
    }
}

impl<'a> FieldHelper<'a, bytes<'a>> for Vec<u8> {
    fn prototk_pack_sz(tag: &Tag, field: &Self) -> usize {
        let field: &[u8] = field;
        stack_pack(tag).pack(field).pack_sz()
    }

    fn prototk_pack(tag: &Tag, field: &Self, out: &mut [u8]) {
        let field: &[u8] = field;
        stack_pack(tag).pack(field).into_slice(out);
    }

    fn prototk_convert_field<'b>(proto: bytes<'a>, out: &'b mut Self) where 'a: 'b {
        *out = proto.0.to_vec();
    }

    fn prototk_convert_variant(proto: bytes<'a>) -> Self {
        proto.0.to_vec()
    }
}

impl<'a> FieldHelper<'a, bytes<'a>> for Buffer {
    fn prototk_pack_sz(tag: &Tag, field: &Self) -> usize {
        let b: &[u8] = field.as_bytes();
        stack_pack(tag).pack(b).pack_sz()
    }

    fn prototk_pack(tag: &Tag, field: &Self, out: &mut [u8]) {
        let b: &[u8] = field.as_bytes();
        stack_pack(tag).pack(b).into_slice(out);
    }

    fn prototk_convert_field<'b>(proto: bytes<'a>, out: &'b mut Self) where 'a: 'b {
        *out = Buffer::from(proto.0);
    }

    fn prototk_convert_variant(proto: bytes<'a>) -> Self {
        Buffer::from(proto.0)
    }
}

impl<'a> FieldHelper<'a, bytes<'a>> for PathBuf {
    fn prototk_pack_sz(tag: &Tag, field: &Self) -> usize {
        let field: &[u8] = field.as_os_str().as_bytes();
        stack_pack(tag).pack(field).pack_sz()
    }

    fn prototk_pack(tag: &Tag, field: &Self, out: &mut [u8]) {
        let field: &[u8] = field.as_os_str().as_bytes();
        stack_pack(tag).pack(field).into_slice(out);
    }

    fn prototk_convert_field<'b>(proto: bytes<'a>, out: &'b mut Self) where 'a: 'b {
        *out = PathBuf::from(OsStr::from_bytes(proto.0));
    }

    fn prototk_convert_variant(proto: bytes<'a>) -> Self {
        PathBuf::from(OsStr::from_bytes(proto.0))
    }
}

impl<'a> Unpackable<'a> for bytes<'a> {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let mut up = Unpacker::new(buf);
        let v: v64 = up.unpack()?;
        let v: usize = v.into();
        let rem = up.remain();
        if rem.len() < v {
            return Err(Error::BufferTooShort {
                required: v,
                had: rem.len(),
            });
        }
        Ok((Self(&rem[..v]), &rem[v..]))
    }
}

////////////////////////////////////////////// bytes32 /////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct bytes32([u8; 32]);

impl<'a> FieldType<'a> for bytes32 {
    const WIRE_TYPE: WireType = WireType::LengthDelimited;

    type NativeType = [u8; 32];

    fn from_native(x: Self::NativeType) -> Self {
        Self(x)
    }
}

impl<'a> FieldHelper<'a, bytes32> for [u8; 32] {
    fn prototk_pack_sz(tag: &Tag, field: &Self) -> usize {
        let b: &[u8] = &*field;
        stack_pack(tag).pack(b).pack_sz()
    }

    fn prototk_pack(tag: &Tag, field: &Self, out: &mut [u8]) {
        let b: &[u8] = &*field;
        stack_pack(tag).pack(b).into_slice(out);
    }

    fn prototk_convert_field<'b>(proto: bytes32, out: &'b mut Self) where 'a: 'b {
        *out = proto.0;
    }

    fn prototk_convert_variant(proto: bytes32) -> Self {
        proto.0
    }
}

impl<'a> Unpackable<'a> for bytes32 {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let mut up = Unpacker::new(buf);
        let v: v64 = up.unpack()?;
        let v: usize = v.into();
        let rem = up.remain();
        if rem.len() < v {
            return Err(Error::BufferTooShort {
                required: v,
                had: rem.len(),
            });
        }
        if v < 32 {
            return Err(Error::BufferTooShort {
                required: 32,
                had: v.into(),
            });
        }
        if v != 32 {
            return Err(Error::WrongLength {
                required: 32,
                had: v.into(),
            });
        }
        let mut ret = [0u8; 32];
        for i in 0..32 {
            ret[i] = rem[i];
        }
        Ok((Self(ret), &rem[v..]))
    }
}

///////////////////////////////////////////// string ////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct string<'a>(&'a str);

impl<'a> FieldType<'a> for string<'a> {
    const WIRE_TYPE: WireType = WireType::LengthDelimited;

    type NativeType = &'a str;

    fn from_native(s: Self::NativeType) -> Self {
        Self(s)
    }
}

impl<'a> FieldHelper<'a, string<'a>> for &'a str {
    fn prototk_pack_sz(tag: &Tag, field: &Self) -> usize {
        let field: &[u8] = field.as_bytes();
        stack_pack(tag).pack(field).pack_sz()
    }

    fn prototk_pack(tag: &Tag, field: &Self, out: &mut [u8]) {
        let field: &[u8] = field.as_bytes();
        stack_pack(tag).pack(field).into_slice(out);
    }

    fn prototk_convert_field<'b>(proto: string<'a>, out: &'b mut Self) where 'a: 'b {
        *out = proto.0;
    }

    fn prototk_convert_variant(proto: string<'a>) -> Self {
        proto.0
    }
}

impl<'a> FieldHelper<'a, string<'a>> for String {
    fn prototk_pack_sz(tag: &Tag, field: &Self) -> usize {
        let field: &[u8] = field.as_bytes();
        stack_pack(tag).pack(field).pack_sz()
    }

    fn prototk_pack(tag: &Tag, field: &Self, out: &mut [u8]) {
        let field: &[u8] = field.as_bytes();
        stack_pack(tag).pack(field).into_slice(out);
    }

    fn prototk_convert_field<'b>(proto: string, out: &'b mut Self) where 'a: 'b {
        *out = proto.0.to_owned();
    }

    fn prototk_convert_variant(proto: string) -> Self {
        proto.0.to_owned()
    }
}

impl<'a> FieldHelper<'a, string<'a>> for PathBuf {
    fn prototk_pack_sz(tag: &Tag, field: &Self) -> usize {
        let field: &[u8] = field.as_os_str().as_bytes();
        stack_pack(tag).pack(field).pack_sz()
    }

    fn prototk_pack(tag: &Tag, field: &Self, out: &mut [u8]) {
        let field: &[u8] = field.as_os_str().as_bytes();
        stack_pack(tag).pack(field).into_slice(out);
    }

    fn prototk_convert_field<'b>(proto: string, out: &'b mut Self) where 'a: 'b {
        *out = proto.0.into();
    }

    fn prototk_convert_variant(proto: string) -> Self {
        proto.0.into()
    }
}

impl<'a> Unpackable<'a> for string<'a> {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let mut up = Unpacker::new(buf);
        let v: v64 = up.unpack()?;
        let v: usize = v.into();
        let rem = up.remain();
        if rem.len() < v {
            return Err(Error::BufferTooShort {
                required: v,
                had: rem.len(),
            });
        }
        let x: &'a [u8] = &rem[..v];
        let s: &'a str = match std::str::from_utf8(x) {
            Ok(s) => s,
            Err(_) => {
                return Err(Error::StringEncoding);
            }
        };
        Ok((string(s), &rem[v..]))
    }
}

////////////////////////////////////////////// message /////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct message<M>(M);

impl<'a, M> FieldType<'a> for message<M>
where
    M: Message<'a>,
    <M as Unpackable<'a>>::Error: From<buffertk::Error>,
{
    const WIRE_TYPE: WireType = WireType::LengthDelimited;

    type NativeType = M;

    fn from_native(msg: M) -> Self {
        Self(msg)
    }
}

impl<'a, M> FieldHelper<'a, message<M>> for M
where
    M: Message<'a>,
    <M as Unpackable<'a>>::Error: From<buffertk::Error>,
{
    fn prototk_pack_sz(tag: &Tag, field: &Self) -> usize {
        stack_pack(tag).pack(stack_pack(field).length_prefixed()).pack_sz()
    }

    fn prototk_pack(tag: &Tag, field: &Self, out: &mut [u8]) {
        stack_pack(tag).pack(stack_pack(field).length_prefixed()).into_slice(out);
    }

    fn prototk_convert_field<'b>(proto: message<M>, out: &'b mut Self) where 'a: 'b {
        *out = proto.0;
    }

    fn prototk_convert_variant(proto: message<M>) -> Self {
        proto.0
    }
}

impl<'a, M> Unpackable<'a> for message<M>
where
    M: Message<'a>,
    <M as Unpackable<'a>>::Error: From<buffertk::Error>,
{
    type Error = M::Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Self::Error> {
        let mut up = Unpacker::new(buf);
        let v: v64 = match up.unpack() {
            Ok(v) => v,
            Err(e) => {
                return Err(e.into());
            }
        };
        let v: usize = v.into();
        let rem = up.remain();
        // TODO(rescrv): this pattern multiple times; try to move to Unpacker.
        if rem.len() < v {
            return Err(buffertk::Error::BufferTooShort {
                required: v,
                had: rem.len(),
            }
            .into());
        }
        let buf: &'b [u8] = &rem[..v];
        let rem: &'b [u8] = &rem[v..];
        let (m, empty): (M, &'a [u8]) = <M as Unpackable<'a>>::unpack(buf)?;
        // TODO(rescrv): assert is nasty
        assert_eq!(0, empty.len());
        Ok((Self(m), rem))
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use crate::field_types::*;

    // expect is the body of the field, including length prefix if necessary.
    fn helper_test<'a, T, H>(value: H, expect: &'a [u8])
    where
        T: Clone + FieldType<'a>,
        H: Debug + Default + Eq + FieldHelper<'a, T>,
    {
        // tag
        let tag = Tag {
            field_number: FieldNumber::must(1),
            wire_type: T::WIRE_TYPE,
        };
        // pack_sz
        assert_eq!(1 + expect.len(), <H as FieldHelper<'a, T>>::prototk_pack_sz(&tag, &value));
        // pack
        let mut output: Vec<u8> = Vec::with_capacity(1 + expect.len());
        output.resize(1 + expect.len(), 0);
        <H as FieldHelper<'a, T>>::prototk_pack(&tag, &value, &mut output);
        assert_eq!(expect, &output[1..]);
        // unpack
        let mut up = Unpacker::new(expect);
        let unpacked: T = match up.unpack() {
            Ok(x) => x,
            Err(_) => { panic!("up.unpack() failed"); }
        };
        let mut field: H = H::default();
        FieldHelper::<T>::prototk_convert_field(unpacked.clone(), &mut field);
        assert_eq!(value, field);
        let variant: H = FieldHelper::<'a, T>::prototk_convert_variant(unpacked);
        assert_eq!(value, variant);
    }

    #[test]
    fn int32() {
        helper_test::<int32, i32>(i32::min_value(), &[0x80, 0x80, 0x80, 0x80, 0xf8, 0xff, 0xff, 0xff, 0xff, 1]);
        helper_test::<int32, i32>(-1, &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 1]);
        helper_test::<int32, i32>(0, &[0]);
        helper_test::<int32, i32>(1, &[1]);
        helper_test::<int32, i32>(i32::max_value(), &[0xff, 0xff, 0xff, 0xff, 0x07]);
    }

    #[test]
    fn int64() {
        helper_test::<int64, i64>(i64::min_value(), &[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 1]);
        helper_test::<int64, i64>(-1, &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 1]);
        helper_test::<int64, i64>(0, &[0]);
        helper_test::<int64, i64>(1, &[1]);
        helper_test::<int64, i64>(i64::max_value(), &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f]);
    }

    #[test]
    fn uint32() {
        helper_test::<uint32, u32>(0, &[0]);
        helper_test::<uint32, u32>(1, &[1]);
        helper_test::<uint32, u32>(u32::max_value(), &[0xff, 0xff, 0xff, 0xff, 0x0f]);
    }

    #[test]
    fn uint64() {
        helper_test::<uint64, u64>(0, &[0]);
        helper_test::<uint64, u64>(1, &[1]);
        helper_test::<uint64, u64>(u64::max_value(), &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 1]);
    }

    #[test]
    fn sint32() {
        helper_test::<sint32, i32>(i32::min_value(), &[0xff, 0xff, 0xff, 0xff, 0x0f]);
        helper_test::<sint32, i32>(-1, &[1]);
        helper_test::<sint32, i32>(0, &[0]);
        helper_test::<sint32, i32>(1, &[2]);
        helper_test::<sint32, i32>(i32::max_value(), &[0xfe, 0xff, 0xff, 0xff, 0x0f]);
    }

    #[test]
    fn sint64() {
        helper_test::<sint64, i64>(i64::min_value(), &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 1]);
        helper_test::<sint64, i64>(-1, &[1]);
        helper_test::<sint64, i64>(0, &[0]);
        helper_test::<sint64, i64>(1, &[2]);
        helper_test::<sint64, i64>(i64::max_value(), &[0xfe, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 1]);
    }

    #[test]
    fn fixed32() {
        helper_test::<fixed32, u32>(0, &[0, 0, 0, 0]);
        helper_test::<fixed32, u32>(1, &[1, 0, 0, 0]);
        helper_test::<fixed32, u32>(u32::max_value(), &[0xff, 0xff, 0xff, 0xff]);
    }

    #[test]
    fn fixed64() {
        helper_test::<fixed64, u64>(0, &[0, 0, 0, 0, 0, 0, 0, 0]);
        helper_test::<fixed64, u64>(1, &[1, 0, 0, 0, 0, 0, 0, 0]);
        helper_test::<fixed64, u64>(u64::max_value(), &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
    }

    #[test]
    fn sfixed32() {
        helper_test::<sfixed32, i32>(i32::min_value(), &[0, 0, 0, 0x80]);
        helper_test::<sfixed32, i32>(-1, &[0xff, 0xff, 0xff, 0xff]);
        helper_test::<sfixed32, i32>(0, &[0, 0, 0, 0]);
        helper_test::<sfixed32, i32>(1, &[1, 0, 0, 0]);
        helper_test::<sfixed32, i32>(i32::max_value(), &[0xff, 0xff, 0xff, 0x7f]);
    }

    #[test]
    fn sfixed64() {
        helper_test::<sfixed64, i64>(i64::min_value(), &[0, 0, 0, 0, 0, 0, 0, 0x80]);
        helper_test::<sfixed64, i64>(-1, &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
        helper_test::<sfixed64, i64>(0, &[0, 0, 0, 0, 0, 0, 0, 0]);
        helper_test::<sfixed64, i64>(1, &[1, 0, 0, 0, 0, 0, 0, 0]);
        helper_test::<sfixed64, i64>(i64::max_value(), &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f]);
    }

    #[test]
    fn float() {
        let value = 3.14159;
        let expect = &[0xd0, 0x0f, 0x49, 0x40];

        // tag
        let tag = Tag {
            field_number: FieldNumber::must(1),
            wire_type: float::WIRE_TYPE,
        };
        // pack_sz
        assert_eq!(1 + expect.len(), FieldHelper::<float>::prototk_pack_sz(&tag, &value));
        // pack
        let mut output: Vec<u8> = Vec::with_capacity(1 + expect.len());
        output.resize(1 + expect.len(), 0);
        FieldHelper::<float>::prototk_pack(&tag, &value, &mut output);
        assert_eq!(expect, &output[1..]);
        // unpack
        let mut up = Unpacker::new(expect);
        let unpacked: float = match up.unpack() {
            Ok(x) => x,
            Err(_) => { panic!("up.unpack() failed"); }
        };
        let mut field: f32 = f32::default();
        FieldHelper::<float>::prototk_convert_field(unpacked.clone(), &mut field);
        assert!(field * 0.9999 < value && field * 1.0001 > value);
        let variant: f32 = FieldHelper::<float>::prototk_convert_variant(unpacked);
        assert!(variant * 0.9999 < value && variant * 1.0001 > value);
    }

    #[test]
    fn double() {
        let value = 3.14159;
        let expect = &[0x6e, 0x86, 0x1b, 0xf0, 0xf9, 0x21, 0x09, 0x40];

        // tag
        let tag = Tag {
            field_number: FieldNumber::must(1),
            wire_type: double::WIRE_TYPE,
        };
        // pack_sz
        assert_eq!(1 + expect.len(), FieldHelper::<double>::prototk_pack_sz(&tag, &value));
        // pack
        let mut output: Vec<u8> = Vec::with_capacity(1 + expect.len());
        output.resize(1 + expect.len(), 0);
        FieldHelper::<double>::prototk_pack(&tag, &value, &mut output);
        assert_eq!(expect, &output[1..]);
        // unpack
        let mut up = Unpacker::new(expect);
        let unpacked: double = match up.unpack() {
            Ok(x) => x,
            Err(_) => { panic!("up.unpack() failed"); }
        };
        let mut field: f64 = f64::default();
        FieldHelper::<double>::prototk_convert_field(unpacked.clone(), &mut field);
        assert!(field * 0.9999 < value && field * 1.0001 > value);
        let variant: f64 = FieldHelper::<double>::prototk_convert_variant(unpacked);
        assert!(variant * 0.9999 < value && variant * 1.0001 > value);
    }

    #[test]
    #[allow(non_snake_case)]
    fn Bool() {
        helper_test::<Bool, bool>(false, &[0]);
        helper_test::<Bool, bool>(true, &[1]);
    }

    #[test]
    fn bytes() {
        helper_test::<bytes, &[u8]>(&[0xff, 0x00], &[0x2, 0xff, 0x00]);
        helper_test::<bytes, Vec<u8>>(vec![0xff, 0x00], &[0x2, 0xff, 0x00]);
    }

    #[test]
    fn buffer() {
        let buf: &[u8] = &[0u8, 1, 2, 3, 4, 5, 6, 7];
        let buf: Buffer = Buffer::from(buf);
        helper_test::<bytes, Buffer>(buf, &[8, 0, 1, 2, 3, 4, 5, 6, 7]);
    }

    #[test]
    fn bytes32() {
        let mut input: [u8; 32] = [0u8; 32];
        let mut expect: Vec<u8> = Vec::new();
        expect.push(32);
        for i in 0..32 {
            input[i] = i as u8;
            expect.push(i as u8);
        }
        helper_test::<bytes32, [u8; 32]>(input, &expect);
    }

    #[test]
    fn string() {
        helper_test::<string, String>("string \u{1F600}".to_owned(), &[0xb, 0x73, 0x74, 0x72, 0x69, 0x6e, 0x67, 0x20, 0xf0, 0x9f, 0x98, 0x80]);
        helper_test::<string, &str>("string \u{1F600}", &[0xb, 0x73, 0x74, 0x72, 0x69, 0x6e, 0x67, 0x20, 0xf0, 0x9f, 0x98, 0x80]);
    }
}
