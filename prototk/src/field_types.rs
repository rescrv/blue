//! Field types supported by prototk.
//!
//! Add `use prototk::field_types::*` to your includes to access all field types in the derive
//! macros.

#![allow(non_camel_case_types)]

// We allow non-CamelCase types here because we want the struct names to appear as close to they do
// in the proto documentation and official implementation.  Thus, `uint64` is how we represent the
// type of `u64`.  The primary use of these field types is to pull the token from a field annotated
// with e.g. #[prototk(7, uint64)], where the uint64 token is used verbatim.

use std::convert::TryInto;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use buffertk::{stack_pack, Unpackable, Unpacker};

use super::*;

/////////////////////////////////////////////// int32 //////////////////////////////////////////////

/// [int32] corresponds to the protobuf type of the same name.  It's a signed 32-bit integer
/// represented as a varint.
#[derive(Clone, Debug, Default)]
pub struct int32(pub i32);

impl FieldType<'_> for int32 {
    const WIRE_TYPE: WireType = WireType::Varint;

    type Native = i32;

    fn from_native(x: Self::Native) -> Self {
        Self(x)
    }

    fn into_native(self) -> Self::Native {
        self.0
    }
}

impl FieldPackHelper<'_, int32> for i32 {
    fn field_pack_sz(&self, tag: &Tag) -> usize {
        stack_pack(tag).pack(int32(*self)).pack_sz()
    }

    fn field_pack(&self, tag: &Tag, out: &mut [u8]) {
        stack_pack(tag).pack(int32(*self)).into_slice(out);
    }
}

impl FieldUnpackHelper<'_, int32> for i32 {
    fn merge_field(&mut self, proto: int32) {
        *self = proto.into();
    }
}

impl From<int32> for i32 {
    fn from(f: int32) -> Self {
        f.0
    }
}

impl Packable for int32 {
    fn pack_sz(&self) -> usize {
        let v: v64 = v64::from(self.0);
        stack_pack(v).pack_sz()
    }

    fn pack(&self, out: &mut [u8]) {
        let v: v64 = v64::from(self.0);
        stack_pack(v).into_slice(out);
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

/// [int64] corresponds to the protobuf type of the same name.  It's a signed 64-bit integer
/// represented as a varint.
#[derive(Clone, Debug, Default)]
pub struct int64(pub i64);

impl FieldType<'_> for int64 {
    const WIRE_TYPE: WireType = WireType::Varint;

    type Native = i64;

    fn from_native(x: Self::Native) -> Self {
        Self(x)
    }

    fn into_native(self) -> Self::Native {
        self.0
    }
}

impl FieldPackHelper<'_, int64> for i64 {
    fn field_pack_sz(&self, tag: &Tag) -> usize {
        stack_pack(tag).pack(int64(*self)).pack_sz()
    }

    fn field_pack(&self, tag: &Tag, out: &mut [u8]) {
        stack_pack(tag).pack(int64(*self)).into_slice(out);
    }
}

impl FieldUnpackHelper<'_, int64> for i64 {
    fn merge_field(&mut self, proto: int64) {
        *self = proto.into();
    }
}

impl From<int64> for i64 {
    fn from(f: int64) -> i64 {
        f.0
    }
}

impl Packable for int64 {
    fn pack_sz(&self) -> usize {
        let v: v64 = v64::from(self.0);
        stack_pack(v).pack_sz()
    }

    fn pack(&self, out: &mut [u8]) {
        let v: v64 = v64::from(self.0);
        stack_pack(v).into_slice(out);
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

/// [uint32] corresponds to the protobuf type of the same name.  It's a signed 32-bit integer
/// represented as a varint.
#[derive(Clone, Debug, Default)]
pub struct uint32(pub u32);

impl FieldType<'_> for uint32 {
    const WIRE_TYPE: WireType = WireType::Varint;

    type Native = u32;

    fn from_native(x: Self::Native) -> Self {
        Self(x)
    }

    fn into_native(self) -> Self::Native {
        self.0
    }
}

impl FieldPackHelper<'_, uint32> for u32 {
    fn field_pack_sz(&self, tag: &Tag) -> usize {
        stack_pack(tag).pack(uint32(*self)).pack_sz()
    }

    fn field_pack(&self, tag: &Tag, out: &mut [u8]) {
        stack_pack(tag).pack(uint32(*self)).into_slice(out);
    }
}

impl FieldUnpackHelper<'_, uint32> for u32 {
    fn merge_field(&mut self, proto: uint32) {
        *self = proto.into();
    }
}

impl From<uint32> for u32 {
    fn from(f: uint32) -> u32 {
        f.0
    }
}

impl Packable for uint32 {
    fn pack_sz(&self) -> usize {
        let v: v64 = v64::from(self.0);
        stack_pack(v).pack_sz()
    }

    fn pack(&self, out: &mut [u8]) {
        let v: v64 = v64::from(self.0);
        stack_pack(v).into_slice(out);
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

/// [uint64] corresponds to the protobuf type of the same name.  It's a signed 64-bit integer
/// represented as a varint.
#[derive(Clone, Debug, Default)]
pub struct uint64(pub u64);

impl FieldType<'_> for uint64 {
    const WIRE_TYPE: WireType = WireType::Varint;

    type Native = u64;

    fn from_native(x: Self::Native) -> Self {
        Self(x)
    }

    fn into_native(self) -> Self::Native {
        self.0
    }
}

impl FieldPackHelper<'_, uint64> for u64 {
    fn field_pack_sz(&self, tag: &Tag) -> usize {
        let v: v64 = v64::from(*self);
        stack_pack(tag).pack(v).pack_sz()
    }

    fn field_pack(&self, tag: &Tag, out: &mut [u8]) {
        let v: v64 = v64::from(*self);
        stack_pack(tag).pack(v).into_slice(out);
    }
}

impl FieldUnpackHelper<'_, uint64> for u64 {
    fn merge_field(&mut self, proto: uint64) {
        *self = proto.into();
    }
}

impl From<uint64> for u64 {
    fn from(f: uint64) -> u64 {
        f.0
    }
}

impl FieldPackHelper<'_, uint64> for usize {
    fn field_pack_sz(&self, tag: &Tag) -> usize {
        stack_pack(tag).pack(uint64(*self as u64)).pack_sz()
    }

    fn field_pack(&self, tag: &Tag, out: &mut [u8]) {
        stack_pack(tag).pack(uint64(*self as u64)).into_slice(out);
    }
}

impl FieldUnpackHelper<'_, uint64> for usize {
    fn merge_field(&mut self, proto: uint64) {
        *self = proto.into();
    }
}

impl From<uint64> for usize {
    fn from(f: uint64) -> usize {
        f.0 as usize
    }
}

impl Packable for uint64 {
    fn pack_sz(&self) -> usize {
        let v: v64 = v64::from(self.0);
        stack_pack(v).pack_sz()
    }

    fn pack(&self, out: &mut [u8]) {
        let v: v64 = v64::from(self.0);
        stack_pack(v).into_slice(out);
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

/// [sint32] corresponds to the protobuf type of the same name.  It's a signed 32-bit
/// zig-zag-integer represented as a varint.
#[derive(Clone, Debug, Default)]
pub struct sint32(pub i32);

impl FieldType<'_> for sint32 {
    const WIRE_TYPE: WireType = WireType::Varint;

    type Native = i32;

    fn from_native(x: Self::Native) -> Self {
        Self(x)
    }

    fn into_native(self) -> Self::Native {
        self.0
    }
}

impl FieldPackHelper<'_, sint32> for i32 {
    fn field_pack_sz(&self, tag: &Tag) -> usize {
        stack_pack(tag).pack(sint32(*self)).pack_sz()
    }

    fn field_pack(&self, tag: &Tag, out: &mut [u8]) {
        stack_pack(tag).pack(sint32(*self)).into_slice(out);
    }
}

impl FieldUnpackHelper<'_, sint32> for i32 {
    fn merge_field(&mut self, proto: sint32) {
        *self = proto.into();
    }
}

impl From<sint32> for i32 {
    fn from(f: sint32) -> i32 {
        f.0
    }
}

impl Packable for sint32 {
    fn pack_sz(&self) -> usize {
        let v: v64 = v64::from(zigzag(self.0 as i64));
        stack_pack(v).pack_sz()
    }

    fn pack(&self, out: &mut [u8]) {
        let v: v64 = v64::from(zigzag(self.0 as i64));
        stack_pack(v).into_slice(out);
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

/// [sint64] corresponds to the protobuf type of the same name.  It's a signed 64-bit
/// zig-zag-integer represented as a varint.
#[derive(Clone, Debug, Default)]
pub struct sint64(pub i64);

impl FieldType<'_> for sint64 {
    const WIRE_TYPE: WireType = WireType::Varint;

    type Native = i64;

    fn from_native(x: Self::Native) -> Self {
        Self(x)
    }

    fn into_native(self) -> Self::Native {
        self.0
    }
}

impl FieldPackHelper<'_, sint64> for i64 {
    fn field_pack_sz(&self, tag: &Tag) -> usize {
        stack_pack(tag).pack(sint64(*self)).pack_sz()
    }

    fn field_pack(&self, tag: &Tag, out: &mut [u8]) {
        stack_pack(tag).pack(sint64(*self)).into_slice(out);
    }
}

impl FieldUnpackHelper<'_, sint64> for i64 {
    fn merge_field(&mut self, proto: sint64) {
        *self = proto.into();
    }
}

impl From<sint64> for i64 {
    fn from(f: sint64) -> i64 {
        f.0
    }
}

impl Packable for sint64 {
    fn pack_sz(&self) -> usize {
        let v: v64 = v64::from(zigzag(self.0));
        stack_pack(v).pack_sz()
    }

    fn pack(&self, out: &mut [u8]) {
        let v: v64 = v64::from(zigzag(self.0));
        stack_pack(v).into_slice(out);
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

/// [fixed32] corresponds to the protobuf type of the same name.  It's an unsigned 32-bit integer
/// represented as a little-endian unsigned integer.
#[derive(Clone, Debug, Default)]
pub struct fixed32(pub u32);

impl FieldType<'_> for fixed32 {
    const WIRE_TYPE: WireType = WireType::ThirtyTwo;

    type Native = u32;

    fn from_native(x: Self::Native) -> Self {
        Self(x)
    }

    fn into_native(self) -> Self::Native {
        self.0
    }
}

impl FieldPackHelper<'_, fixed32> for u32 {
    fn field_pack_sz(&self, tag: &Tag) -> usize {
        stack_pack(tag).pack(fixed32(*self)).pack_sz()
    }

    fn field_pack(&self, tag: &Tag, out: &mut [u8]) {
        stack_pack(tag).pack(fixed32(*self)).into_slice(out);
    }
}

impl FieldUnpackHelper<'_, fixed32> for u32 {
    fn merge_field(&mut self, proto: fixed32) {
        *self = proto.into();
    }
}

impl From<fixed32> for u32 {
    fn from(f: fixed32) -> u32 {
        f.0
    }
}

impl Packable for fixed32 {
    fn pack_sz(&self) -> usize {
        stack_pack(self.0).pack_sz()
    }

    fn pack(&self, out: &mut [u8]) {
        stack_pack(self.0).into_slice(out);
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

/// [fixed64] corresponds to the protobuf type of the same name.  It's an unsigned 64-bit integer
/// represented as a little-endian unsigned integer.
#[derive(Clone, Debug, Default)]
pub struct fixed64(pub u64);

impl FieldType<'_> for fixed64 {
    const WIRE_TYPE: WireType = WireType::SixtyFour;

    type Native = u64;

    fn from_native(x: Self::Native) -> Self {
        Self(x)
    }

    fn into_native(self) -> Self::Native {
        self.0
    }
}

impl FieldPackHelper<'_, fixed64> for u64 {
    fn field_pack_sz(&self, tag: &Tag) -> usize {
        stack_pack(tag).pack(fixed64(*self)).pack_sz()
    }

    fn field_pack(&self, tag: &Tag, out: &mut [u8]) {
        stack_pack(tag).pack(fixed64(*self)).into_slice(out);
    }
}

impl FieldUnpackHelper<'_, fixed64> for u64 {
    fn merge_field(&mut self, proto: fixed64) {
        *self = proto.into();
    }
}

impl From<fixed64> for u64 {
    fn from(f: fixed64) -> u64 {
        f.0
    }
}

impl Packable for fixed64 {
    fn pack_sz(&self) -> usize {
        stack_pack(self.0).pack_sz()
    }

    fn pack(&self, out: &mut [u8]) {
        stack_pack(self.0).into_slice(out);
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

/// [sfixed32] corresponds to the protobuf type of the same name.  It's a signed 32-bit integer
/// represented as a little-endian unsigned integer.
#[derive(Clone, Debug, Default)]
pub struct sfixed32(pub i32);

impl FieldType<'_> for sfixed32 {
    const WIRE_TYPE: WireType = WireType::ThirtyTwo;

    type Native = i32;

    fn from_native(x: Self::Native) -> Self {
        Self(x)
    }

    fn into_native(self) -> Self::Native {
        self.0
    }
}

impl FieldPackHelper<'_, sfixed32> for i32 {
    fn field_pack_sz(&self, tag: &Tag) -> usize {
        stack_pack(tag).pack(sfixed32(*self)).pack_sz()
    }

    fn field_pack(&self, tag: &Tag, out: &mut [u8]) {
        stack_pack(tag).pack(sfixed32(*self)).into_slice(out);
    }
}

impl FieldUnpackHelper<'_, sfixed32> for i32 {
    fn merge_field(&mut self, proto: sfixed32) {
        *self = proto.into();
    }
}

impl From<sfixed32> for i32 {
    fn from(f: sfixed32) -> i32 {
        f.0
    }
}

impl Packable for sfixed32 {
    fn pack_sz(&self) -> usize {
        stack_pack(self.0).pack_sz()
    }

    fn pack(&self, out: &mut [u8]) {
        stack_pack(self.0).into_slice(out);
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

/// [sfixed64] corresponds to the protobuf type of the same name.  It's a signed 64-bit integer
/// represented as a little-endian unsigned integer.
#[derive(Clone, Debug, Default)]
pub struct sfixed64(pub i64);

impl FieldType<'_> for sfixed64 {
    const WIRE_TYPE: WireType = WireType::SixtyFour;

    type Native = i64;

    fn from_native(x: Self::Native) -> Self {
        Self(x)
    }

    fn into_native(self) -> Self::Native {
        self.0
    }
}

impl FieldPackHelper<'_, sfixed64> for i64 {
    fn field_pack_sz(&self, tag: &Tag) -> usize {
        stack_pack(tag).pack(sfixed64(*self)).pack_sz()
    }

    fn field_pack(&self, tag: &Tag, out: &mut [u8]) {
        stack_pack(tag).pack(sfixed64(*self)).into_slice(out);
    }
}

impl FieldUnpackHelper<'_, sfixed64> for i64 {
    fn merge_field(&mut self, proto: sfixed64) {
        *self = proto.into();
    }
}

impl From<sfixed64> for i64 {
    fn from(f: sfixed64) -> i64 {
        f.0
    }
}

impl Packable for sfixed64 {
    fn pack_sz(&self) -> usize {
        stack_pack(self.0).pack_sz()
    }

    fn pack(&self, out: &mut [u8]) {
        stack_pack(self.0).into_slice(out);
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

/// [float] corresponds to the protobuf type of the same name.  It's a 32-bit IEEE 754 float point
/// number in little-endian format.
#[derive(Clone, Debug, Default)]
pub struct float(pub f32);

impl FieldType<'_> for float {
    const WIRE_TYPE: WireType = WireType::SixtyFour;

    type Native = f32;

    fn from_native(x: Self::Native) -> Self {
        Self(x)
    }

    fn into_native(self) -> Self::Native {
        self.0
    }
}

impl FieldPackHelper<'_, float> for f32 {
    fn field_pack_sz(&self, tag: &Tag) -> usize {
        stack_pack(tag).pack(float(*self)).pack_sz()
    }

    fn field_pack(&self, tag: &Tag, out: &mut [u8]) {
        stack_pack(tag).pack(float(*self)).into_slice(out);
    }
}

impl FieldUnpackHelper<'_, float> for f32 {
    fn merge_field(&mut self, proto: float) {
        *self = proto.into();
    }
}

impl From<float> for f32 {
    fn from(f: float) -> f32 {
        f.0
    }
}

impl Packable for float {
    fn pack_sz(&self) -> usize {
        stack_pack(self.0).pack_sz()
    }

    fn pack(&self, out: &mut [u8]) {
        stack_pack(self.0).into_slice(out);
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

/// [double] corresponds to the protobuf type of the same name.  It's a 64-bit IEEE 754 float point
/// number in little-endian format.
#[derive(Clone, Debug, Default)]
pub struct double(pub f64);

impl FieldType<'_> for double {
    const WIRE_TYPE: WireType = WireType::SixtyFour;

    type Native = f64;

    fn from_native(x: Self::Native) -> Self {
        Self(x)
    }

    fn into_native(self) -> Self::Native {
        self.0
    }
}

impl FieldPackHelper<'_, double> for f64 {
    fn field_pack_sz(&self, tag: &Tag) -> usize {
        stack_pack(tag).pack(double(*self)).pack_sz()
    }

    fn field_pack(&self, tag: &Tag, out: &mut [u8]) {
        stack_pack(tag).pack(double(*self)).into_slice(out);
    }
}

impl FieldUnpackHelper<'_, double> for f64 {
    fn merge_field(&mut self, proto: double) {
        *self = proto.into();
    }
}

impl From<double> for f64 {
    fn from(f: double) -> f64 {
        f.0
    }
}

impl Packable for double {
    fn pack_sz(&self) -> usize {
        stack_pack(self.0).pack_sz()
    }

    fn pack(&self, out: &mut [u8]) {
        stack_pack(self.0).into_slice(out);
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

/// [Bool] corresponds to the protobuf type of the same name.
#[derive(Clone, Debug, Default)]
pub struct Bool(pub bool);

impl FieldType<'_> for Bool {
    const WIRE_TYPE: WireType = WireType::Varint;

    type Native = bool;

    fn from_native(b: Self::Native) -> Self {
        Self(b)
    }

    fn into_native(self) -> Self::Native {
        self.0
    }
}

impl FieldPackHelper<'_, Bool> for bool {
    fn field_pack_sz(&self, tag: &Tag) -> usize {
        stack_pack(tag).pack(Bool(*self)).pack_sz()
    }

    fn field_pack(&self, tag: &Tag, out: &mut [u8]) {
        stack_pack(tag).pack(Bool(*self)).into_slice(out);
    }
}

impl FieldUnpackHelper<'_, Bool> for bool {
    fn merge_field(&mut self, proto: Bool) {
        *self = proto.into();
    }
}

impl From<Bool> for bool {
    fn from(f: Bool) -> bool {
        f.0
    }
}

impl Packable for Bool {
    fn pack_sz(&self) -> usize {
        let v: v64 = v64::from(if self.0 { 1 } else { 0 });
        stack_pack(v).pack_sz()
    }

    fn pack(&self, out: &mut [u8]) {
        let v: v64 = v64::from(if self.0 { 1 } else { 0 });
        stack_pack(v).into_slice(out);
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

/// [bytes] represents a variable-number of bytes.
#[derive(Clone, Debug, Default)]
pub struct bytes<'a>(pub &'a [u8]);

impl<'a> FieldType<'a> for bytes<'a> {
    const WIRE_TYPE: WireType = WireType::LengthDelimited;

    type Native = &'a [u8];

    fn from_native(x: Self::Native) -> Self {
        Self(x)
    }

    fn into_native(self) -> Self::Native {
        self.0
    }
}

impl<'a> FieldPackHelper<'a, bytes<'a>> for &'a [u8] {
    fn field_pack_sz(&self, tag: &Tag) -> usize {
        stack_pack(tag).pack(bytes(self)).pack_sz()
    }

    fn field_pack(&self, tag: &Tag, out: &mut [u8]) {
        stack_pack(tag).pack(bytes(self)).into_slice(out);
    }
}

impl<'a> FieldUnpackHelper<'a, bytes<'a>> for &'a [u8] {
    fn merge_field(&mut self, proto: bytes<'a>) {
        *self = proto.into();
    }
}

impl<'a> From<bytes<'a>> for &'a [u8] {
    fn from(f: bytes<'a>) -> &'a [u8] {
        f.0
    }
}

impl<'a> FieldPackHelper<'a, bytes<'a>> for Vec<u8> {
    fn field_pack_sz(&self, tag: &Tag) -> usize {
        let field: &[u8] = self;
        stack_pack(tag).pack(bytes(field)).pack_sz()
    }

    fn field_pack(&self, tag: &Tag, out: &mut [u8]) {
        let field: &[u8] = self;
        stack_pack(tag).pack(bytes(field)).into_slice(out);
    }
}

impl<'a> FieldUnpackHelper<'a, bytes<'a>> for Vec<u8> {
    fn merge_field(&mut self, proto: bytes<'a>) {
        *self = proto.into();
    }
}

impl<'a> From<bytes<'a>> for Vec<u8> {
    fn from(f: bytes<'a>) -> Vec<u8> {
        f.0.to_vec()
    }
}

impl<'a> FieldPackHelper<'a, bytes<'a>> for PathBuf {
    fn field_pack_sz(&self, tag: &Tag) -> usize {
        let field: &[u8] = self.as_os_str().as_bytes();
        stack_pack(tag).pack(bytes(field)).pack_sz()
    }

    fn field_pack(&self, tag: &Tag, out: &mut [u8]) {
        let field: &[u8] = self.as_os_str().as_bytes();
        stack_pack(tag).pack(bytes(field)).into_slice(out);
    }
}

impl<'a> FieldUnpackHelper<'a, bytes<'a>> for PathBuf {
    fn merge_field(&mut self, proto: bytes<'a>) {
        *self = proto.into();
    }
}

impl<'a> From<bytes<'a>> for PathBuf {
    fn from(f: bytes<'a>) -> PathBuf {
        PathBuf::from(OsStr::from_bytes(f.0))
    }
}

impl Packable for bytes<'_> {
    fn pack_sz(&self) -> usize {
        stack_pack(self.0).pack_sz()
    }

    fn pack(&self, out: &mut [u8]) {
        stack_pack(self.0).into_slice(out);
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

////////////////////////////////////////////// bytes16 /////////////////////////////////////////////

/// [bytes] represents 16 bytes.
#[derive(Clone, Debug, Default)]
pub struct bytes16(pub [u8; 16]);

impl FieldType<'_> for bytes16 {
    const WIRE_TYPE: WireType = WireType::LengthDelimited;

    type Native = [u8; 16];

    fn from_native(x: Self::Native) -> Self {
        Self(x)
    }

    fn into_native(self) -> Self::Native {
        self.0
    }
}

impl FieldPackHelper<'_, bytes16> for [u8; 16] {
    fn field_pack_sz(&self, tag: &Tag) -> usize {
        stack_pack(tag).pack(bytes16(*self)).pack_sz()
    }

    fn field_pack(&self, tag: &Tag, out: &mut [u8]) {
        stack_pack(tag).pack(bytes16(*self)).into_slice(out);
    }
}

impl FieldUnpackHelper<'_, bytes16> for [u8; 16] {
    fn merge_field(&mut self, proto: bytes16) {
        *self = proto.into();
    }
}

impl From<bytes16> for [u8; 16] {
    fn from(f: bytes16) -> [u8; 16] {
        f.0
    }
}

impl Packable for bytes16 {
    fn pack_sz(&self) -> usize {
        let b: &[u8] = &self.0;
        stack_pack(b).pack_sz()
    }

    fn pack(&self, out: &mut [u8]) {
        let b: &[u8] = &self.0;
        stack_pack(b).into_slice(out);
    }
}

impl<'a> Unpackable<'a> for bytes16 {
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
        if v < 16 {
            return Err(Error::BufferTooShort {
                required: 16,
                had: v,
            });
        }
        if v != 16 {
            return Err(Error::WrongLength {
                required: 16,
                had: v,
            });
        }
        let mut ret = [0u8; 16];
        ret[..16].copy_from_slice(&rem[..16]);
        Ok((Self(ret), &rem[v..]))
    }
}
////////////////////////////////////////////// bytes32 /////////////////////////////////////////////

/// [bytes] represents 32 bytes.
#[derive(Clone, Debug, Default)]
pub struct bytes32(pub [u8; 32]);

impl FieldType<'_> for bytes32 {
    const WIRE_TYPE: WireType = WireType::LengthDelimited;

    type Native = [u8; 32];

    fn from_native(x: Self::Native) -> Self {
        Self(x)
    }

    fn into_native(self) -> Self::Native {
        self.0
    }
}

impl FieldPackHelper<'_, bytes32> for [u8; 32] {
    fn field_pack_sz(&self, tag: &Tag) -> usize {
        stack_pack(tag).pack(bytes32(*self)).pack_sz()
    }

    fn field_pack(&self, tag: &Tag, out: &mut [u8]) {
        stack_pack(tag).pack(bytes32(*self)).into_slice(out);
    }
}

impl FieldUnpackHelper<'_, bytes32> for [u8; 32] {
    fn merge_field(&mut self, proto: bytes32) {
        *self = proto.into();
    }
}

impl From<bytes32> for [u8; 32] {
    fn from(f: bytes32) -> [u8; 32] {
        f.0
    }
}

impl Packable for bytes32 {
    fn pack_sz(&self) -> usize {
        let b: &[u8] = &self.0;
        stack_pack(b).pack_sz()
    }

    fn pack(&self, out: &mut [u8]) {
        let b: &[u8] = &self.0;
        stack_pack(b).into_slice(out);
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
                had: v,
            });
        }
        if v != 32 {
            return Err(Error::WrongLength {
                required: 32,
                had: v,
            });
        }
        let mut ret = [0u8; 32];
        ret[..32].copy_from_slice(&rem[..32]);
        Ok((Self(ret), &rem[v..]))
    }
}

////////////////////////////////////////////// bytes64 /////////////////////////////////////////////

/// [bytes] represents 64 bytes.
#[derive(Clone, Debug)]
pub struct bytes64(pub [u8; 64]);

impl FieldType<'_> for bytes64 {
    const WIRE_TYPE: WireType = WireType::LengthDelimited;

    type Native = [u8; 64];

    fn from_native(x: Self::Native) -> Self {
        Self(x)
    }

    fn into_native(self) -> Self::Native {
        self.0
    }
}

impl Default for bytes64 {
    fn default() -> Self {
        Self([0u8; 64])
    }
}

impl FieldPackHelper<'_, bytes64> for [u8; 64] {
    fn field_pack_sz(&self, tag: &Tag) -> usize {
        stack_pack(tag).pack(bytes64(*self)).pack_sz()
    }

    fn field_pack(&self, tag: &Tag, out: &mut [u8]) {
        stack_pack(tag).pack(bytes64(*self)).into_slice(out);
    }
}

impl FieldUnpackHelper<'_, bytes64> for [u8; 64] {
    fn merge_field(&mut self, proto: bytes64) {
        *self = proto.into();
    }
}

impl From<bytes64> for [u8; 64] {
    fn from(f: bytes64) -> [u8; 64] {
        f.0
    }
}

impl Packable for bytes64 {
    fn pack_sz(&self) -> usize {
        let b: &[u8] = &self.0;
        stack_pack(b).pack_sz()
    }

    fn pack(&self, out: &mut [u8]) {
        let b: &[u8] = &self.0;
        stack_pack(b).into_slice(out);
    }
}

impl<'a> Unpackable<'a> for bytes64 {
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
        if v < 64 {
            return Err(Error::BufferTooShort {
                required: 64,
                had: v,
            });
        }
        if v != 64 {
            return Err(Error::WrongLength {
                required: 64,
                had: v,
            });
        }
        let mut ret = [0u8; 64];
        ret[..64].copy_from_slice(&rem[..64]);
        Ok((Self(ret), &rem[v..]))
    }
}

///////////////////////////////////////////// string ////////////////////////////////////////////

/// [string] represents a UTF-8 string of variable length.
#[derive(Clone, Debug, Default)]
pub struct string<'a>(pub &'a str);

impl<'a> FieldType<'a> for string<'a> {
    const WIRE_TYPE: WireType = WireType::LengthDelimited;

    type Native = &'a str;

    fn from_native(s: Self::Native) -> Self {
        Self(s)
    }

    fn into_native(self) -> Self::Native {
        self.0
    }
}

impl<'a> FieldPackHelper<'a, string<'a>> for &'a str {
    fn field_pack_sz(&self, tag: &Tag) -> usize {
        stack_pack(tag).pack(string(self)).pack_sz()
    }

    fn field_pack(&self, tag: &Tag, out: &mut [u8]) {
        stack_pack(tag).pack(string(self)).into_slice(out);
    }
}

impl<'a> FieldUnpackHelper<'a, string<'a>> for &'a str {
    fn merge_field(&mut self, proto: string<'a>) {
        *self = proto.into();
    }
}

impl<'a> From<string<'a>> for &'a str {
    fn from(f: string<'a>) -> &'a str {
        f.0
    }
}

impl<'a> FieldPackHelper<'a, string<'a>> for String {
    fn field_pack_sz(&self, tag: &Tag) -> usize {
        stack_pack(tag).pack(string(self)).pack_sz()
    }

    fn field_pack(&self, tag: &Tag, out: &mut [u8]) {
        stack_pack(tag).pack(string(self)).into_slice(out);
    }
}

impl<'a> FieldUnpackHelper<'a, string<'a>> for String {
    fn merge_field(&mut self, proto: string<'a>) {
        *self = proto.into();
    }
}

impl<'a> From<string<'a>> for String {
    fn from(f: string<'a>) -> String {
        f.0.to_owned()
    }
}

impl<'a> FieldPackHelper<'a, string<'a>> for PathBuf {
    fn field_pack_sz(&self, tag: &Tag) -> usize {
        stack_pack(tag).pack(self.as_os_str().as_bytes()).pack_sz()
    }

    fn field_pack(&self, tag: &Tag, out: &mut [u8]) {
        stack_pack(tag)
            .pack(self.as_os_str().as_bytes())
            .into_slice(out);
    }
}

impl<'a> FieldUnpackHelper<'a, string<'a>> for PathBuf {
    fn merge_field(&mut self, proto: string<'a>) {
        *self = proto.into();
    }
}

impl<'a> From<string<'a>> for PathBuf {
    fn from(f: string<'a>) -> PathBuf {
        f.0.into()
    }
}

impl Packable for string<'_> {
    fn pack_sz(&self) -> usize {
        let b: &[u8] = self.0.as_bytes();
        stack_pack(b).pack_sz()
    }

    fn pack(&self, out: &mut [u8]) {
        let b: &[u8] = self.0.as_bytes();
        stack_pack(b).into_slice(out);
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

/// [message] represents a ProtoTK message.
#[derive(Clone, Debug)]
pub struct message<M>(pub M);

impl<M> message<M> {
    /// Return the message that's held by this [message].
    pub fn unwrap_message(self) -> M {
        self.0
    }
}

impl<M> FieldType<'_> for message<M> {
    const WIRE_TYPE: WireType = WireType::LengthDelimited;

    type Native = M;

    fn from_native(msg: M) -> Self {
        Self(msg)
    }

    fn into_native(self) -> Self::Native {
        self.0
    }
}

impl<M: Packable> Packable for message<M> {
    fn pack_sz(&self) -> usize {
        self.0.pack_sz()
    }

    fn pack(&self, buf: &mut [u8]) {
        self.0.pack(buf)
    }
}

impl<'a, M> Unpackable<'a> for message<M>
where
    M: Unpackable<'a>,
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

impl<M: Default> Default for message<M> {
    fn default() -> Self {
        message::from_native(M::default())
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
        T: Clone + FieldType<'a> + Unpackable<'a>,
        H: Debug + Default + Eq + From<T> + FieldPackHelper<'a, T> + FieldUnpackHelper<'a, T>,
    {
        // tag
        let tag = Tag {
            field_number: FieldNumber::must(1),
            wire_type: T::WIRE_TYPE,
        };
        // pack_sz
        assert_eq!(1 + expect.len(), value.field_pack_sz(&tag));
        // pack
        let mut output: Vec<u8> = vec![0; 1 + expect.len()];
        value.field_pack(&tag, &mut output);
        assert_eq!(expect, &output[1..]);
        // unpack
        let mut up = Unpacker::new(expect);
        let unpacked: T = match up.unpack() {
            Ok(x) => x,
            Err(_) => {
                panic!("up.unpack() failed");
            }
        };
        let mut field = H::default();
        field.merge_field(unpacked.clone());
        assert_eq!(value, field);
    }

    #[test]
    fn int32() {
        helper_test::<int32, i32>(
            i32::MIN,
            &[0x80, 0x80, 0x80, 0x80, 0xf8, 0xff, 0xff, 0xff, 0xff, 1],
        );
        helper_test::<int32, i32>(
            -1,
            &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 1],
        );
        helper_test::<int32, i32>(0, &[0]);
        helper_test::<int32, i32>(1, &[1]);
        helper_test::<int32, i32>(i32::MAX, &[0xff, 0xff, 0xff, 0xff, 0x07]);
    }

    #[test]
    fn int64() {
        helper_test::<int64, i64>(
            i64::MIN,
            &[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 1],
        );
        helper_test::<int64, i64>(
            -1,
            &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 1],
        );
        helper_test::<int64, i64>(0, &[0]);
        helper_test::<int64, i64>(1, &[1]);
        helper_test::<int64, i64>(
            i64::MAX,
            &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f],
        );
    }

    #[test]
    fn uint32() {
        helper_test::<uint32, u32>(0, &[0]);
        helper_test::<uint32, u32>(1, &[1]);
        helper_test::<uint32, u32>(u32::MAX, &[0xff, 0xff, 0xff, 0xff, 0x0f]);
    }

    #[test]
    fn uint64() {
        helper_test::<uint64, u64>(0, &[0]);
        helper_test::<uint64, u64>(1, &[1]);
        helper_test::<uint64, u64>(
            u64::MAX,
            &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 1],
        );
    }

    #[test]
    fn sint32() {
        helper_test::<sint32, i32>(i32::MIN, &[0xff, 0xff, 0xff, 0xff, 0x0f]);
        helper_test::<sint32, i32>(-1, &[1]);
        helper_test::<sint32, i32>(0, &[0]);
        helper_test::<sint32, i32>(1, &[2]);
        helper_test::<sint32, i32>(i32::MAX, &[0xfe, 0xff, 0xff, 0xff, 0x0f]);
    }

    #[test]
    fn sint64() {
        helper_test::<sint64, i64>(
            i64::MIN,
            &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 1],
        );
        helper_test::<sint64, i64>(-1, &[1]);
        helper_test::<sint64, i64>(0, &[0]);
        helper_test::<sint64, i64>(1, &[2]);
        helper_test::<sint64, i64>(
            i64::MAX,
            &[0xfe, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 1],
        );
    }

    #[test]
    fn fixed32() {
        helper_test::<fixed32, u32>(0, &[0, 0, 0, 0]);
        helper_test::<fixed32, u32>(1, &[1, 0, 0, 0]);
        helper_test::<fixed32, u32>(u32::MAX, &[0xff, 0xff, 0xff, 0xff]);
    }

    #[test]
    fn fixed64() {
        helper_test::<fixed64, u64>(0, &[0, 0, 0, 0, 0, 0, 0, 0]);
        helper_test::<fixed64, u64>(1, &[1, 0, 0, 0, 0, 0, 0, 0]);
        helper_test::<fixed64, u64>(u64::MAX, &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
    }

    #[test]
    fn sfixed32() {
        helper_test::<sfixed32, i32>(i32::MIN, &[0, 0, 0, 0x80]);
        helper_test::<sfixed32, i32>(-1, &[0xff, 0xff, 0xff, 0xff]);
        helper_test::<sfixed32, i32>(0, &[0, 0, 0, 0]);
        helper_test::<sfixed32, i32>(1, &[1, 0, 0, 0]);
        helper_test::<sfixed32, i32>(i32::MAX, &[0xff, 0xff, 0xff, 0x7f]);
    }

    #[test]
    fn sfixed64() {
        helper_test::<sfixed64, i64>(i64::MIN, &[0, 0, 0, 0, 0, 0, 0, 0x80]);
        helper_test::<sfixed64, i64>(-1, &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
        helper_test::<sfixed64, i64>(0, &[0, 0, 0, 0, 0, 0, 0, 0]);
        helper_test::<sfixed64, i64>(1, &[1, 0, 0, 0, 0, 0, 0, 0]);
        helper_test::<sfixed64, i64>(i64::MAX, &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f]);
    }

    #[test]
    fn float() {
        let value = std::f32::consts::PI;
        let expect = &[0xdb, 0xf, 0x49, 0x40];

        // tag
        let tag = Tag {
            field_number: FieldNumber::must(1),
            wire_type: float::WIRE_TYPE,
        };
        // pack_sz
        assert_eq!(1 + expect.len(), value.field_pack_sz(&tag));
        // pack
        let mut output: Vec<u8> = vec![0; 1 + expect.len()];
        value.field_pack(&tag, &mut output);
        assert_eq!(expect, &output[1..]);
        // unpack
        let mut up = Unpacker::new(expect);
        let unpacked: float = match up.unpack() {
            Ok(x) => x,
            Err(_) => {
                panic!("up.unpack() failed");
            }
        };
        let mut field: f32 = f32::default();
        field.merge_field(unpacked.clone());
        assert!(field * 0.9999 < value && field * 1.0001 > value);
    }

    #[test]
    fn double() {
        let value = std::f64::consts::PI;
        let expect = &[0x18, 0x2d, 0x44, 0x54, 0xfb, 0x21, 0x9, 0x40];

        // tag
        let tag = Tag {
            field_number: FieldNumber::must(1),
            wire_type: double::WIRE_TYPE,
        };
        // pack_sz
        assert_eq!(1 + expect.len(), value.field_pack_sz(&tag));
        // pack
        let mut output: Vec<u8> = vec![0; 1 + expect.len()];
        value.field_pack(&tag, &mut output);
        assert_eq!(expect, &output[1..]);
        // unpack
        let mut up = Unpacker::new(expect);
        let unpacked: double = match up.unpack() {
            Ok(x) => x,
            Err(_) => {
                panic!("up.unpack() failed");
            }
        };
        let mut field: f64 = f64::default();
        field.merge_field(unpacked.clone());
        assert!(field * 0.9999 < value && field * 1.0001 > value);
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
    fn bytes16() {
        let mut input: [u8; 16] = [0u8; 16];
        let mut expect: Vec<u8> = Vec::new();
        expect.push(16);
        #[allow(clippy::needless_range_loop)]
        for i in 0..16 {
            input[i] = i as u8;
            expect.push(i as u8);
        }
        helper_test::<bytes16, [u8; 16]>(input, &expect);
    }

    #[test]
    fn bytes32() {
        let mut input: [u8; 32] = [0u8; 32];
        let mut expect: Vec<u8> = Vec::new();
        expect.push(32);
        #[allow(clippy::needless_range_loop)]
        for i in 0..32 {
            input[i] = i as u8;
            expect.push(i as u8);
        }
        helper_test::<bytes32, [u8; 32]>(input, &expect);
    }

    #[test]
    fn string() {
        helper_test::<string, String>(
            "string \u{1F600}".to_owned(),
            &[
                0xb, 0x73, 0x74, 0x72, 0x69, 0x6e, 0x67, 0x20, 0xf0, 0x9f, 0x98, 0x80,
            ],
        );
        helper_test::<string, &str>(
            "string \u{1F600}",
            &[
                0xb, 0x73, 0x74, 0x72, 0x69, 0x6e, 0x67, 0x20, 0xf0, 0x9f, 0x98, 0x80,
            ],
        );
    }
}
