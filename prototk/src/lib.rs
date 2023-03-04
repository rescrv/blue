//! prototk is a protocol buffer (protobuf) library with a low-level API.  Unlike protobuf libraries
//! that focus on ease of use, code generation, or performance, prototk aims to expose every level
//! of abstraction it has internally so that developers can use as much or as little as they wish.

pub mod field_types;
pub mod zigzag;

pub use zigzag::unzigzag;
pub use zigzag::zigzag;

use buffertk::{v64, Packable, Unpackable, Unpacker};

/////////////////////////////////////////////// Error //////////////////////////////////////////////

/// Error captures the possible error conditions for packing and unpacking.
// TODO(rescrv):  Some notion of the error context so that these can be tracked down.
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// BufferTooShort indicates that there was a need to pack or unpack more bytes than were
    /// available in the underlying memory.
    BufferTooShort { required: usize, had: usize },
    /// InvalidFieldNumber indicates that the field is not a user-assignable field.
    InvalidFieldNumber {
        field_number: u32,
        what: &'static str,
    },
    /// UnhandledWireType inidcates that the wire type is not currently understood by prototk.
    UnhandledWireType { wire_type: u32 },
    /// TagTooLarge indicates the tag would overflow a 32-bit number.
    TagTooLarge { tag: u64 },
    /// VarintOverflow indicates that a varint field did not terminate with a number < 128.
    VarintOverflow { bytes: usize },
    /// UnsignedOverflow indicates that a value will not fit its intended (unsigned) target.
    UnsignedOverflow { value: u64 },
    /// SignedOverflow indicates that a value will not fit its intended (signed) target.
    SignedOverflow { value: i64 },
    /// WrongLength indicates that a bytes32 did not have 32 bytes.
    WrongLength { required: usize, had: usize },
    /// StringEncoding indicates that a value is not UTF-8 friendly.
    StringEncoding,
    /// UnknownDiscriminant indicates a variant that is not understood by this code.
    UnknownDiscriminant { discriminant: u32 },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::BufferTooShort { required, had } => {
                write!(f, "buffer too short:  expected {}, had {}", required, had)
            }
            Error::InvalidFieldNumber { field_number, what } => {
                write!(f, "invalid field_number={}: {}", field_number, what)
            }
            Error::UnhandledWireType { wire_type } => write!(
                f,
                "wire_type={} not handled by this implementation",
                wire_type
            ),
            Error::TagTooLarge { tag } => write!(f, "tag={} overflows 32-bits", tag),
            Error::VarintOverflow { bytes } => {
                write!(f, "varint did not fit in space={} bytes", bytes)
            }
            Error::UnsignedOverflow { value } => {
                write!(f, "unsigned integer cannot hold value={}", value)
            }
            Error::SignedOverflow { value } => {
                write!(f, "signed integer cannot hold value={}", value)
            }
            Error::WrongLength { required, had } => {
                write!(f, "buffer wrong length: expected {}, had {}", required, had)
            }
            Error::StringEncoding => {
                write!(f, "strings must be encoded in UTF-8")
            }
            Error::UnknownDiscriminant { discriminant } => {
                write!(f, "unknown discriminant {}", discriminant)
            }
        }
    }
}

impl From<buffertk::Error> for Error {
    fn from(x: buffertk::Error) -> Self {
        match x {
            buffertk::Error::BufferTooShort { required, had } => {
                Error::BufferTooShort { required, had }
            }
            buffertk::Error::VarintOverflow { bytes } => Error::VarintOverflow { bytes },
            buffertk::Error::UnsignedOverflow { value } => Error::UnsignedOverflow { value },
            buffertk::Error::SignedOverflow { value } => Error::SignedOverflow { value },
        }
    }
}

///////////////////////////////////////////// WireType /////////////////////////////////////////////

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
    pub fn new(tag_bits: u32) -> Result<WireType, Error> {
        match tag_bits {
            0 => Ok(WireType::Varint),
            1 => Ok(WireType::SixtyFour),
            2 => Ok(WireType::LengthDelimited),
            5 => Ok(WireType::ThirtyTwo),
            _ => Err(Error::UnhandledWireType {
                wire_type: tag_bits,
            }),
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

pub const FIRST_FIELD_NUMBER: u32 = 1;
pub const LAST_FIELD_NUMBER: u32 = (1 << 29) - 1;

pub const FIRST_RESERVED_FIELD_NUMBER: u32 = 19000;
pub const LAST_RESERVED_FIELD_NUMBER: u32 = 19999;

/// [FieldNumber] wraps a u32 and guards it against reserved or invalid field numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    pub fn new(field_number: u32) -> Result<FieldNumber, Error> {
        if field_number < FIRST_FIELD_NUMBER {
            return Err(Error::InvalidFieldNumber {
                field_number,
                what: "field number must be positive integer",
            });
        }
        if field_number > LAST_FIELD_NUMBER {
            return Err(Error::InvalidFieldNumber {
                field_number,
                what: "field number too large",
            });
        }
        if (FIRST_RESERVED_FIELD_NUMBER..=LAST_RESERVED_FIELD_NUMBER).contains(&field_number) {
            return Err(Error::InvalidFieldNumber {
                field_number,
                what: "field is reserved",
            });
        }
        Ok(FieldNumber { field_number })
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
#[derive(Clone, Debug)]
pub struct Tag {
    pub field_number: FieldNumber,
    pub wire_type: WireType,
}

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
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let mut up = Unpacker::new(buf);
        let tag: v64 = up.unpack()?;
        let tag: u64 = tag.into();
        if tag > u32::max_value() as u64 {
            return Err(Error::TagTooLarge { tag });
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

pub trait FieldType<'a> {
    const WIRE_TYPE: WireType;

    type Native;

    fn from_native(x: Self::Native) -> Self;
    fn into_native(self) -> Self::Native;
}

////////////////////////////////////////// FieldPackHelper /////////////////////////////////////////

pub trait FieldPackHelper<'a, T: FieldType<'a>> {
    fn field_pack_sz(&self, tag: &Tag) -> usize;
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

///////////////////////////////////////// FieldUnpackHelper ////////////////////////////////////////

pub trait FieldUnpackHelper<'a, T: FieldType<'a>> {
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

//////////////////////////////////////////// FieldPacker ///////////////////////////////////////////

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
    pub fn new(tag: Tag, field_value: &'b F, field_type: std::marker::PhantomData<&'a T>) -> Self {
        Self {
            tag,
            field_value,
            _phantom: field_type,
        }
    }
}

impl<'a, 'b, T, F> Packable for FieldPacker<'a, 'b, T, F>
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

pub struct FieldIterator<'a, 'b> {
    up: Unpacker<'a>,
    err: &'b mut Option<Error>,
}

impl<'a, 'b> FieldIterator<'a, 'b> {
    pub fn new(buf: &'a [u8], err: &'b mut Option<Error>) -> Self {
        Self {
            up: Unpacker::new(buf),
            err,
        }
    }

    pub fn remain(&self) -> &'a [u8] {
        self.up.remain()
    }
}

impl<'a, 'b> Iterator for FieldIterator<'a, 'b> {
    type Item = (Tag, &'a [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        if self.up.is_empty() {
            return None;
        }
        let tag: Tag = match self.up.unpack() {
            Ok(tag) => { tag },
            Err(e) => {
                *self.err = Some(e);
                return None;
            },
        };
        match tag.wire_type {
            WireType::Varint => {
                let buf: &[u8] = self.up.remain();
                let x: v64 = match self.up.unpack() {
                    Ok(x) => { x },
                    Err(e) => {
                        *self.err = Some(e.into());
                        return None;
                    },
                };
                Some((tag, &buf[0..x.pack_sz()]))
            },
            WireType::SixtyFour => {
                let buf: &[u8] = self.up.remain();
                if buf.len() < 8 {
                    *self.err = Some(Error::BufferTooShort { required: 8, had: buf.len() });
                    return None;
                }
                self.up.advance(8);
                Some((tag, &buf[0..8]))
            },
            WireType::LengthDelimited => {
                let buf: &[u8] = self.up.remain();
                let x: v64 = match self.up.unpack() {
                    Ok(x) => { x },
                    Err(e) => {
                        *self.err = Some(e.into());
                        return None;
                    },
                };
                let sz: usize = x.into();
                self.up.advance(sz);
                Some((tag, &buf[0..x.pack_sz() + sz]))
            },
            WireType::ThirtyTwo => {
                let buf: &[u8] = self.up.remain();
                if buf.len() < 4 {
                    *self.err = Some(Error::BufferTooShort { required: 4, had: buf.len() });
                    return None;
                }
                self.up.advance(4);
                Some((tag, &buf[0..4]))
            },
        }
    }
}

////////////////////////////////////////////// Message /////////////////////////////////////////////

pub trait Message<'a>: Default + buffertk::Packable + buffertk::Unpackable<'a> {}
