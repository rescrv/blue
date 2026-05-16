//! Tuple keys built from order-preserving, self-delimiting element encodings.
//!
//! Encoded keys preserve the natural order of tuples that use the same element
//! sequence. Mixed-type positions compare by their raw encoded bytes.
//!
//! The primary API is [`TupleKey::builder`]:
//!
//! ```
//! let key = tuple_key2::TupleKey::builder()
//!     .string("region")
//!     .u64(42)
//!     .i64(-7)
//!     .build();
//! ```
//!
//! Byte strings are encoded with Anchor & Escape framing: payload bytes are
//! copied as-is, `0x00` payload bytes become `0x00 0xff`, and `0x00 0x00`
//! terminates the element. Integers use compact, order-preserving prefix
//! varints. Small values such as `-1`, `0`, and unsigned `0` encode as a single
//! byte; full-width `i64`/`u64` values encode as one tag byte plus eight data
//! bytes.

#![deny(missing_docs)]

use std::error::Error as StdError;
use std::fmt;
use std::ops::Deref;
use std::result::Result;

const SIGNED_NEG_BASE: u8 = 0x10;
const SIGNED_NEG_LAST: u8 = 0x18;
const SIGNED_NONNEG_BASE: u8 = 0x19;
const SIGNED_NONNEG_LAST: u8 = 0x21;
const UNSIGNED_BASE: u8 = 0x22;
const UNSIGNED_LAST: u8 = 0x2a;
const UNIT_TAG: u8 = 0x2b;

const INTEGER_TAG_MIN: u8 = SIGNED_NEG_BASE;
const INTEGER_TAG_MAX: u8 = UNIT_TAG;

const ONES: u64 = 0x0101_0101_0101_0101;
const HIGHS: u64 = 0x8080_8080_8080_8080;
const NIBBLE_MASK: u64 = 0xf0f0_f0f0_f0f0_f0f0;
const TAG_NIBBLE_1: u64 = 0x1010_1010_1010_1010;
const TAG_NIBBLE_2: u64 = 0x2020_2020_2020_2020;

/// Errors returned while decoding a tuple key with an expected type sequence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    /// The next expected element was absent or truncated.
    UnexpectedEnd,
    /// The parser expected an integer tag, but found another byte.
    InvalidIntegerTag {
        /// The byte encountered where an integer tag was required.
        tag: u8,
    },
    /// The parser expected a unit element, but found another byte.
    InvalidUnitTag {
        /// The byte encountered where the unit tag was required.
        tag: u8,
    },
    /// An integer used a longer representation than necessary.
    NonCanonicalInteger,
    /// The encoded integer does not fit the requested Rust type.
    ValueOutOfRange {
        /// The Rust integer type requested by the parser.
        target: &'static str,
    },
    /// A byte string contained `0x00` followed by neither `0x00` nor `0xff`.
    InvalidBytesEscape {
        /// The byte following `0x00` in the invalid escape sequence.
        byte: u8,
    },
    /// A byte string did not contain a `0x00 0x00` terminator.
    UnterminatedBytes,
    /// A string element was not valid UTF-8.
    InvalidUtf8,
    /// The parser reached the end of the expected sequence with bytes left over.
    TrailingBytes {
        /// The number of bytes that remained after the expected parse.
        remaining: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEnd => write!(f, "unexpected end of tuple key"),
            Self::InvalidIntegerTag { tag } => write!(f, "invalid integer tag: 0x{tag:02x}"),
            Self::InvalidUnitTag { tag } => write!(f, "invalid unit tag: 0x{tag:02x}"),
            Self::NonCanonicalInteger => write!(f, "non-canonical integer encoding"),
            Self::ValueOutOfRange { target } => write!(f, "value out of range for {target}"),
            Self::InvalidBytesEscape { byte } => {
                write!(f, "invalid byte string escape: 0x00 0x{byte:02x}")
            }
            Self::UnterminatedBytes => write!(f, "unterminated byte string"),
            Self::InvalidUtf8 => write!(f, "invalid UTF-8 string"),
            Self::TrailingBytes { remaining } => {
                write!(f, "{remaining} trailing bytes after tuple key parse")
            }
        }
    }
}

impl StdError for Error {}

/// A byte string that sorts like the same-shape tuple it encodes.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TupleKey {
    bytes: Vec<u8>,
}

impl TupleKey {
    /// Start a tuple as an empty sequence of encoded elements.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let key = tuple_key2::TupleKey::builder()
    ///     .string("table")
    ///     .u64(7)
    ///     .build();
    ///
    /// let mut parser = key.parser();
    /// assert_eq!("table", parser.string()?);
    /// assert_eq!(7, parser.u64()?);
    /// parser.finish()?;
    /// # Ok::<(), tuple_key2::Error>(())
    /// ```
    pub fn builder() -> TupleKeyBuilder {
        TupleKeyBuilder::new()
    }

    /// Start a tuple with storage reserved for at least `capacity` encoded bytes.
    pub fn builder_with_capacity(capacity: usize) -> TupleKeyBuilder {
        TupleKeyBuilder::with_capacity(capacity)
    }

    /// Wrap encoded bytes.
    ///
    /// This is intentionally unchecked. Byte string elements require an
    /// expected type sequence to parse, and arbitrary byte slices can be useful
    /// when keys are read back from storage.
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    /// Expose the encoded byte string that storage engines compare directly.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Consume the tuple key and recover its byte string.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Return the encoded length in bytes.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Return true if the encoded key has no elements.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Append another encoded tuple without changing either element sequence.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let mut prefix = tuple_key2::TupleKey::builder().string("a").build();
    /// let suffix = tuple_key2::TupleKey::builder().u64(1).build();
    /// let whole = tuple_key2::TupleKey::builder().string("a").u64(1).build();
    ///
    /// prefix.append(&suffix);
    /// assert_eq!(whole.as_bytes(), prefix.as_bytes());
    /// ```
    pub fn append(&mut self, other: &TupleKey) {
        self.bytes.extend_from_slice(other.as_bytes());
    }

    /// Interpret this byte string through a caller-supplied element sequence.
    pub fn parser(&self) -> TupleKeyParser<'_> {
        TupleKeyParser::new(self.as_bytes())
    }

    /// Return probabilistic boundary candidates in this encoded key.
    pub fn boundary_candidates(&self) -> Vec<BoundaryCandidate> {
        boundary_candidates(self.as_bytes())
    }
}

impl AsRef<[u8]> for TupleKey {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl Deref for TupleKey {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_bytes()
    }
}

impl From<TupleKeyBuilder> for TupleKey {
    fn from(builder: TupleKeyBuilder) -> Self {
        builder.build()
    }
}

impl From<Vec<u8>> for TupleKey {
    fn from(bytes: Vec<u8>) -> Self {
        Self::from_bytes(bytes)
    }
}

/// Fluent builder for constructing tuple keys.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TupleKeyBuilder {
    bytes: Vec<u8>,
}

impl TupleKeyBuilder {
    /// Create the empty prefix of a tuple key.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an empty prefix with storage reserved for encoded bytes.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(capacity),
        }
    }

    /// Observe the encoded bytes accumulated by this unfinished tuple.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Extend this tuple with another tuple's complete element sequence.
    pub fn tuple_key(mut self, key: &TupleKey) -> Self {
        self.bytes.extend_from_slice(key.as_bytes());
        self
    }

    /// Extend this tuple with another tuple's complete element sequence.
    pub fn extend(self, key: &TupleKey) -> Self {
        self.tuple_key(key)
    }

    /// Append the singleton unit value.
    pub fn unit(mut self) -> Self {
        self.bytes.push(UNIT_TAG);
        self
    }

    /// Append arbitrary bytes with order-preserving escape framing.
    pub fn bytes(mut self, bytes: impl AsRef<[u8]>) -> Self {
        encode_bytes(bytes.as_ref(), &mut self.bytes);
        self
    }

    /// Append a UTF-8 string as its underlying ordered byte sequence.
    pub fn string(self, string: impl AsRef<str>) -> Self {
        self.bytes(string.as_ref().as_bytes())
    }

    /// Append an unsigned byte using the compact unsigned integer family.
    pub fn u8(self, value: u8) -> Self {
        self.u64(value as u64)
    }

    /// Append an unsigned 16-bit integer using the compact unsigned integer family.
    pub fn u16(self, value: u16) -> Self {
        self.u64(value as u64)
    }

    /// Append an unsigned 32-bit integer using the compact unsigned integer family.
    pub fn u32(self, value: u32) -> Self {
        self.u64(value as u64)
    }

    /// Append an unsigned 64-bit integer in its shortest ordered representation.
    pub fn u64(mut self, value: u64) -> Self {
        encode_u64(value, &mut self.bytes);
        self
    }

    /// Append a signed byte using the compact signed integer family.
    pub fn i8(self, value: i8) -> Self {
        self.i64(value as i64)
    }

    /// Append a signed 16-bit integer using the compact signed integer family.
    pub fn i16(self, value: i16) -> Self {
        self.i64(value as i64)
    }

    /// Append a signed 32-bit integer using the compact signed integer family.
    pub fn i32(self, value: i32) -> Self {
        self.i64(value as i64)
    }

    /// Append a signed 64-bit integer in its shortest ordered representation.
    pub fn i64(mut self, value: i64) -> Self {
        encode_i64(value, &mut self.bytes);
        self
    }

    /// End construction and return the encoded tuple key.
    pub fn build(self) -> TupleKey {
        TupleKey { bytes: self.bytes }
    }

    /// End construction and return the encoded tuple key.
    pub fn finish(self) -> TupleKey {
        self.build()
    }
}

/// Carries an expected element sequence across an encoded tuple key.
#[derive(Clone, Debug)]
pub struct TupleKeyParser<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> TupleKeyParser<'a> {
    /// Bind a parser to encoded bytes without validating them eagerly.
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    /// Return the byte position of the next expected element.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Return the suffix not yet claimed by typed parser calls.
    pub fn remaining(&self) -> &'a [u8] {
        &self.bytes[self.offset..]
    }

    /// Report whether every encoded byte has been parsed.
    pub fn is_empty(&self) -> bool {
        self.remaining().is_empty()
    }

    /// Require that the expected element sequence consumed the full key.
    ///
    /// # Errors
    ///
    /// Returns [`Error::TrailingBytes`] when bytes remain after the caller's
    /// expected sequence is exhausted.
    pub fn finish(self) -> Result<(), Error> {
        if self.is_empty() {
            Ok(())
        } else {
            Err(Error::TrailingBytes {
                remaining: self.remaining().len(),
            })
        }
    }

    /// Parse the singleton unit value.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnexpectedEnd`] when no byte remains and
    /// [`Error::InvalidUnitTag`] when the next byte is not the unit tag.
    pub fn unit(&mut self) -> Result<(), Error> {
        let tag = self.take_one()?;
        if tag == UNIT_TAG {
            Ok(())
        } else {
            Err(Error::InvalidUnitTag { tag })
        }
    }

    /// Parse arbitrary bytes from Anchor & Escape framing.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnterminatedBytes`] for a missing `0x00 0x00`
    /// terminator and [`Error::InvalidBytesEscape`] for a malformed
    /// `0x00` escape pair.
    pub fn bytes(&mut self) -> Result<Vec<u8>, Error> {
        let mut decoded = Vec::new();
        while self.offset < self.bytes.len() {
            let byte = self.bytes[self.offset];
            self.offset += 1;
            if byte == 0x00 {
                let escape = *self
                    .bytes
                    .get(self.offset)
                    .ok_or(Error::UnterminatedBytes)?;
                self.offset += 1;
                match escape {
                    0x00 => return Ok(decoded),
                    0xff => decoded.push(0x00),
                    byte => return Err(Error::InvalidBytesEscape { byte }),
                }
            } else {
                decoded.push(byte);
            }
        }
        Err(Error::UnterminatedBytes)
    }

    /// Parse a UTF-8 string from Anchor & Escape framing.
    ///
    /// # Errors
    ///
    /// Returns the same byte-framing errors as [`TupleKeyParser::bytes`] and
    /// [`Error::InvalidUtf8`] when the decoded bytes are not a string.
    pub fn string(&mut self) -> Result<String, Error> {
        String::from_utf8(self.bytes()?).map_err(|_| Error::InvalidUtf8)
    }

    /// Parse a compact unsigned integer as `u64`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnexpectedEnd`] for truncation,
    /// [`Error::InvalidIntegerTag`] when the next byte is not an unsigned
    /// integer tag, and [`Error::NonCanonicalInteger`] when the encoded value
    /// is not shortest-form.
    pub fn u64(&mut self) -> Result<u64, Error> {
        let tag = self.take_one()?;
        let len = match unsigned_len(tag) {
            Some(len) => len,
            None => return Err(Error::InvalidIntegerTag { tag }),
        };
        let payload = self.take_payload(len)?;
        decode_u64_payload(payload)
    }

    /// Parse a compact unsigned integer as `u32`.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`TupleKeyParser::u64`] and
    /// [`Error::ValueOutOfRange`] when the decoded value exceeds `u32::MAX`.
    pub fn u32(&mut self) -> Result<u32, Error> {
        self.parse_u64_as("u32")
    }

    /// Parse a compact unsigned integer as `u16`.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`TupleKeyParser::u64`] and
    /// [`Error::ValueOutOfRange`] when the decoded value exceeds `u16::MAX`.
    pub fn u16(&mut self) -> Result<u16, Error> {
        self.parse_u64_as("u16")
    }

    /// Parse a compact unsigned integer as `u8`.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`TupleKeyParser::u64`] and
    /// [`Error::ValueOutOfRange`] when the decoded value exceeds `u8::MAX`.
    pub fn u8(&mut self) -> Result<u8, Error> {
        self.parse_u64_as("u8")
    }

    /// Parse a compact signed integer as `i64`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnexpectedEnd`] for truncation,
    /// [`Error::InvalidIntegerTag`] when the next byte is not a signed integer
    /// tag, [`Error::NonCanonicalInteger`] when the encoded value is not
    /// shortest-form, and [`Error::ValueOutOfRange`] for impossible signed
    /// encodings outside `i64`.
    pub fn i64(&mut self) -> Result<i64, Error> {
        let tag = self.take_one()?;
        if let Some(len) = signed_negative_len(tag) {
            let payload = self.take_payload(len)?;
            decode_negative_i64_payload(payload)
        } else if let Some(len) = signed_nonnegative_len(tag) {
            let payload = self.take_payload(len)?;
            decode_nonnegative_i64_payload(payload)
        } else {
            Err(Error::InvalidIntegerTag { tag })
        }
    }

    /// Parse a compact signed integer as `i32`.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`TupleKeyParser::i64`] and
    /// [`Error::ValueOutOfRange`] when the decoded value is outside `i32`.
    pub fn i32(&mut self) -> Result<i32, Error> {
        self.parse_i64_as("i32")
    }

    /// Parse a compact signed integer as `i16`.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`TupleKeyParser::i64`] and
    /// [`Error::ValueOutOfRange`] when the decoded value is outside `i16`.
    pub fn i16(&mut self) -> Result<i16, Error> {
        self.parse_i64_as("i16")
    }

    /// Parse a compact signed integer as `i8`.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`TupleKeyParser::i64`] and
    /// [`Error::ValueOutOfRange`] when the decoded value is outside `i8`.
    pub fn i8(&mut self) -> Result<i8, Error> {
        self.parse_i64_as("i8")
    }

    fn parse_u64_as<T>(&mut self, target: &'static str) -> Result<T, Error>
    where
        T: TryFrom<u64>,
    {
        T::try_from(self.u64()?).map_err(|_| Error::ValueOutOfRange { target })
    }

    fn parse_i64_as<T>(&mut self, target: &'static str) -> Result<T, Error>
    where
        T: TryFrom<i64>,
    {
        T::try_from(self.i64()?).map_err(|_| Error::ValueOutOfRange { target })
    }

    fn take_one(&mut self) -> Result<u8, Error> {
        let byte = *self.bytes.get(self.offset).ok_or(Error::UnexpectedEnd)?;
        self.offset += 1;
        Ok(byte)
    }

    fn take_payload(&mut self, len: usize) -> Result<&'a [u8], Error> {
        let limit = self
            .offset
            .checked_add(len)
            .filter(|limit| *limit <= self.bytes.len())
            .ok_or(Error::UnexpectedEnd)?;
        let payload = &self.bytes[self.offset..limit];
        self.offset = limit;
        Ok(payload)
    }
}

/// Kind of probabilistic boundary candidate found in encoded bytes.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BoundaryKind {
    /// Offset just after a `0x00 0x00` byte-string terminator.
    BytesTerminator,
    /// Offset of a byte that looks like a compact integer or unit tag.
    Tag,
}

/// A candidate element boundary.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct BoundaryCandidate {
    /// The candidate byte offset.
    pub offset: usize,
    /// Why the offset is a candidate.
    pub kind: BoundaryKind,
}

/// Return true if `byte` is a compact integer tag.
pub fn is_integer_tag(byte: u8) -> bool {
    (SIGNED_NEG_BASE..=SIGNED_NONNEG_LAST).contains(&byte)
        || (UNSIGNED_BASE..=UNSIGNED_LAST).contains(&byte)
}

/// Return true if `byte` is the unit element tag.
pub fn is_unit_tag(byte: u8) -> bool {
    byte == UNIT_TAG
}

/// Return true if `byte` can begin a tagged element.
pub fn is_tag(byte: u8) -> bool {
    (INTEGER_TAG_MIN..=INTEGER_TAG_MAX).contains(&byte)
}

/// Return a high-bit mask for zero bytes in `word`.
pub fn zero_byte_mask(word: u64) -> u64 {
    word.wrapping_sub(ONES) & !word & HIGHS
}

/// Return a high-bit mask for bytes with high nibble `0x1` or `0x2`.
///
/// All tuple_key2 tagged elements currently live in `0x10..=0x2b`; this mask
/// intentionally treats `0x2c..=0x2f` as candidates so callers can use one
/// cheap SWAR predicate and then verify exact tags.
pub fn broad_tag_candidate_mask(word: u64) -> u64 {
    let high_nibbles = word & NIBBLE_MASK;
    byte_eq_mask(high_nibbles, TAG_NIBBLE_1) | byte_eq_mask(high_nibbles, TAG_NIBBLE_2)
}

/// Return probabilistic element boundary candidates in `bytes`.
pub fn boundary_candidates(bytes: &[u8]) -> Vec<BoundaryCandidate> {
    let mut candidates = Vec::new();
    let mut offset = 0;
    while offset < bytes.len() {
        let byte = bytes[offset];
        if byte == 0x00 && bytes.get(offset + 1) == Some(&0x00) {
            candidates.push(BoundaryCandidate {
                offset: offset + 2,
                kind: BoundaryKind::BytesTerminator,
            });
            offset += 2;
            continue;
        }
        if is_tag(byte) {
            candidates.push(BoundaryCandidate {
                offset,
                kind: BoundaryKind::Tag,
            });
        }
        offset += 1;
    }
    candidates
}

fn byte_eq_mask(word: u64, repeated_byte: u64) -> u64 {
    zero_byte_mask(word ^ repeated_byte)
}

fn encode_bytes(bytes: &[u8], out: &mut Vec<u8>) {
    for byte in bytes {
        out.push(*byte);
        if *byte == 0x00 {
            out.push(0xff);
        }
    }
    out.push(0x00);
    out.push(0x00);
}

fn encode_u64(value: u64, out: &mut Vec<u8>) {
    let len = minimal_u64_len(value);
    out.push(UNSIGNED_BASE + len as u8);
    push_big_endian_suffix(value, len, out);
}

fn encode_i64(value: i64, out: &mut Vec<u8>) {
    if value < 0 {
        let magnitude = !(value as u64);
        let len = minimal_u64_len(magnitude);
        out.push(SIGNED_NEG_BASE + (8 - len) as u8);
        if len > 0 {
            let inverted = !magnitude;
            push_big_endian_suffix(inverted, len, out);
        }
    } else {
        let value = value as u64;
        let len = minimal_u64_len(value);
        out.push(SIGNED_NONNEG_BASE + len as u8);
        push_big_endian_suffix(value, len, out);
    }
}

fn push_big_endian_suffix(value: u64, len: usize, out: &mut Vec<u8>) {
    debug_assert!(len <= 8);
    let bytes = value.to_be_bytes();
    out.extend_from_slice(&bytes[8 - len..]);
}

fn minimal_u64_len(value: u64) -> usize {
    if value == 0 {
        0
    } else {
        value.ilog2() as usize / 8 + 1
    }
}

fn unsigned_len(tag: u8) -> Option<usize> {
    if (UNSIGNED_BASE..=UNSIGNED_LAST).contains(&tag) {
        Some((tag - UNSIGNED_BASE) as usize)
    } else {
        None
    }
}

fn signed_negative_len(tag: u8) -> Option<usize> {
    if (SIGNED_NEG_BASE..=SIGNED_NEG_LAST).contains(&tag) {
        Some(8 - (tag - SIGNED_NEG_BASE) as usize)
    } else {
        None
    }
}

fn signed_nonnegative_len(tag: u8) -> Option<usize> {
    if (SIGNED_NONNEG_BASE..=SIGNED_NONNEG_LAST).contains(&tag) {
        Some((tag - SIGNED_NONNEG_BASE) as usize)
    } else {
        None
    }
}

fn decode_u64_payload(payload: &[u8]) -> Result<u64, Error> {
    let value = decode_big_endian_payload(payload);
    if minimal_u64_len(value) == payload.len() {
        Ok(value)
    } else {
        Err(Error::NonCanonicalInteger)
    }
}

fn decode_nonnegative_i64_payload(payload: &[u8]) -> Result<i64, Error> {
    let value = decode_u64_payload(payload)?;
    i64::try_from(value).map_err(|_| Error::ValueOutOfRange { target: "i64" })
}

fn decode_negative_i64_payload(payload: &[u8]) -> Result<i64, Error> {
    let magnitude = if payload.is_empty() {
        0
    } else {
        let encoded = decode_big_endian_payload(payload);
        let mask = if payload.len() == 8 {
            u64::MAX
        } else {
            (1u64 << (payload.len() * 8)) - 1
        };
        !encoded & mask
    };
    if minimal_u64_len(magnitude) != payload.len() {
        return Err(Error::NonCanonicalInteger);
    }
    if magnitude > i64::MAX as u64 {
        return Err(Error::ValueOutOfRange { target: "i64" });
    }
    Ok((!magnitude) as i64)
}

fn decode_big_endian_payload(payload: &[u8]) -> u64 {
    debug_assert!(payload.len() <= 8);
    let mut bytes = [0u8; 8];
    bytes[8 - payload.len()..].copy_from_slice(payload);
    u64::from_be_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[derive(Clone, Debug, Eq, PartialEq)]
    enum TestElement {
        Unit,
        Bytes(Vec<u8>),
        String(String),
        U8(u8),
        U16(u16),
        U32(u32),
        U64(u64),
        I8(i8),
        I16(i16),
        I32(i32),
        I64(i64),
    }

    impl TestElement {
        fn append_to(&self, builder: TupleKeyBuilder) -> TupleKeyBuilder {
            match self {
                Self::Unit => builder.unit(),
                Self::Bytes(value) => builder.bytes(value),
                Self::String(value) => builder.string(value),
                Self::U8(value) => builder.u8(*value),
                Self::U16(value) => builder.u16(*value),
                Self::U32(value) => builder.u32(*value),
                Self::U64(value) => builder.u64(*value),
                Self::I8(value) => builder.i8(*value),
                Self::I16(value) => builder.i16(*value),
                Self::I32(value) => builder.i32(*value),
                Self::I64(value) => builder.i64(*value),
            }
        }

        fn parse_from(&self, parser: &mut TupleKeyParser<'_>) -> Result<Self, Error> {
            match self {
                Self::Unit => {
                    parser.unit()?;
                    Ok(Self::Unit)
                }
                Self::Bytes(_) => parser.bytes().map(Self::Bytes),
                Self::String(_) => parser.string().map(Self::String),
                Self::U8(_) => parser.u8().map(Self::U8),
                Self::U16(_) => parser.u16().map(Self::U16),
                Self::U32(_) => parser.u32().map(Self::U32),
                Self::U64(_) => parser.u64().map(Self::U64),
                Self::I8(_) => parser.i8().map(Self::I8),
                Self::I16(_) => parser.i16().map(Self::I16),
                Self::I32(_) => parser.i32().map(Self::I32),
                Self::I64(_) => parser.i64().map(Self::I64),
            }
        }
    }

    fn encode_elements(elements: &[TestElement]) -> TupleKey {
        elements
            .iter()
            .fold(TupleKey::builder(), |builder, element| {
                element.append_to(builder)
            })
            .build()
    }

    fn decode_elements(
        elements: &[TestElement],
        key: &TupleKey,
    ) -> Result<Vec<TestElement>, Error> {
        let mut parser = key.parser();
        let decoded = elements
            .iter()
            .map(|element| element.parse_from(&mut parser))
            .collect::<Result<Vec<_>, Error>>()?;
        parser.finish()?;
        Ok(decoded)
    }

    fn arb_string() -> impl Strategy<Value = String> {
        proptest::collection::vec(any::<char>(), 0..32)
            .prop_map(|chars| chars.into_iter().collect())
    }

    fn arb_element() -> impl Strategy<Value = TestElement> {
        prop_oneof![
            Just(TestElement::Unit),
            proptest::collection::vec(any::<u8>(), 0..64).prop_map(TestElement::Bytes),
            arb_string().prop_map(TestElement::String),
            any::<u8>().prop_map(TestElement::U8),
            any::<u16>().prop_map(TestElement::U16),
            any::<u32>().prop_map(TestElement::U32),
            any::<u64>().prop_map(TestElement::U64),
            any::<i8>().prop_map(TestElement::I8),
            any::<i16>().prop_map(TestElement::I16),
            any::<i32>().prop_map(TestElement::I32),
            any::<i64>().prop_map(TestElement::I64),
        ]
    }

    proptest! {
        #[test]
        fn arbitrary_tuples_encode_decode_and_concat(
            elements in proptest::collection::vec(arb_element(), 1..17)
        ) {
            let key = encode_elements(&elements);
            prop_assert_eq!(Ok(elements.clone()), decode_elements(&elements, &key));

            for split in 0..=elements.len() {
                let prefix = encode_elements(&elements[..split]);
                let suffix = encode_elements(&elements[split..]);

                let mut appended = prefix.clone();
                appended.append(&suffix);
                prop_assert_eq!(key.as_bytes(), appended.as_bytes());
                prop_assert_eq!(Ok(elements.clone()), decode_elements(&elements, &appended));

                let built_from_keys = TupleKey::builder()
                    .tuple_key(&prefix)
                    .tuple_key(&suffix)
                    .build();
                prop_assert_eq!(key.as_bytes(), built_from_keys.as_bytes());
                prop_assert_eq!(Ok(elements.clone()), decode_elements(&elements, &built_from_keys));
            }
        }
    }

    fn encoded_i64(value: i64) -> Vec<u8> {
        TupleKey::builder().i64(value).build().into_bytes()
    }

    fn encoded_u64(value: u64) -> Vec<u8> {
        TupleKey::builder().u64(value).build().into_bytes()
    }

    fn encoded_bytes(value: &[u8]) -> Vec<u8> {
        TupleKey::builder().bytes(value).build().into_bytes()
    }

    #[test]
    fn fluent_builder_mixes_element_types() {
        let key = TupleKey::builder()
            .string("a")
            .u64(42)
            .i64(-7)
            .bytes([0x00, 0x01])
            .unit()
            .build();

        assert_eq!(
            &[
                0x61, 0x00, 0x00, 0x23, 0x2a, 0x17, 0xf9, 0x00, 0xff, 0x01, 0x00, 0x00, 0x2b
            ],
            key.as_bytes()
        );

        let mut parser = key.parser();
        assert_eq!("a", parser.string().unwrap());
        assert_eq!(42, parser.u64().unwrap());
        assert_eq!(-7, parser.i64().unwrap());
        assert_eq!(vec![0x00, 0x01], parser.bytes().unwrap());
        assert_eq!((), parser.unit().unwrap());
        assert_eq!((), parser.finish().unwrap());
    }

    #[test]
    fn unsigned_varints_pack_small_values() {
        assert_eq!(vec![0x22], encoded_u64(0));
        assert_eq!(vec![0x23, 0x01], encoded_u64(1));
        assert_eq!(vec![0x23, 0xff], encoded_u64(255));
        assert_eq!(vec![0x24, 0x01, 0x00], encoded_u64(256));
        assert_eq!(
            vec![0x2a, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
            encoded_u64(u64::MAX)
        );
    }

    #[test]
    fn signed_varints_pack_small_values() {
        assert_eq!(vec![0x18], encoded_i64(-1));
        assert_eq!(vec![0x17, 0xfe], encoded_i64(-2));
        assert_eq!(vec![0x19], encoded_i64(0));
        assert_eq!(vec![0x1a, 0x01], encoded_i64(1));
        assert_eq!(vec![0x1a, 0xff], encoded_i64(255));
        assert_eq!(vec![0x1b, 0x01, 0x00], encoded_i64(256));
        assert_eq!(
            vec![0x10, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            encoded_i64(i64::MIN)
        );
        assert_eq!(
            vec![0x21, 0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
            encoded_i64(i64::MAX)
        );
    }

    #[test]
    fn parser_round_trips_integer_edges() {
        let key = TupleKey::builder()
            .u8(u8::MAX)
            .u16(u16::MAX)
            .u32(u32::MAX)
            .u64(u64::MAX)
            .i8(i8::MIN)
            .i16(i16::MIN)
            .i32(i32::MIN)
            .i64(i64::MIN)
            .build();

        let mut parser = key.parser();
        assert_eq!(u8::MAX, parser.u8().unwrap());
        assert_eq!(u16::MAX, parser.u16().unwrap());
        assert_eq!(u32::MAX, parser.u32().unwrap());
        assert_eq!(u64::MAX, parser.u64().unwrap());
        assert_eq!(i8::MIN, parser.i8().unwrap());
        assert_eq!(i16::MIN, parser.i16().unwrap());
        assert_eq!(i32::MIN, parser.i32().unwrap());
        assert_eq!(i64::MIN, parser.i64().unwrap());
        assert_eq!((), parser.finish().unwrap());
    }

    #[test]
    fn bytes_escape_nulls_and_terminate_with_null() {
        let key = TupleKey::builder()
            .bytes([])
            .bytes([0x00])
            .bytes([0x00, 0x01, 0x00])
            .build();

        assert_eq!(
            &[
                0x00, 0x00, 0x00, 0xff, 0x00, 0x00, 0x00, 0xff, 0x01, 0x00, 0xff, 0x00, 0x00
            ],
            key.as_bytes()
        );

        let mut parser = key.parser();
        assert_eq!(Vec::<u8>::new(), parser.bytes().unwrap());
        assert_eq!(vec![0x00], parser.bytes().unwrap());
        assert_eq!(vec![0x00, 0x01, 0x00], parser.bytes().unwrap());
        assert_eq!((), parser.finish().unwrap());
    }

    #[test]
    fn empty_bytes_before_ff_payload_is_unambiguous() {
        let key = TupleKey::builder().bytes([]).bytes([0xff]).build();

        assert_eq!(&[0x00, 0x00, 0xff, 0x00, 0x00], key.as_bytes());

        let mut parser = key.parser();
        assert_eq!(Vec::<u8>::new(), parser.bytes().unwrap());
        assert_eq!(vec![0xff], parser.bytes().unwrap());
        assert_eq!((), parser.finish().unwrap());
    }

    #[test]
    fn strings_round_trip_utf8() {
        let key = TupleKey::builder().string("").string("hello").build();
        let mut parser = key.parser();
        assert_eq!("", parser.string().unwrap());
        assert_eq!("hello", parser.string().unwrap());
        assert_eq!((), parser.finish().unwrap());
    }

    #[test]
    fn byte_string_encoding_preserves_lexicographic_order() {
        let values: Vec<&[u8]> = vec![
            b"",
            &[0x00],
            &[0x00, 0x00],
            &[0x00, 0x01],
            &[0x01],
            b"a",
            &[b'a', 0x00],
            &[b'a', 0x01],
            b"b",
        ];
        let mut encoded = values
            .iter()
            .map(|value| (*value, encoded_bytes(value)))
            .collect::<Vec<_>>();
        encoded.sort_by(|lhs, rhs| lhs.1.cmp(&rhs.1));

        let got = encoded
            .into_iter()
            .map(|(value, _)| value)
            .collect::<Vec<_>>();
        assert_eq!(values, got);
    }

    #[test]
    fn unsigned_encoding_preserves_numeric_order() {
        let values = vec![0, 1, 2, 255, 256, 65_535, 65_536, u32::MAX as u64, u64::MAX];
        let mut encoded = values
            .iter()
            .map(|value| (*value, encoded_u64(*value)))
            .collect::<Vec<_>>();
        encoded.sort_by(|lhs, rhs| lhs.1.cmp(&rhs.1));

        let got = encoded
            .into_iter()
            .map(|(value, _)| value)
            .collect::<Vec<_>>();
        assert_eq!(values, got);
    }

    #[test]
    fn signed_encoding_preserves_numeric_order() {
        let values = vec![
            i64::MIN,
            -65_537,
            -65_536,
            -257,
            -256,
            -2,
            -1,
            0,
            1,
            255,
            256,
            65_535,
            65_536,
            i64::MAX,
        ];
        let mut encoded = values
            .iter()
            .map(|value| (*value, encoded_i64(*value)))
            .collect::<Vec<_>>();
        encoded.sort_by(|lhs, rhs| lhs.1.cmp(&rhs.1));

        let got = encoded
            .into_iter()
            .map(|(value, _)| value)
            .collect::<Vec<_>>();
        assert_eq!(values, got);
    }

    #[test]
    fn tuple_encoding_preserves_lexicographic_order_for_same_shape_tuples() {
        let values = vec![("a", 1u64), ("a", 2u64), ("a", 256u64), ("b", 0u64)];
        let mut encoded = values
            .iter()
            .map(|(name, number)| {
                (
                    (*name, *number),
                    TupleKey::builder().string(*name).u64(*number).build(),
                )
            })
            .collect::<Vec<_>>();
        encoded.sort_by(|lhs, rhs| lhs.1.cmp(&rhs.1));

        let got = encoded
            .into_iter()
            .map(|(value, _)| value)
            .collect::<Vec<_>>();
        assert_eq!(values, got);
    }

    #[test]
    fn parser_rejects_non_canonical_integer_encodings() {
        let key = TupleKey::from_bytes(vec![0x23, 0x00]);
        let mut parser = key.parser();
        assert_eq!(Err(Error::NonCanonicalInteger), parser.u64());

        let key = TupleKey::from_bytes(vec![0x17, 0xff]);
        let mut parser = key.parser();
        assert_eq!(Err(Error::NonCanonicalInteger), parser.i64());
    }

    #[test]
    fn parser_reports_truncation_and_trailing_bytes() {
        let key = TupleKey::from_bytes(vec![0x24, 0x01]);
        let mut parser = key.parser();
        assert_eq!(Err(Error::UnexpectedEnd), parser.u64());

        let key = TupleKey::builder().u64(1).unit().build();
        let mut parser = key.parser();
        assert_eq!(1, parser.u64().unwrap());
        assert_eq!(Err(Error::TrailingBytes { remaining: 1 }), parser.finish());
    }

    #[test]
    fn parser_rejects_invalid_utf8_strings() {
        let key = TupleKey::builder().bytes([0xff]).build();
        let mut parser = key.parser();
        assert_eq!(Err(Error::InvalidUtf8), parser.string());
    }

    #[test]
    fn parser_rejects_invalid_byte_escape() {
        let key = TupleKey::from_bytes(vec![0x00, 0x01]);
        let mut parser = key.parser();
        assert_eq!(
            Err(Error::InvalidBytesEscape { byte: 0x01 }),
            parser.bytes()
        );
    }

    #[test]
    fn boundary_candidates_include_terminators_and_tags() {
        let key = TupleKey::builder().string("a").u64(42).unit().build();

        assert_eq!(
            vec![
                BoundaryCandidate {
                    offset: 3,
                    kind: BoundaryKind::BytesTerminator,
                },
                BoundaryCandidate {
                    offset: 3,
                    kind: BoundaryKind::Tag,
                },
                BoundaryCandidate {
                    offset: 4,
                    kind: BoundaryKind::Tag,
                },
                BoundaryCandidate {
                    offset: 5,
                    kind: BoundaryKind::Tag,
                },
            ],
            key.boundary_candidates()
        );
    }

    #[test]
    fn swar_masks_find_zeroes_and_broad_tags() {
        let word = u64::from_le_bytes([0x00, 0x11, 0x2b, 0x30, 0xff, 0x00, 0x19, 0x7f]);

        assert_eq!(
            u64::from_le_bytes([0x80, 0x00, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00]),
            zero_byte_mask(word)
        );
        assert_eq!(
            u64::from_le_bytes([0x00, 0x80, 0x80, 0x00, 0x00, 0x00, 0x80, 0x00]),
            broad_tag_candidate_mask(word)
        );
    }

    #[test]
    fn appending_keys_preserves_concatenability() {
        let mut lhs = TupleKey::builder().string("left").build();
        let rhs = TupleKey::builder().u64(7).string("right").build();
        lhs.append(&rhs);

        let mut parser = lhs.parser();
        assert_eq!("left", parser.string().unwrap());
        assert_eq!(7, parser.u64().unwrap());
        assert_eq!("right", parser.string().unwrap());
        assert_eq!((), parser.finish().unwrap());
    }
}
