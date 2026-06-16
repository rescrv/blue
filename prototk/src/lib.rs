#![doc = include_str!("../README.md")]

use std::fmt::{Debug, Display, Formatter};

pub mod field_types;
mod zigzag;

pub use zigzag::unzigzag;
pub use zigzag::zigzag;

use buffertk::{Packable, Unpackable, Unpacker, stack_pack, v64};
pub use handled::SError;

/////////////////////////////////////////////// Error //////////////////////////////////////////////

const PHASE: &str = "prototk";

/// A default `prototk` error value.
pub const CODE_SUCCESS: &str = "success";
/// A buffer did not contain enough bytes to unpack a value.
pub const CODE_BUFFER_TOO_SHORT: &str = "buffer-too-short";
/// A field number is not user-assignable.
pub const CODE_INVALID_FIELD_NUMBER: &str = "invalid-field-number";
/// A wire type is not understood by this process.
pub const CODE_UNHANDLED_WIRE_TYPE: &str = "unhandled-wire-type";
/// A tag exceeded the 32-bit protobuf tag representation.
pub const CODE_TAG_TOO_LARGE: &str = "tag-too-large";
/// A varint exceeded the maximum encoded length.
pub const CODE_VARINT_OVERFLOW: &str = "varint-overflow";
/// An unsigned value did not fit the requested target type.
pub const CODE_UNSIGNED_OVERFLOW: &str = "unsigned-overflow";
/// A signed value did not fit the requested target type.
pub const CODE_SIGNED_OVERFLOW: &str = "signed-overflow";
/// A fixed-size bytes field had the wrong length.
pub const CODE_WRONG_LENGTH: &str = "wrong-length";
/// A serialized string was not valid UTF-8 or not a valid S-expression.
pub const CODE_STRING_ENCODING: &str = "string-encoding";
/// A discriminant was not recognized.
pub const CODE_UNKNOWN_DISCRIMINANT: &str = "unknown-discriminant";
/// A numeric value is not a valid Unicode scalar value.
pub const CODE_NOT_A_CHAR: &str = "not-a-char";

fn error(code: &str) -> SError {
    SError::new(PHASE).with_code(code)
}

/// Construct a default `prototk` error value.
pub fn success() -> SError {
    error(CODE_SUCCESS).with_message("success")
}

/// Construct a buffer-too-short error.
pub fn buffer_too_short(required: usize, had: usize) -> SError {
    error(CODE_BUFFER_TOO_SHORT)
        .with_message("buffer too short")
        .with_atom_field("required", required)
        .with_atom_field("had", had)
}

/// Construct an invalid-field-number error.
pub fn invalid_field_number(field_number: u32, what: impl AsRef<str>) -> SError {
    error(CODE_INVALID_FIELD_NUMBER)
        .with_message("invalid field number")
        .with_atom_field("field_number", field_number)
        .with_string_field("what", what.as_ref())
}

/// Construct an unhandled-wire-type error.
pub fn unhandled_wire_type(wire_type: u32) -> SError {
    error(CODE_UNHANDLED_WIRE_TYPE)
        .with_message("unhandled wire type")
        .with_atom_field("wire_type", wire_type)
}

/// Construct a tag-too-large error.
pub fn tag_too_large(tag: u64) -> SError {
    error(CODE_TAG_TOO_LARGE)
        .with_message("tag too large")
        .with_atom_field("tag", tag)
}

/// Construct a varint-overflow error.
pub fn varint_overflow(bytes: usize) -> SError {
    error(CODE_VARINT_OVERFLOW)
        .with_message("varint overflow")
        .with_atom_field("bytes", bytes)
}

/// Construct an unsigned-overflow error.
pub fn unsigned_overflow(value: u64) -> SError {
    error(CODE_UNSIGNED_OVERFLOW)
        .with_message("unsigned integer overflow")
        .with_atom_field("value", value)
}

/// Construct a signed-overflow error.
pub fn signed_overflow(value: i64) -> SError {
    error(CODE_SIGNED_OVERFLOW)
        .with_message("signed integer overflow")
        .with_atom_field("value", value)
}

/// Construct a wrong-length error.
pub fn wrong_length(required: usize, had: usize) -> SError {
    error(CODE_WRONG_LENGTH)
        .with_message("wrong length")
        .with_atom_field("required", required)
        .with_atom_field("had", had)
}

/// Construct a string-encoding error.
pub fn string_encoding() -> SError {
    error(CODE_STRING_ENCODING).with_message("string encoding error")
}

/// Construct an unknown-discriminant error.
pub fn unknown_discriminant(discriminant: u32) -> SError {
    error(CODE_UNKNOWN_DISCRIMINANT)
        .with_message("unknown discriminant")
        .with_atom_field("discriminant", discriminant)
}

/// Construct a not-a-char error.
pub fn not_a_char(value: u32) -> SError {
    error(CODE_NOT_A_CHAR)
        .with_message("not a valid char")
        .with_atom_field("value", value)
}

fn error_field<'a>(err: &'a SError, name: &str) -> Option<&'a handled::SExpr> {
    match err.detail() {
        handled::SExpr::List(fields) => fields.iter().find_map(|field| match field {
            handled::SExpr::List(pair) if pair.len() == 2 => match &pair[0] {
                handled::SExpr::Atom(field_name) if field_name == name => Some(&pair[1]),
                _ => None,
            },
            _ => None,
        }),
        _ => None,
    }
}

/// Return the machine-readable code from a `prototk` error.
pub fn error_code(err: &SError) -> Option<&str> {
    match error_field(err, "code") {
        Some(handled::SExpr::Atom(code)) => Some(code.as_str()),
        _ => None,
    }
}

///////////////////////////////////////////// WireType /////////////////////////////////////////////

/// WireType represents the different protocol buffers wire types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireType {
    /// Varint is wire type 0.  The payload is a single v64.
    Varint,
    /// SixtyFour represents wire type 1.  The payload is a single u64.
    SixtyFour,
    /// LengthDelimited represents wire type 2.  The payload depends upon how the system interprets
    /// the field number.
    LengthDelimited,
    // wiretype 3 and 4 were deprecated and therefore not implemented
    /// ThirtyTwo represents wire type 5.  The payload is a single u32.
    ThirtyTwo,
}

impl WireType {
    /// Possibly create a new WireType from the tag bits.
    pub fn new(tag_bits: u32) -> Result<WireType, SError> {
        match tag_bits {
            0 => Ok(WireType::Varint),
            1 => Ok(WireType::SixtyFour),
            2 => Ok(WireType::LengthDelimited),
            5 => Ok(WireType::ThirtyTwo),
            _ => Err(unhandled_wire_type(tag_bits)),
        }
    }

    /// `tag_bits` returns the WireType's contribution to the tag, suitable for bit-wise or'ing with
    /// the FieldNumber.
    pub fn tag_bits(&self) -> u32 {
        match self {
            WireType::Varint => 0,
            WireType::SixtyFour => 1,
            WireType::LengthDelimited => 2,
            WireType::ThirtyTwo => 5,
        }
    }
}

//////////////////////////////////////////// FieldNumber ///////////////////////////////////////////

/// The first valid field number.
pub const FIRST_FIELD_NUMBER: u32 = 1;
/// The last valid field number.
pub const LAST_FIELD_NUMBER: u32 = (1 << 29) - 1;

/// The first field number reserved by protocol buffers.
pub const FIRST_RESERVED_FIELD_NUMBER: u32 = 19000;
/// The last field number reserved by protocol buffers.
pub const LAST_RESERVED_FIELD_NUMBER: u32 = 19999;

/// [FieldNumber] wraps a u32 and guards it against reserved or invalid field numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub struct FieldNumber {
    field_number: u32,
}

impl FieldNumber {
    /// Returns true if and only if `field_number` is not valid and not reserved.
    pub fn is_valid(field_number: u32) -> bool {
        FieldNumber::new(field_number).is_ok()
    }

    /// Returns a valid [FieldNumber], panicking if `field_number` is invalid or reserved.
    pub fn must(field_number: u32) -> FieldNumber {
        FieldNumber::new(field_number).unwrap()
    }

    /// Create a new [FieldNumber] if `field_number` is valid and not reserved.
    pub fn new(field_number: u32) -> Result<FieldNumber, SError> {
        if field_number < FIRST_FIELD_NUMBER {
            return Err(invalid_field_number(
                field_number,
                "field number must be positive integer",
            ));
        }
        if field_number > LAST_FIELD_NUMBER {
            return Err(invalid_field_number(field_number, "field number too large"));
        }
        if (FIRST_RESERVED_FIELD_NUMBER..=LAST_RESERVED_FIELD_NUMBER).contains(&field_number) {
            return Err(invalid_field_number(field_number, "field is reserved"));
        }
        Ok(FieldNumber { field_number })
    }

    /// Return the field number as a u32.
    pub fn get(&self) -> u32 {
        self.field_number
    }
}

impl Display for FieldNumber {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), std::fmt::Error> {
        write!(fmt, "{}", self.field_number)
    }
}

impl From<FieldNumber> for u32 {
    fn from(f: FieldNumber) -> u32 {
        f.field_number
    }
}

impl std::cmp::PartialEq<u32> for FieldNumber {
    fn eq(&self, other: &u32) -> bool {
        self.field_number == *other
    }
}

//////////////////////////////////////////////// Tag ///////////////////////////////////////////////

/// A protobuf tag has two parts:  A `field_number` and a `wire_type`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Tag {
    /// The field number of this tag.
    pub field_number: FieldNumber,
    /// The wire type of this tag.
    pub wire_type: WireType,
}

/// A helper macro to construct tags
///
/// # Panics
///
/// Panics if the field number is invalid.
#[macro_export]
macro_rules! tag {
    ($field_number:literal, $wire_type:ident) => {
        $crate::Tag {
            field_number: $crate::FieldNumber::must($field_number),
            wire_type: $crate::WireType::$wire_type,
        }
    };
}

impl Tag {
    fn v64(&self) -> v64 {
        let f: u32 = self.field_number.into();
        let w: u32 = self.wire_type.tag_bits();
        let t: u32 = (f << 3) | w;
        t.into()
    }
}

impl Packable for Tag {
    fn pack_sz(&self) -> usize {
        let v = self.v64();
        v.pack_sz()
    }

    fn pack(&self, buf: &mut [u8]) {
        let v = self.v64();
        v.pack(buf);
    }
}

impl<'a> Unpackable<'a> for Tag {
    type Error = crate::SError;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), SError> {
        let mut up = Unpacker::new(buf);
        let tag: v64 = up.unpack()?;
        let tag: u64 = tag.into();
        if tag > u32::MAX as u64 {
            return Err(tag_too_large(tag));
        }
        let tag: u32 = tag as u32;
        let f: u32 = tag >> 3;
        let w: u32 = tag & 7;
        let field_number = FieldNumber::new(f)?;
        let wire_type = WireType::new(w)?;
        Ok((
            Tag {
                field_number,
                wire_type,
            },
            up.remain(),
        ))
    }
}

///////////////////////////////////////////// FieldType ////////////////////////////////////////////

/// A field type is a rust-native type used to convert to and from the wire format.
pub trait FieldType<'a>: Sized {
    /// The wire type used by this field type.
    const WIRE_TYPE: WireType;

    /// How this field type represents native values.
    type Native;

    /// Convert the native value into an instance of Self.
    fn from_native(x: Self::Native) -> Self;
    /// Convert an instance of self into a native type.
    fn into_native(self) -> Self::Native;

    /// Return a field packer for a field number and given field_value.
    fn field_packer<'b, F: FieldPackHelper<'a, Self>>(
        field_number: FieldNumber,
        field_value: &'b F,
    ) -> FieldPacker<'a, 'b, Self, F> {
        FieldPacker {
            tag: Tag {
                field_number,
                wire_type: Self::WIRE_TYPE,
            },
            field_value,
            _phantom: std::marker::PhantomData,
        }
    }
}

////////////////////////////////////////// FieldPackHelper /////////////////////////////////////////

/// A FieldPackHelper packs a tag and value in the provided space.
///
/// For option this may be zero.  For vector this may be repeated instances of tag + the contents
/// of the vector.
pub trait FieldPackHelper<'a, T: FieldType<'a>> {
    /// The size of encoding self with tag.
    fn field_pack_sz(&self, tag: &Tag) -> usize;
    /// Pack the tag into the output buffer.
    fn field_pack(&self, tag: &Tag, out: &mut [u8]);
}

impl<'a, T, F> FieldPackHelper<'a, T> for Vec<F>
where
    T: FieldType<'a>,
    F: FieldPackHelper<'a, T>,
{
    fn field_pack_sz(&self, tag: &Tag) -> usize {
        let mut bytes = 0;
        for f in self {
            bytes += f.field_pack_sz(tag);
        }
        bytes
    }

    fn field_pack(&self, tag: &Tag, out: &mut [u8]) {
        let mut out = out;
        for f in self {
            let size = f.field_pack_sz(tag);
            f.field_pack(tag, &mut out[..size]);
            out = &mut out[size..];
        }
    }
}

impl<'a, T, F> FieldPackHelper<'a, T> for Option<F>
where
    T: FieldType<'a>,
    F: FieldPackHelper<'a, T>,
{
    fn field_pack_sz(&self, tag: &Tag) -> usize {
        if let Some(f) = self {
            f.field_pack_sz(tag)
        } else {
            0
        }
    }

    fn field_pack(&self, tag: &Tag, out: &mut [u8]) {
        if let Some(f) = self {
            f.field_pack(tag, out)
        }
    }
}

impl<'a, T, F> FieldPackHelper<'a, T> for Box<F>
where
    T: FieldType<'a>,
    F: FieldPackHelper<'a, T>,
{
    fn field_pack_sz(&self, tag: &Tag) -> usize {
        let f: &F = self;
        f.field_pack_sz(tag)
    }

    fn field_pack(&self, tag: &Tag, out: &mut [u8]) {
        let f: &F = self;
        f.field_pack(tag, out)
    }
}

impl<'a, T, E> FieldPackHelper<'a, field_types::message<Result<T, E>>> for Result<T, E>
where
    T: FieldPackHelper<'a, field_types::message<T>> + 'a,
    E: FieldPackHelper<'a, field_types::message<E>> + 'a,
{
    fn field_pack_sz(&self, tag: &Tag) -> usize {
        match self {
            Ok(x) => stack_pack(tag)
                .pack(
                    stack_pack(field_types::message::field_packer(FieldNumber::must(1), x))
                        .length_prefixed(),
                )
                .pack_sz(),
            Err(e) => stack_pack(tag)
                .pack(
                    stack_pack(field_types::message::field_packer(FieldNumber::must(2), e))
                        .length_prefixed(),
                )
                .pack_sz(),
        }
    }

    fn field_pack(&self, tag: &Tag, out: &mut [u8]) {
        match self {
            Ok(x) => {
                stack_pack(tag)
                    .pack(
                        stack_pack(field_types::message::field_packer(FieldNumber::must(1), x))
                            .length_prefixed(),
                    )
                    .into_slice(out);
            }
            Err(e) => {
                stack_pack(tag)
                    .pack(
                        stack_pack(field_types::message::field_packer(FieldNumber::must(2), e))
                            .length_prefixed(),
                    )
                    .into_slice(out);
            }
        }
    }
}

///////////////////////////////////////// FieldUnpackHelper ////////////////////////////////////////

/// Given a field type that was unpacked, merge it into the rust-native value.
pub trait FieldUnpackHelper<'a, T: FieldType<'a>> {
    /// Merge the proto into self.
    fn merge_field(&mut self, proto: T);
}

impl<'a, T, F> FieldUnpackHelper<'a, T> for Vec<F>
where
    T: FieldType<'a> + Into<F>,
    F: FieldUnpackHelper<'a, T>,
{
    fn merge_field(&mut self, proto: T) {
        self.push(proto.into());
    }
}

impl<'a, T, F> FieldUnpackHelper<'a, T> for Option<F>
where
    T: FieldType<'a> + Into<F>,
    F: FieldUnpackHelper<'a, T>,
{
    fn merge_field(&mut self, proto: T) {
        *self = Some(proto.into());
    }
}

impl<'a, T, F> FieldUnpackHelper<'a, T> for Box<F>
where
    T: FieldType<'a> + Into<F>,
    F: FieldUnpackHelper<'a, T>,
{
    fn merge_field(&mut self, proto: T) {
        **self = proto.into();
    }
}

impl<T, E> FieldUnpackHelper<'_, field_types::message<Result<T, E>>> for Result<T, E> {
    fn merge_field(&mut self, proto: field_types::message<Result<T, E>>) {
        *self = proto.unwrap_message();
    }
}

//////////////////////////////////////////// FieldPacker ///////////////////////////////////////////

/// A wrapper type that combines [FieldType] and [FieldPackHelper] to make a [buffertk::Packable]
/// type.
pub struct FieldPacker<'a, 'b, T, F>
where
    T: FieldType<'a>,
    F: FieldPackHelper<'a, T>,
{
    tag: Tag,
    field_value: &'b F,
    _phantom: std::marker::PhantomData<&'a T>,
}

impl<'a, 'b, T, F> FieldPacker<'a, 'b, T, F>
where
    T: FieldType<'a>,
    F: FieldPackHelper<'a, T>,
{
    /// Create a new FieldPacker from the value and field type.
    pub fn new(tag: Tag, field_value: &'b F, field_type: std::marker::PhantomData<&'a T>) -> Self {
        Self {
            tag,
            field_value,
            _phantom: field_type,
        }
    }
}

impl<'a, T, F> Packable for FieldPacker<'a, '_, T, F>
where
    T: FieldType<'a>,
    F: FieldPackHelper<'a, T>,
{
    fn pack_sz(&self) -> usize {
        self.field_value.field_pack_sz(&self.tag)
    }

    fn pack(&self, out: &mut [u8]) {
        self.field_value.field_pack(&self.tag, out)
    }
}

/////////////////////////////////////////// FieldIterator //////////////////////////////////////////

/// An iterator over tags and byte strings.
// TODO(rescrv): This panicked once and I didn't debug it.  Fix that.
pub struct FieldIterator<'a, 'b> {
    up: Unpacker<'a>,
    err: &'b mut Option<SError>,
}

impl<'a, 'b> FieldIterator<'a, 'b> {
    /// Create a new field iterator that will return tags and their byte strings.
    pub fn new(buf: &'a [u8], err: &'b mut Option<SError>) -> Self {
        Self {
            up: Unpacker::new(buf),
            err,
        }
    }

    /// The remaining unprocessed buffer.
    pub fn remain(&self) -> &'a [u8] {
        self.up.remain()
    }
}

impl<'a> Iterator for FieldIterator<'a, '_> {
    type Item = (Tag, &'a [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        if self.up.is_empty() {
            return None;
        }
        let tag: Tag = match self.up.unpack() {
            Ok(tag) => tag,
            Err(e) => {
                *self.err = Some(e);
                return None;
            }
        };
        match tag.wire_type {
            WireType::Varint => {
                let buf: &[u8] = self.up.remain();
                let x: v64 = match self.up.unpack() {
                    Ok(x) => x,
                    Err(e) => {
                        *self.err = Some(e.into());
                        return None;
                    }
                };
                Some((tag, &buf[0..x.pack_sz()]))
            }
            WireType::SixtyFour => {
                let buf: &[u8] = self.up.remain();
                if buf.len() < 8 {
                    *self.err = Some(buffer_too_short(8, buf.len()));
                    return None;
                }
                self.up.advance(8);
                Some((tag, &buf[0..8]))
            }
            WireType::LengthDelimited => {
                let buf: &[u8] = self.up.remain();
                let x: v64 = match self.up.unpack() {
                    Ok(x) => x,
                    Err(e) => {
                        *self.err = Some(e.into());
                        return None;
                    }
                };
                let sz: usize = x.into();
                if buf.len() < x.pack_sz() + sz {
                    *self.err = Some(buffer_too_short(sz, buf.len()));
                    return None;
                }
                self.up.advance(sz);
                Some((tag, &buf[0..x.pack_sz() + sz]))
            }
            WireType::ThirtyTwo => {
                let buf: &[u8] = self.up.remain();
                if buf.len() < 4 {
                    *self.err = Some(buffer_too_short(4, buf.len()));
                    return None;
                }
                self.up.advance(4);
                Some((tag, &buf[0..4]))
            }
        }
    }
}

////////////////////////////////////////////// Unpack //////////////////////////////////////////////

/// Unpack `T` from `buf`, converting its native unpack error into [SError].
pub fn unpack_as<'a, T>(buf: &'a [u8]) -> Result<(T, &'a [u8]), SError>
where
    T: Unpackable<'a>,
    <T as Unpackable<'a>>::Error: Into<SError>,
{
    T::unpack(buf).map_err(Into::into)
}

/// Unpack `T` from an [Unpacker], converting its native unpack error into [SError].
pub fn unpack_from<'a, T>(up: &mut Unpacker<'a>) -> Result<T, SError>
where
    T: Unpackable<'a>,
    <T as Unpackable<'a>>::Error: Into<SError>,
{
    let before = up.remain();
    let (t, after) = T::unpack(before).map_err(Into::into)?;
    up.advance(before.len() - after.len());
    Ok(t)
}

////////////////////////////////////////////// Message /////////////////////////////////////////////

/// A protocol buffers messsage.
pub trait Message<'a>:
    Default
    + buffertk::Packable
    + buffertk::Unpackable<'a, Error: Into<SError> + From<buffertk::SError>>
    + FieldPackHelper<'a, field_types::message<Self>>
    + FieldUnpackHelper<'a, field_types::message<Self>>
    + 'a
{
}

impl FieldPackHelper<'_, field_types::message<SError>> for SError {
    fn field_pack_sz(&self, tag: &Tag) -> usize {
        stack_pack(tag)
            .pack(stack_pack(self).length_prefixed())
            .pack_sz()
    }

    fn field_pack(&self, tag: &Tag, out: &mut [u8]) {
        stack_pack(tag)
            .pack(stack_pack(self).length_prefixed())
            .into_slice(out);
    }
}

impl FieldUnpackHelper<'_, field_types::message<SError>> for SError {
    fn merge_field(&mut self, proto: field_types::message<SError>) {
        *self = proto.unwrap_message();
    }
}

impl From<field_types::message<SError>> for SError {
    fn from(proto: field_types::message<SError>) -> Self {
        proto.unwrap_message()
    }
}

impl Message<'_> for SError {}
