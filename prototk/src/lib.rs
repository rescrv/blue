//! prototk is a protocol buffer (protobuf) library with a low-level API.  Unlike protobuf libraries
//! that focus on ease of use, code generation, or performance, prototk aims to expose every level
//! of abstraction it has internally so that developers can use as much or as little as they wish.

pub mod field_types;
pub mod zigzag;

pub use zigzag::unzigzag;
pub use zigzag::zigzag;

use buffertk::{stack_pack, v64, Packable, Unpackable, Unpacker};

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

#[derive(Clone, Debug, PartialEq, Eq)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldNumber {
    field_number: u32,
}

impl FieldNumber {
    pub fn must(field_number: u32) -> FieldNumber {
        FieldNumber::new(field_number).unwrap()
    }

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
        if field_number >= FIRST_RESERVED_FIELD_NUMBER && field_number <= LAST_RESERVED_FIELD_NUMBER
        {
            return Err(Error::InvalidFieldNumber {
                field_number,
                what: "field is reserved",
            });
        }
        Ok(FieldNumber { field_number })
    }
}

impl Into<u32> for FieldNumber {
    fn into(self) -> u32 {
        self.field_number
    }
}

impl std::cmp::PartialEq<u32> for FieldNumber {
    fn eq(&self, other: &u32) -> bool {
        self.field_number == *other
    }
}

//////////////////////////////////////////////// Tag ///////////////////////////////////////////////

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

pub trait FieldType<'a>: Unpackable<'a> {
    const WIRE_TYPE: WireType;

    type NativeType;

    fn from_native(x: Self::NativeType) -> Self;
}

//////////////////////////////////////////// FieldHelper ///////////////////////////////////////////

pub trait FieldHelper<'a, T: FieldType<'a>> {
    fn prototk_pack_sz(tag: &Tag, field: &Self) -> usize;
    fn prototk_pack(tag: &Tag, field: &Self, out: &mut [u8]);

    fn prototk_convert_field<'b>(proto: T, out: &'b mut Self) where 'a: 'b;
    fn prototk_convert_variant(proto: T) -> Self;
}

impl<'a, T: FieldType<'a>, F: Default + FieldHelper<'a, T>> FieldHelper<'a, T> for Vec<F> {
    fn prototk_pack_sz(tag: &Tag, field: &Self) -> usize {
        let mut bytes = 0;
        for f in field {
            bytes += <F as FieldHelper<'a, T>>::prototk_pack_sz(tag, f);
        }
        bytes
    }

    fn prototk_pack(tag: &Tag, field: &Self, out: &mut [u8]) {
        let mut out = out;
        for f in field {
            let size = <F as FieldHelper<'a, T>>::prototk_pack_sz(tag, f);
            <F as FieldHelper<'a, T>>::prototk_pack(tag, f, &mut out[..size]);
            out = &mut out[size..];
        }
    }

    fn prototk_convert_field<'b>(proto: T, out: &'b mut Self) where 'a: 'b, {
        out.push(F::default());
        let idx = out.len() - 1;
        <F as FieldHelper<'a, T>>::prototk_convert_field(proto, &mut out[idx]);
    }

    fn prototk_convert_variant(proto: T) -> Self {
        vec![<F as FieldHelper<'a, T>>::prototk_convert_variant(proto)]
    }
}

impl<'a, T: FieldType<'a>, F: Default + FieldHelper<'a, T>> FieldHelper<'a, T> for Option<F> {
    fn prototk_pack_sz(tag: &Tag, field: &Self) -> usize {
        if let Some(f) = &field {
            <F as FieldHelper<'a, T>>::prototk_pack_sz(tag, f)
        } else {
            0
        }
    }

    fn prototk_pack(tag: &Tag, field: &Self, out: &mut [u8]) {
        if let Some(f) = &field {
            <F as FieldHelper<'a, T>>::prototk_pack(tag, f, out)
        }
    }

    fn prototk_convert_field<'b>(proto: T, out: &'b mut Self) where 'a: 'b, {
        *out = Self::prototk_convert_variant(proto);
    }

    fn prototk_convert_variant(proto: T) -> Self {
        Some(<F as FieldHelper<'a, T>>::prototk_convert_variant(proto))
    }
}

//////////////////////////////////////////// FieldPacker ///////////////////////////////////////////

pub struct FieldPacker<'a, 'b, T: FieldType<'a>, F: FieldHelper<'a, T>> {
    tag: Tag,
    field_value: &'b F,
    _phantom: std::marker::PhantomData<&'a T>,
}

impl<'a, 'b, T: FieldType<'a>, F: FieldHelper<'a, T>> FieldPacker<'a, 'b, T, F> {
    pub fn new(tag: Tag, field_value: &'b F, field_type: std::marker::PhantomData<&'a T>) -> Self {
        Self {
            tag,
            field_value,
            _phantom: field_type,
        }
    }
}

impl<'a, 'b, T: FieldType<'a>, F: FieldHelper<'a, T>> Packable for FieldPacker<'a, 'b, T, F> {
    fn pack_sz(&self) -> usize {
        FieldHelper::prototk_pack_sz(&self.tag, self.field_value)
    }

    fn pack(&self, out: &mut [u8]) {
        FieldHelper::prototk_pack(&self.tag, self.field_value, out)
    }
}

////////////////////////////////////////////// Message /////////////////////////////////////////////

pub trait Message<'a>: Default + buffertk::Packable + buffertk::Unpackable<'a> {}

////////////////////////////////////////////// Builder /////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct Builder {
    buffer: Vec<u8>,
}

impl Builder {
    pub fn push<'a, T, const N: u32>(&mut self, field_value: T::NativeType) -> &mut Self
    where
        T: FieldType<'a> + 'a,
        T::NativeType: FieldHelper<'a, T> + 'a,
    {
        let tag = Tag {
            field_number: FieldNumber::must(N),
            wire_type: T::WIRE_TYPE,
        };
        let packer = FieldPacker::new(tag, &field_value, std::marker::PhantomData::<&T>);
        stack_pack(packer).append_to_vec(&mut self.buffer);
        self
    }

    pub fn append(&mut self, buffer: &[u8]) -> &mut Self {
        self.buffer.extend_from_slice(buffer);
        self
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.buffer
    }
}
