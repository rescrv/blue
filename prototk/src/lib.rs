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

    // TODO(rescrv): custom error type so that apps can extend
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
            },
            Error::UnsignedOverflow { value } => {
                write!(f, "unsigned integer cannot hold value={}", value)
            }
            Error::SignedOverflow { value } => {
                write!(f, "signed integer cannot hold value={}", value)
            }
        }
    }
}

impl From<buffertk::Error> for Error {
    fn from(x: buffertk::Error) -> Self {
        match x {
            buffertk::Error::BufferTooShort { required, had } => Error::BufferTooShort { required, had },
            buffertk::Error::VarintOverflow { bytes } => Error::VarintOverflow { bytes },
            buffertk::Error::UnsignedOverflow { value } => Error::UnsignedOverflow { value },
            buffertk::Error::SignedOverflow { value } => Error::SignedOverflow { value },
        }
    }
}

///////////////////////////////////////////// WireType /////////////////////////////////////////////

#[derive(Debug, PartialEq, Eq)]
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

#[derive(Debug)]
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

pub trait FieldType<'a>: Packable + Unpackable<'a> {
    const WIRE_TYPE: WireType;
    const LENGTH_PREFIXED: bool;

    type NativeType;

    fn into_native(self) -> Self::NativeType;
    fn from_native(x: Self::NativeType) -> Self;

    fn assign<A: FieldTypeAssigner<NativeType=Self::NativeType>>(lhs: &mut A, x: Self::NativeType) {
        lhs.assign_field_type(x);
    }
}
////////////////////////////////////////// FieldTypePacker /////////////////////////////////////////

pub struct FieldTypePacker<'a, A, B> {
    t: Tag,
    a: std::marker::PhantomData<A>,
    b: &'a B,
}

impl<'a, A, B> FieldTypePacker<'a, A, B> {
    pub fn new(t: Tag, a: std::marker::PhantomData<A>, b: &'a B) -> Self {
        Self {
            t,
            a,
            b,
        }
    }
}

pub trait FieldTypePackable {}

impl FieldTypePackable for i32 {}
impl FieldTypePackable for i64 {}
impl FieldTypePackable for u32 {}
impl FieldTypePackable for u64 {}
impl FieldTypePackable for f32 {}
impl FieldTypePackable for f64 {}
impl<'a> FieldTypePackable for &'a [u8] {}
impl<'a> FieldTypePackable for &'a str {}
impl<'a> FieldTypePackable for String {}
impl<'a, M: Message<'a>> FieldTypePackable for M {}

impl<'a, F, T> Packable for FieldTypePacker<'a, F, T>
where
    F: FieldType<'a>,
    T: FieldTypePackable + Clone,
    &'a T: std::convert::Into<F>,
{
    fn pack_sz(&self) -> usize {
        let pb: F = self.b.into();
        stack_pack(&self.t).pack(&pb).pack_sz()
    }

    fn pack(&self, buf: &mut [u8]) {
        let pb: F = self.b.into();
        stack_pack(&self.t).pack(&pb).into_slice(buf);
    }
}

pub trait FieldTypeVectorPackable {}

impl FieldTypeVectorPackable for i32 {}
impl FieldTypeVectorPackable for i64 {}
impl FieldTypeVectorPackable for u32 {}
impl FieldTypeVectorPackable for u64 {}
impl FieldTypeVectorPackable for f32 {}
impl FieldTypeVectorPackable for f64 {}
impl<'a> FieldTypeVectorPackable for &'a [u8] {}
impl<'a, M: Message<'a>> FieldTypeVectorPackable for M {}

impl<'a, F, T> Packable for FieldTypePacker<'a, F, Vec<T>>
where
    F: FieldType<'a>,
    T: FieldTypeVectorPackable + Clone,
    &'a T: std::convert::Into<F>,
{
    fn pack_sz(&self) -> usize {
        let mut sz = self.t.pack_sz() * self.b.len();
        for x in self.b.iter() {
            let px: F = x.into();
            let elem_sz = px.pack_sz();
            if F::LENGTH_PREFIXED {
                sz += v64::from(elem_sz).pack_sz();
            }
            sz += elem_sz;
        }
        sz
    }

    fn pack(&self, buffer: &mut [u8]) {
        let tag_sz = self.t.pack_sz();
        let mut total_sz = 0;
        for x in self.b.iter() {
            // TODO(rescrv): cleanup
            let px: F = x.into();
            let sz = px.pack_sz();
            if F::LENGTH_PREFIXED {
                let prefix: v64 = sz.into();
                let buf = &mut buffer[total_sz..total_sz+tag_sz+prefix.pack_sz()+sz];
                stack_pack(&self.t).pack(prefix).pack(px).into_slice(buf);
            } else {
                let buf = &mut buffer[total_sz..total_sz+tag_sz+sz];
                stack_pack(&self.t).pack(px).into_slice(buf);
            }
            total_sz += tag_sz + sz;
        }
    }
}

///////////////////////////////////////// FieldTypeAssigner ////////////////////////////////////////

pub trait FieldTypeAssigner {
    type NativeType;

    fn assign_field_type(&mut self, x: Self::NativeType);
}

trait TemplateFieldTypeAssigner {}

impl FieldTypeAssigner for i32 {
    type NativeType = i32;

    fn assign_field_type(&mut self, x: i32) {
        *self = x;
    }
}

impl FieldTypeAssigner for i64 {
    type NativeType = i64;

    fn assign_field_type(&mut self, x: i64) {
        *self = x;
    }
}

impl FieldTypeAssigner for u32 {
    type NativeType = u32;

    fn assign_field_type(&mut self, x: u32) {
        *self = x;
    }
}

impl FieldTypeAssigner for u64 {
    type NativeType = u64;

    fn assign_field_type(&mut self, x: u64) {
        *self = x;
    }
}

impl FieldTypeAssigner for f32 {
    type NativeType = f32;

    fn assign_field_type(&mut self, x: f32) {
        *self = x;
    }
}

impl FieldTypeAssigner for f64 {
    type NativeType = f64;

    fn assign_field_type(&mut self, x: f64) {
        *self = x;
    }
}

impl<'a> FieldTypeAssigner for &'a [u8] {
    type NativeType = &'a [u8];

    fn assign_field_type(&mut self, x: &'a [u8]) {
        *self = x;
    }
}

impl<'a> FieldTypeAssigner for &'a str {
    type NativeType = &'a str;

    fn assign_field_type(&mut self, x: &'a str) {
        *self = x;
    }
}

impl FieldTypeAssigner for String {
    type NativeType = String;

    fn assign_field_type(&mut self, x: String) {
        *self = x;
    }
}

impl<'a, M: Message<'a>> FieldTypeAssigner for M {
    type NativeType = M;

    fn assign_field_type(&mut self, x: M) {
        *self = x;
    }
}

impl FieldTypeAssigner for Vec<i32> {
    type NativeType = i32;

    fn assign_field_type(&mut self, x: i32) {
        self.push(x);
    }
}

impl FieldTypeAssigner for Vec<i64> {
    type NativeType = i64;

    fn assign_field_type(&mut self, x: i64) {
        self.push(x);
    }
}

impl FieldTypeAssigner for Vec<u32> {
    type NativeType = u32;

    fn assign_field_type(&mut self, x: u32) {
        self.push(x);
    }
}

impl FieldTypeAssigner for Vec<u64> {
    type NativeType = u64;

    fn assign_field_type(&mut self, x: u64) {
        self.push(x);
    }
}

impl FieldTypeAssigner for Vec<f32> {
    type NativeType = f32;

    fn assign_field_type(&mut self, x: f32) {
        self.push(x);
    }
}

impl FieldTypeAssigner for Vec<f64> {
    type NativeType = f64;

    fn assign_field_type(&mut self, x: f64) {
        self.push(x);
    }
}

impl<'a, M: Message<'a>> FieldTypeAssigner for Vec<M> {
    type NativeType = M;

    fn assign_field_type(&mut self, x: M) {
        self.push(x);
    }
}

////////////////////////////////////////////// Message /////////////////////////////////////////////

// TODO(rescrv):  There's an extra clone type here because I couldn't do From/Into of
// message<M:Message> to make it zero copy.  Get the macros up and revisit.
pub trait Message<'a>: Clone + Default + buffertk::Packable + buffertk::Unpackable<'a> {
}

impl<'a, M> Message<'a> for &'a M
where
    M: Message<'a>,
    &'a M: Default + buffertk::Packable + buffertk::Unpackable<'a>,
{
}
