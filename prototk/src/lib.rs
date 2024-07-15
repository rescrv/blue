#![doc = include_str!("../README.md")]

use std::fmt::{Debug, Display, Formatter};

pub mod field_types;
mod zigzag;

pub use zigzag::unzigzag;
pub use zigzag::zigzag;

use buffertk::{stack_pack, v64, Packable, Unpackable, Unpacker};

/////////////////////////////////////////////// Error //////////////////////////////////////////////

/// Error captures the possible error conditions for packing and unpacking.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum Error {
    /// The default error is succes.
    #[default]
    Success,
    /// BufferTooShort indicates that there was a need to pack or unpack more bytes than were
    /// available in the underlying memory.
    BufferTooShort {
        /// The number of bytes required to unpack.
        required: usize,
        /// The number of bytes available to unpack.
        had: usize,
    },
    /// InvalidFieldNumber indicates that the field is not a user-assignable field.
    InvalidFieldNumber {
        /// The u32 field number that's invalid.
        field_number: u32,
        /// A human-readable description of why the field number is invalid.
        what: String,
    },
    /// UnhandledWireType indicates that the wire_type is not understood by this process and cannot
    /// be skipped.
    UnhandledWireType {
        /// The wire type that's not handled.
        wire_type: u32,
    },
    /// TagTooLarge indicates the tag would overflow a 32-bit number.
    TagTooLarge {
        /// The tag that's too large.
        tag: u64,
    },
    /// VarintOverflow indicates that a varint field did not terminate with a number < 128.
    VarintOverflow {
        /// The number of bytes witnessed in the varint.
        bytes: usize,
    },
    /// UnsignedOverflow indicates that a value will not fit its intended (unsigned) target.
    UnsignedOverflow {
        /// The u64 that doesn't fit a u32.
        value: u64,
    },
    /// SignedOverflow indicates that a value will not fit its intended (signed) target.
    SignedOverflow {
        /// The i64 that doesn't fit an i32.
        value: i64,
    },
    /// WrongLength indicates that a bytes32 did not have 32 bytes.
    WrongLength {
        /// The required number of bytes for the type.
        required: usize,
        /// The number of bytes the type claims to be.
        had: usize,
    },
    /// StringEncoding indicates that a value is not UTF-8 friendly.
    StringEncoding,
    /// UnknownDiscriminant indicates a variant that is not understood by this code.
    UnknownDiscriminant {
        /// The discriminant that's not handled by this process.
        discriminant: u32,
    },
    /// NotAChar indicates that the prescribed value was tried to unpack as a char, but it's not a
    /// char.
    NotAChar {
        /// Value that's not a char.
        value: u32,
    },
}

impl Display for Error {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), std::fmt::Error> {
        match self {
            Error::Success {} => fmt.debug_struct("SerializationError").finish(),
            Error::BufferTooShort { required, had } => fmt
                .debug_struct("BufferTooShort")
                .field("required", required)
                .field("had", had)
                .finish(),
            Error::InvalidFieldNumber { field_number, what } => fmt
                .debug_struct("InvalidFieldNumber")
                .field("field_number", field_number)
                .field("what", what)
                .finish(),
            Error::UnhandledWireType { wire_type } => fmt
                .debug_struct("UnhandledWireType")
                .field("wire_type", wire_type)
                .finish(),
            Error::TagTooLarge { tag } => {
                fmt.debug_struct("TagTooLarge").field("tag", tag).finish()
            }
            Error::VarintOverflow { bytes } => fmt
                .debug_struct("VarintOverflow")
                .field("bytes", bytes)
                .finish(),
            Error::UnsignedOverflow { value } => fmt
                .debug_struct("UnsignedOverflow")
                .field("value", value)
                .finish(),
            Error::SignedOverflow { value } => fmt
                .debug_struct("SignedOverflow")
                .field("value", value)
                .finish(),
            Error::WrongLength { required, had } => fmt
                .debug_struct("WrongLength")
                .field("required", required)
                .field("had", had)
                .finish(),
            Error::StringEncoding => fmt.debug_struct("StringEncoding").finish(),
            Error::UnknownDiscriminant { discriminant } => fmt
                .debug_struct("UnknownDiscriminant")
                .field("discriminant", discriminant)
                .finish(),
            Error::NotAChar { value } => {
                fmt.debug_struct("NotAChar").field("value", value).finish()
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
            buffertk::Error::TagTooLarge { tag } => Error::TagTooLarge { tag },
            buffertk::Error::UnknownDiscriminant { discriminant } => {
                Error::UnknownDiscriminant { discriminant }
            }
            buffertk::Error::NotAChar { value } => Error::NotAChar { value },
        }
    }
}

impl Packable for Error {
    fn pack_sz(&self) -> usize {
        match self {
            Error::Success => {
                let prototk_empty: &[u8] = &[];
                stack_pack(field_types::bytes::field_packer(
                    FieldNumber::must(262144),
                    &prototk_empty,
                ))
                .pack_sz()
            }
            Error::BufferTooShort { required, had } => {
                let pa = stack_pack(());
                let pa = pa.pack(field_types::uint64::field_packer(
                    FieldNumber::must(1),
                    required,
                ));
                let pa = pa.pack(field_types::uint64::field_packer(FieldNumber::must(2), had));
                stack_pack(Tag {
                    field_number: FieldNumber::must(262145),
                    wire_type: WireType::LengthDelimited,
                })
                .pack(pa.length_prefixed())
                .pack_sz()
            }
            Error::InvalidFieldNumber { field_number, what } => {
                let pa = stack_pack(());
                let pa = pa.pack(field_types::uint32::field_packer(
                    FieldNumber::must(1),
                    field_number,
                ));
                let pa = pa.pack(field_types::string::field_packer(
                    FieldNumber::must(2),
                    what,
                ));
                stack_pack(Tag {
                    field_number: FieldNumber::must(262146),
                    wire_type: WireType::LengthDelimited,
                })
                .pack(pa.length_prefixed())
                .pack_sz()
            }
            Error::UnhandledWireType { wire_type } => {
                let pa = stack_pack(());
                let pa = pa.pack(field_types::uint32::field_packer(
                    FieldNumber::must(1),
                    wire_type,
                ));
                stack_pack(Tag {
                    field_number: FieldNumber::must(262147),
                    wire_type: WireType::LengthDelimited,
                })
                .pack(pa.length_prefixed())
                .pack_sz()
            }
            Error::TagTooLarge { tag } => {
                let pa = stack_pack(());
                let pa = pa.pack(field_types::uint64::field_packer(FieldNumber::must(1), tag));
                stack_pack(Tag {
                    field_number: FieldNumber::must(262148),
                    wire_type: WireType::LengthDelimited,
                })
                .pack(pa.length_prefixed())
                .pack_sz()
            }
            Error::VarintOverflow { bytes } => {
                let pa = stack_pack(());
                let pa = pa.pack(field_types::uint64::field_packer(
                    FieldNumber::must(1),
                    bytes,
                ));
                stack_pack(Tag {
                    field_number: FieldNumber::must(262149),
                    wire_type: WireType::LengthDelimited,
                })
                .pack(pa.length_prefixed())
                .pack_sz()
            }
            Error::UnsignedOverflow { value } => {
                let pa = stack_pack(());
                let pa = pa.pack(field_types::uint64::field_packer(
                    FieldNumber::must(1),
                    value,
                ));
                stack_pack(Tag {
                    field_number: FieldNumber::must(262150),
                    wire_type: WireType::LengthDelimited,
                })
                .pack(pa.length_prefixed())
                .pack_sz()
            }
            Error::SignedOverflow { value } => {
                let pa = stack_pack(());
                let pa = pa.pack(field_types::int64::field_packer(
                    FieldNumber::must(1),
                    value,
                ));
                stack_pack(Tag {
                    field_number: FieldNumber::must(262151),
                    wire_type: WireType::LengthDelimited,
                })
                .pack(pa.length_prefixed())
                .pack_sz()
            }
            Error::WrongLength { required, had } => {
                let pa = stack_pack(());
                let pa = pa.pack(field_types::uint64::field_packer(
                    FieldNumber::must(1),
                    required,
                ));
                let pa = pa.pack(field_types::uint64::field_packer(FieldNumber::must(2), had));
                stack_pack(Tag {
                    field_number: FieldNumber::must(262152),
                    wire_type: WireType::LengthDelimited,
                })
                .pack(pa.length_prefixed())
                .pack_sz()
            }
            Error::StringEncoding => {
                let prototk_empty: &[u8] = &[];
                stack_pack(field_types::bytes::field_packer(
                    FieldNumber::must(262153),
                    &prototk_empty,
                ))
                .pack_sz()
            }
            Error::UnknownDiscriminant { discriminant } => {
                let pa = stack_pack(());
                let pa = pa.pack(field_types::uint32::field_packer(
                    FieldNumber::must(1),
                    discriminant,
                ));
                stack_pack(Tag {
                    field_number: FieldNumber::must(262154),
                    wire_type: WireType::LengthDelimited,
                })
                .pack(pa.length_prefixed())
                .pack_sz()
            }
            Error::NotAChar { value } => {
                let pa = stack_pack(());
                let pa = pa.pack(field_types::uint32::field_packer(
                    FieldNumber::must(1),
                    value,
                ));
                stack_pack(Tag {
                    field_number: FieldNumber::must(262155),
                    wire_type: WireType::LengthDelimited,
                })
                .pack(pa.length_prefixed())
                .pack_sz()
            }
        }
    }

    fn pack(&self, buf: &mut [u8]) {
        match self {
            Error::Success => {
                let prototk_empty: &[u8] = &[];
                stack_pack(field_types::bytes::field_packer(
                    FieldNumber::must(262144),
                    &prototk_empty,
                ))
                .into_slice(buf);
            }
            Error::BufferTooShort { required, had } => {
                let pa = stack_pack(());
                let pa = pa.pack(field_types::uint64::field_packer(
                    FieldNumber::must(1),
                    required,
                ));
                let pa = pa.pack(field_types::uint64::field_packer(FieldNumber::must(2), had));
                stack_pack(Tag {
                    field_number: FieldNumber::must(262145),
                    wire_type: WireType::LengthDelimited,
                })
                .pack(pa.length_prefixed())
                .into_slice(buf);
            }
            Error::InvalidFieldNumber { field_number, what } => {
                let pa = stack_pack(());
                let pa = pa.pack(field_types::uint32::field_packer(
                    FieldNumber::must(1),
                    field_number,
                ));
                let pa = pa.pack(field_types::string::field_packer(
                    FieldNumber::must(2),
                    what,
                ));
                stack_pack(Tag {
                    field_number: FieldNumber::must(262146),
                    wire_type: WireType::LengthDelimited,
                })
                .pack(pa.length_prefixed())
                .into_slice(buf);
            }
            Error::UnhandledWireType { wire_type } => {
                let pa = stack_pack(());
                let pa = pa.pack(field_types::uint32::field_packer(
                    FieldNumber::must(1),
                    wire_type,
                ));
                stack_pack(Tag {
                    field_number: FieldNumber::must(262147),
                    wire_type: WireType::LengthDelimited,
                })
                .pack(pa.length_prefixed())
                .into_slice(buf);
            }
            Error::TagTooLarge { tag } => {
                let pa = stack_pack(());
                let pa = pa.pack(field_types::uint64::field_packer(FieldNumber::must(1), tag));
                stack_pack(Tag {
                    field_number: FieldNumber::must(262148),
                    wire_type: WireType::LengthDelimited,
                })
                .pack(pa.length_prefixed())
                .into_slice(buf);
            }
            Error::VarintOverflow { bytes } => {
                let pa = stack_pack(());
                let pa = pa.pack(field_types::uint64::field_packer(
                    FieldNumber::must(1),
                    bytes,
                ));
                stack_pack(Tag {
                    field_number: FieldNumber::must(262149),
                    wire_type: WireType::LengthDelimited,
                })
                .pack(pa.length_prefixed())
                .into_slice(buf);
            }
            Error::UnsignedOverflow { value } => {
                let pa = stack_pack(());
                let pa = pa.pack(field_types::uint64::field_packer(
                    FieldNumber::must(1),
                    value,
                ));
                stack_pack(Tag {
                    field_number: FieldNumber::must(262150),
                    wire_type: WireType::LengthDelimited,
                })
                .pack(pa.length_prefixed())
                .into_slice(buf);
            }
            Error::SignedOverflow { value } => {
                let pa = stack_pack(());
                let pa = pa.pack(field_types::int64::field_packer(
                    FieldNumber::must(1),
                    value,
                ));
                stack_pack(Tag {
                    field_number: FieldNumber::must(262151),
                    wire_type: WireType::LengthDelimited,
                })
                .pack(pa.length_prefixed())
                .into_slice(buf);
            }
            Error::WrongLength { required, had } => {
                let pa = stack_pack(());
                let pa = pa.pack(field_types::uint64::field_packer(
                    FieldNumber::must(1),
                    required,
                ));
                let pa = pa.pack(field_types::uint64::field_packer(FieldNumber::must(2), had));
                stack_pack(Tag {
                    field_number: FieldNumber::must(262152),
                    wire_type: WireType::LengthDelimited,
                })
                .pack(pa.length_prefixed())
                .into_slice(buf);
            }
            Error::StringEncoding => {
                let prototk_empty: &[u8] = &[];
                stack_pack(field_types::bytes::field_packer(
                    FieldNumber::must(262153),
                    &prototk_empty,
                ))
                .into_slice(buf);
            }
            Error::UnknownDiscriminant { discriminant } => {
                let pa = stack_pack(());
                let pa = pa.pack(field_types::uint32::field_packer(
                    FieldNumber::must(1),
                    discriminant,
                ));
                stack_pack(Tag {
                    field_number: FieldNumber::must(262154),
                    wire_type: WireType::LengthDelimited,
                })
                .pack(pa.length_prefixed())
                .into_slice(buf);
            }
            Error::NotAChar { value } => {
                let pa = stack_pack(());
                let pa = pa.pack(field_types::uint32::field_packer(
                    FieldNumber::must(1),
                    value,
                ));
                stack_pack(Tag {
                    field_number: FieldNumber::must(262155),
                    wire_type: WireType::LengthDelimited,
                })
                .pack(pa.length_prefixed())
                .into_slice(buf);
            }
        }
    }
}

impl<'a> Unpackable<'a> for Error {
    type Error = Error;

    fn unpack<'b>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error>
    where
        'b: 'a,
    {
        let mut up = Unpacker::new(buf);
        let tag: Tag = up.unpack()?;
        let num: u32 = tag.field_number.into();
        let wire_type: WireType = tag.wire_type;
        match (num, wire_type) {
            (262144, WireType::LengthDelimited) => {
                let x: v64 = up.unpack()?;
                up.advance(x.into());
                Ok((Error::Success, up.remain()))
            }
            (262145, WireType::LengthDelimited) => {
                let length: v64 = up.unpack()?;
                let mut error: Option<Error> = None;
                let local_buf: &'b [u8] = &up.remain()[0..length.into()];
                up.advance(length.into());
                let fields = FieldIterator::new(local_buf, &mut error);
                let mut prototk_field_required: field_types::uint64 =
                    field_types::uint64::default();
                let mut prototk_field_had: field_types::uint64 = field_types::uint64::default();
                for (tag, buf) in fields {
                    let num: u32 = tag.field_number.into();
                    match (num, tag.wire_type) {
                        (1, field_types::uint64::WIRE_TYPE) => {
                            (prototk_field_required, _) = Unpackable::unpack(buf)?;
                        }
                        (2, field_types::uint64::WIRE_TYPE) => {
                            (prototk_field_had, _) = Unpackable::unpack(buf)?;
                        }
                        (_, _) => {
                            return Err(Error::UnknownDiscriminant { discriminant: num });
                        }
                    }
                }
                let ret = Error::BufferTooShort {
                    required: prototk_field_required.into(),
                    had: prototk_field_had.into(),
                };
                Ok((ret, up.remain()))
            }
            (262146, WireType::LengthDelimited) => {
                let length: v64 = up.unpack()?;
                let mut error: Option<Error> = None;
                let local_buf: &'b [u8] = &up.remain()[0..length.into()];
                up.advance(length.into());
                let fields = FieldIterator::new(local_buf, &mut error);
                let mut prototk_field_field_number: field_types::uint32 =
                    field_types::uint32::default();
                let mut prototk_field_what: field_types::string = field_types::string::default();
                for (tag, buf) in fields {
                    let num: u32 = tag.field_number.into();
                    match (num, tag.wire_type) {
                        (1, field_types::uint32::WIRE_TYPE) => {
                            (prototk_field_field_number, _) = Unpackable::unpack(buf)?;
                        }
                        (2, field_types::string::WIRE_TYPE) => {
                            (prototk_field_what, _) = Unpackable::unpack(buf)?;
                        }
                        (_, _) => {
                            return Err(Error::UnknownDiscriminant { discriminant: num });
                        }
                    }
                }
                let ret = Error::InvalidFieldNumber {
                    field_number: prototk_field_field_number.into(),
                    what: prototk_field_what.into(),
                };
                Ok((ret, up.remain()))
            }
            (262147, WireType::LengthDelimited) => {
                let length: v64 = up.unpack()?;
                let mut error: Option<Error> = None;
                let local_buf: &'b [u8] = &up.remain()[0..length.into()];
                up.advance(length.into());
                let fields = FieldIterator::new(local_buf, &mut error);
                let mut prototk_field_wire_type: field_types::uint32 =
                    field_types::uint32::default();
                for (tag, buf) in fields {
                    let num: u32 = tag.field_number.into();
                    match (num, tag.wire_type) {
                        (1, field_types::uint32::WIRE_TYPE) => {
                            (prototk_field_wire_type, _) = Unpackable::unpack(buf)?;
                        }
                        (_, _) => {
                            return Err(Error::UnknownDiscriminant { discriminant: num });
                        }
                    }
                }
                let ret = Error::UnhandledWireType {
                    wire_type: prototk_field_wire_type.into(),
                };
                Ok((ret, up.remain()))
            }
            (262148, WireType::LengthDelimited) => {
                let length: v64 = up.unpack()?;
                let mut error: Option<Error> = None;
                let local_buf: &'b [u8] = &up.remain()[0..length.into()];
                up.advance(length.into());
                let fields = FieldIterator::new(local_buf, &mut error);
                let mut prototk_field_tag: field_types::uint64 = field_types::uint64::default();
                for (tag, buf) in fields {
                    let num: u32 = tag.field_number.into();
                    match (num, tag.wire_type) {
                        (1, field_types::uint64::WIRE_TYPE) => {
                            (prototk_field_tag, _) = Unpackable::unpack(buf)?;
                        }
                        (_, _) => {
                            return Err(Error::UnknownDiscriminant { discriminant: num });
                        }
                    }
                }
                let ret = Error::TagTooLarge {
                    tag: prototk_field_tag.into(),
                };
                Ok((ret, up.remain()))
            }
            (262149, WireType::LengthDelimited) => {
                let length: v64 = up.unpack()?;
                let mut error: Option<Error> = None;
                let local_buf: &'b [u8] = &up.remain()[0..length.into()];
                up.advance(length.into());
                let fields = FieldIterator::new(local_buf, &mut error);
                let mut prototk_field_bytes: field_types::uint64 = field_types::uint64::default();
                for (tag, buf) in fields {
                    let num: u32 = tag.field_number.into();
                    match (num, tag.wire_type) {
                        (1, field_types::uint64::WIRE_TYPE) => {
                            (prototk_field_bytes, _) = Unpackable::unpack(buf)?;
                        }
                        (_, _) => {
                            return Err(Error::UnknownDiscriminant { discriminant: num });
                        }
                    }
                }
                let ret = Error::VarintOverflow {
                    bytes: prototk_field_bytes.into(),
                };
                Ok((ret, up.remain()))
            }
            (262150, WireType::LengthDelimited) => {
                let length: v64 = up.unpack()?;
                let mut error: Option<Error> = None;
                let local_buf: &'b [u8] = &up.remain()[0..length.into()];
                up.advance(length.into());
                let fields = FieldIterator::new(local_buf, &mut error);
                let mut prototk_field_value: field_types::uint64 = field_types::uint64::default();
                for (tag, buf) in fields {
                    let num: u32 = tag.field_number.into();
                    match (num, tag.wire_type) {
                        (1, field_types::uint64::WIRE_TYPE) => {
                            (prototk_field_value, _) = Unpackable::unpack(buf)?;
                        }
                        (_, _) => {
                            return Err(Error::UnknownDiscriminant { discriminant: num });
                        }
                    }
                }
                let ret = Error::UnsignedOverflow {
                    value: prototk_field_value.into(),
                };
                Ok((ret, up.remain()))
            }
            (262151, WireType::LengthDelimited) => {
                let length: v64 = up.unpack()?;
                let mut error: Option<Error> = None;
                let local_buf: &'b [u8] = &up.remain()[0..length.into()];
                up.advance(length.into());
                let fields = FieldIterator::new(local_buf, &mut error);
                let mut prototk_field_value: field_types::int64 = field_types::int64::default();
                for (tag, buf) in fields {
                    let num: u32 = tag.field_number.into();
                    match (num, tag.wire_type) {
                        (1, field_types::int64::WIRE_TYPE) => {
                            (prototk_field_value, _) = Unpackable::unpack(buf)?;
                        }
                        (_, _) => {
                            return Err(Error::UnknownDiscriminant { discriminant: num });
                        }
                    }
                }
                let ret = Error::SignedOverflow {
                    value: prototk_field_value.into(),
                };
                Ok((ret, up.remain()))
            }
            (262152, WireType::LengthDelimited) => {
                let length: v64 = up.unpack()?;
                let mut error: Option<Error> = None;
                let local_buf: &'b [u8] = &up.remain()[0..length.into()];
                up.advance(length.into());
                let fields = FieldIterator::new(local_buf, &mut error);
                let mut prototk_field_required: field_types::uint64 =
                    field_types::uint64::default();
                let mut prototk_field_had: field_types::uint64 = field_types::uint64::default();
                for (tag, buf) in fields {
                    let num: u32 = tag.field_number.into();
                    match (num, tag.wire_type) {
                        (1, field_types::uint64::WIRE_TYPE) => {
                            (prototk_field_required, _) = Unpackable::unpack(buf)?;
                        }
                        (2, field_types::uint64::WIRE_TYPE) => {
                            (prototk_field_had, _) = Unpackable::unpack(buf)?;
                        }
                        (_, _) => {
                            return Err(Error::UnknownDiscriminant { discriminant: num });
                        }
                    }
                }
                let ret = Error::WrongLength {
                    required: prototk_field_required.into(),
                    had: prototk_field_had.into(),
                };
                Ok((ret, up.remain()))
            }
            (262153, WireType::LengthDelimited) => {
                let x: v64 = up.unpack()?;
                up.advance(x.into());
                Ok((Error::StringEncoding, up.remain()))
            }
            (262154, WireType::LengthDelimited) => {
                let length: v64 = up.unpack()?;
                let mut error: Option<Error> = None;
                let local_buf: &'b [u8] = &up.remain()[0..length.into()];
                up.advance(length.into());
                let fields = FieldIterator::new(local_buf, &mut error);
                let mut prototk_field_discriminant: field_types::uint32 =
                    field_types::uint32::default();
                for (tag, buf) in fields {
                    let num: u32 = tag.field_number.into();
                    match (num, tag.wire_type) {
                        (1, field_types::uint32::WIRE_TYPE) => {
                            (prototk_field_discriminant, _) = Unpackable::unpack(buf)?;
                        }
                        (_, _) => {
                            return Err(Error::UnknownDiscriminant { discriminant: num });
                        }
                    }
                }
                let ret = Error::UnknownDiscriminant {
                    discriminant: prototk_field_discriminant.into(),
                };
                Ok((ret, up.remain()))
            }
            (262155, WireType::LengthDelimited) => {
                let length: v64 = up.unpack()?;
                let mut error: Option<Error> = None;
                let local_buf: &'b [u8] = &up.remain()[0..length.into()];
                up.advance(length.into());
                let fields = FieldIterator::new(local_buf, &mut error);
                let mut prototk_field_value: field_types::uint32 = field_types::uint32::default();
                for (tag, buf) in fields {
                    let num: u32 = tag.field_number.into();
                    match (num, tag.wire_type) {
                        (1, field_types::uint32::WIRE_TYPE) => {
                            (prototk_field_value, _) = Unpackable::unpack(buf)?;
                        }
                        (_, _) => {
                            return Err(Error::UnknownDiscriminant { discriminant: num });
                        }
                    }
                }
                let ret = Error::NotAChar {
                    value: prototk_field_value.into(),
                };
                Ok((ret, up.remain()))
            }
            _ => Err(Error::UnknownDiscriminant { discriminant: num }),
        }
    }
}

impl<'a> FieldPackHelper<'a, field_types::message<Error>> for Error {
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

impl<'a> FieldUnpackHelper<'a, field_types::message<Error>> for Error {
    fn merge_field(&mut self, proto: field_types::message<Error>) {
        *self = proto.unwrap_message();
    }
}

impl From<field_types::message<Error>> for Error {
    fn from(proto: field_types::message<Error>) -> Self {
        proto.unwrap_message()
    }
}

impl<'a> Message<'a> for Error {}

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
    pub fn new(field_number: u32) -> Result<FieldNumber, Error> {
        if field_number < FIRST_FIELD_NUMBER {
            return Err(Error::InvalidFieldNumber {
                field_number,
                what: "field number must be positive integer".to_string(),
            });
        }
        if field_number > LAST_FIELD_NUMBER {
            return Err(Error::InvalidFieldNumber {
                field_number,
                what: "field number too large".to_string(),
            });
        }
        if (FIRST_RESERVED_FIELD_NUMBER..=LAST_RESERVED_FIELD_NUMBER).contains(&field_number) {
            return Err(Error::InvalidFieldNumber {
                field_number,
                what: "field is reserved".to_string(),
            });
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
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let mut up = Unpacker::new(buf);
        let tag: v64 = up.unpack()?;
        let tag: u64 = tag.into();
        if tag > u32::MAX as u64 {
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
        *self = Box::new(proto.into());
    }
}

impl<'a, T, E> FieldUnpackHelper<'a, field_types::message<Result<T, E>>> for Result<T, E> {
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

/// An iterator over tags and byte strings.
// TODO(rescrv): This panicked once and I didn't debug it.  Fix that.
pub struct FieldIterator<'a, 'b> {
    up: Unpacker<'a>,
    err: &'b mut Option<Error>,
}

impl<'a, 'b> FieldIterator<'a, 'b> {
    /// Create a new field iterator that will return tags and their byte strings.
    pub fn new(buf: &'a [u8], err: &'b mut Option<Error>) -> Self {
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

impl<'a, 'b> Iterator for FieldIterator<'a, 'b> {
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
                    *self.err = Some(Error::BufferTooShort {
                        required: 8,
                        had: buf.len(),
                    });
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
                    *self.err = Some(Error::BufferTooShort {
                        required: sz,
                        had: buf.len(),
                    });
                    return None;
                }
                self.up.advance(sz);
                Some((tag, &buf[0..x.pack_sz() + sz]))
            }
            WireType::ThirtyTwo => {
                let buf: &[u8] = self.up.remain();
                if buf.len() < 4 {
                    *self.err = Some(Error::BufferTooShort {
                        required: 4,
                        had: buf.len(),
                    });
                    return None;
                }
                self.up.advance(4);
                Some((tag, &buf[0..4]))
            }
        }
    }
}

////////////////////////////////////////////// Message /////////////////////////////////////////////

/// A protocol buffers messsage.
pub trait Message<'a>:
    Default
    + buffertk::Packable
    + buffertk::Unpackable<'a, Error = Error>
    + FieldPackHelper<'a, field_types::message<Self>>
    + FieldUnpackHelper<'a, field_types::message<Self>>
    + 'a
where
    <Self as Unpackable<'a>>::Error: Into<Error>,
    Error: From<Self::Error>,
{
}
