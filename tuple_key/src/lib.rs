#![doc = include_str!("../README.md")]

use std::collections::HashMap;
use std::fmt::Debug;

use buffertk::{v64, Packable};

use buffertk::Unpackable;
use prototk::{FieldNumber, WireType};
use prototk_derive::Message;
use zerror::{iotoz, Z};
use zerror_core::ErrorCore;

mod combine7;
mod iter7;
mod ordered;

use combine7::Combine7BitChunks;
use iter7::Iterate7BitChunks;

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Clone, Message, zerror_derive::Z)]
pub enum Error {
    #[prototk(311296, message)]
    Success {
        #[prototk(1, message)]
        core: ErrorCore,
    },
    #[prototk(311297, message)]
    CouldNotExtend {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, uint32)]
        field_number: u32,
    },
    #[prototk(311298, message)]
    UnpackError {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, message)]
        err: prototk::Error,
    },
    #[prototk(311299, message)]
    NotValidUtf8 {
        #[prototk(1, message)]
        core: ErrorCore,
    },
    #[prototk(311300, message)]
    InvalidTag {
        #[prototk(1, message)]
        core: ErrorCore,
    },
    #[prototk(311301, message)]
    SchemaIncompatibility {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        what: String,
    },
    #[prototk(311301, message)]
    Corruption {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        what: String,
    },
}

impl Error {
    pub fn schema_incompatibility(s: impl Into<String>) -> Self {
        Self::SchemaIncompatibility {
            core: ErrorCore::default(),
            what: s.into(),
        }
    }
}

impl Default for Error {
    fn default() -> Error {
        Error::Success {
            core: ErrorCore::default(),
        }
    }
}

impl From<buffertk::Error> for Error {
    fn from(err: buffertk::Error) -> Self {
        let err: prototk::Error = err.into();
        Self::from(err)
    }
}

impl From<prototk::Error> for Error {
    fn from(err: prototk::Error) -> Self {
        Self::UnpackError {
            core: ErrorCore::default(),
            err,
        }
    }
}

iotoz! {Error}

//////////////////////////////////////////// KeyDataType ///////////////////////////////////////////

#[derive(Copy, Clone, Debug, Default, Message, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[allow(non_camel_case_types)]
pub enum KeyDataType {
    #[default]
    #[prototk(1, message)]
    unit,
    #[prototk(2, message)]
    fixed32,
    #[prototk(3, message)]
    fixed64,
    #[prototk(4, message)]
    sfixed32,
    #[prototk(5, message)]
    sfixed64,
    #[prototk(7, message)]
    string,
}

impl KeyDataType {
    pub fn wire_type(self) -> WireType {
        match self {
            KeyDataType::unit => WireType::LengthDelimited,
            KeyDataType::fixed32 => WireType::ThirtyTwo,
            KeyDataType::fixed64 => WireType::SixtyFour,
            KeyDataType::sfixed32 => WireType::ThirtyTwo,
            KeyDataType::sfixed64 => WireType::SixtyFour,
            KeyDataType::string => WireType::LengthDelimited,
        }
    }
}

///////////////////////////////////////////// Direction ////////////////////////////////////////////

#[derive(Copy, Clone, Debug, Default, Message, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[allow(non_camel_case_types)]
pub enum Direction {
    #[default]
    #[prototk(1, message)]
    Forward,
    #[prototk(2, message)]
    Reverse,
}

/////////////////////////////////////////// discriminant ///////////////////////////////////////////

pub fn to_discriminant(key: KeyDataType, dir: Direction) -> u8 {
    match (key, dir) {
        (KeyDataType::unit, Direction::Forward) => 1,
        (KeyDataType::fixed32, Direction::Forward) => 2,
        (KeyDataType::fixed64, Direction::Forward) => 3,
        (KeyDataType::sfixed32, Direction::Forward) => 4,
        (KeyDataType::sfixed64, Direction::Forward) => 5,
        (KeyDataType::string, Direction::Forward) => 6,

        (KeyDataType::unit, Direction::Reverse) => 9,
        (KeyDataType::fixed32, Direction::Reverse) => 10,
        (KeyDataType::fixed64, Direction::Reverse) => 11,
        (KeyDataType::sfixed32, Direction::Reverse) => 12,
        (KeyDataType::sfixed64, Direction::Reverse) => 13,
        (KeyDataType::string, Direction::Reverse) => 14,
    }
}

pub fn from_discriminant(discriminant: u8) -> Option<(KeyDataType, Direction)> {
    match discriminant {
        1 => Some((KeyDataType::unit, Direction::Forward)),
        2 => Some((KeyDataType::fixed32, Direction::Forward)),
        3 => Some((KeyDataType::fixed64, Direction::Forward)),
        4 => Some((KeyDataType::sfixed32, Direction::Forward)),
        5 => Some((KeyDataType::sfixed64, Direction::Forward)),
        6 => Some((KeyDataType::string, Direction::Forward)),

        9 => Some((KeyDataType::unit, Direction::Reverse)),
        10 => Some((KeyDataType::fixed32, Direction::Reverse)),
        11 => Some((KeyDataType::fixed64, Direction::Reverse)),
        12 => Some((KeyDataType::sfixed32, Direction::Reverse)),
        13 => Some((KeyDataType::sfixed64, Direction::Reverse)),
        14 => Some((KeyDataType::string, Direction::Reverse)),
        _ => None,
    }
}

///////////////////////////////////////////// TupleKey /////////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct TupleKey {
    buf: Vec<u8>,
}

impl TupleKey {
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    pub fn append(&mut self, other: &mut TupleKey) {
        self.buf.append(&mut other.buf);
    }

    pub fn extend(&mut self, f: FieldNumber) {
        self.extend_field_number(f, KeyDataType::unit, Direction::Forward);
        ().append_to(self);
    }

    pub fn extend_with_key<E: Element>(&mut self, f: FieldNumber, elem: E, dir: Direction) {
        self.extend_field_number(f, E::DATA_TYPE, dir);
        let sz = self.buf.len();
        elem.append_to(self);
        if Direction::Reverse == dir {
            reverse_encoding(&mut self.buf[sz..]);
        }
    }

    pub fn iter(&self) -> TupleKeyIterator<'_> {
        let buf: &[u8] = &self.buf;
        TupleKeyIterator::from(buf)
    }

    pub fn conforms_to<T: Debug>(&self, schema: &Schema<T>) -> bool {
        schema.lookup(self).is_ok()
    }

    fn append_bytes(&mut self, iter: impl Iterator<Item = u8>) -> usize {
        let mut count = 0;
        for c in iter {
            self.buf.push(c);
            count += 1;
        }
        count
    }

    fn field_number(f: FieldNumber, value: KeyDataType, dir: Direction) -> ([u8; 10], usize) {
        let discriminant = to_discriminant(value, dir) as u64;
        assert!(discriminant < 16);
        let f: v64 = v64::from(((f.get() as u64) << 4) | discriminant);
        let mut buf = [0u8; 10];
        let sz = f.pack_sz();
        v64::pack(&f, &mut buf[0..sz]);
        // Shift the high order bit of the varint to the low order bit of the varint
        buf[0..sz].iter_mut().for_each(|c| *c = c.rotate_left(1));
        (buf, sz)
    }

    fn unfield_number(buf_in: &[u8]) -> Option<(FieldNumber, KeyDataType, Direction)> {
        if buf_in.len() > 10 {
            return None;
        }
        let mut buf = [0u8; 10];
        for (f, t) in std::iter::zip(buf_in.iter(), buf.iter_mut()) {
            *t = *f;
        }
        let sz = buf_in.len();
        buf[0..sz].iter_mut().for_each(|c| *c = c.rotate_right(1));
        let x = v64::unpack(&buf[0..sz]).ok()?.0;
        let x: u64 = x.into();
        let (key_type, direction) = from_discriminant(x as u8 & 15u8)?;
        if x >> 4 > u32::MAX as u64 {
            return None;
        }
        let field_number = FieldNumber::new((x >> 4) as u32).ok()?;
        Some((field_number, key_type, direction))
    }

    fn extend_field_number(&mut self, f: FieldNumber, value: KeyDataType, dir: Direction) {
        let (buf, sz) = Self::field_number(f, value, dir);
        self.buf.extend_from_slice(&buf[0..sz])
    }
}

impl std::ops::Deref for TupleKey {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.buf
    }
}

// NOTE(rescrv):  This is not a try_from.  Every byte string is a valid tuple key, because despite
// our conventions above, a tuple key is just a sequence of byte strings.  The typing is such a
// convenience that it's tied in.  If you need a tuple key to adhere to a given structure, you need
// to parse and validate it separately from this function.
impl From<&[u8]> for TupleKey {
    fn from(buf: &[u8]) -> Self {
        Self { buf: buf.to_vec() }
    }
}

///////////////////////////////////////// TupleKeyIterator /////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct TupleKeyIterator<'a> {
    buf: &'a [u8],
    offset: usize,
}

impl TupleKeyIterator<'_> {
    pub fn number_of_elements_in_common_prefix(lhs: Self, rhs: Self) -> usize {
        let mut max_idx = 0;
        for (idx, (x, y)) in std::iter::zip(lhs, rhs).enumerate() {
            if x != y {
                return idx / 2;
            }
            max_idx = idx;
        }
        (max_idx + 1) / 2
    }
}

impl<'a> From<&'a [u8]> for TupleKeyIterator<'a> {
    fn from(buf: &'a [u8]) -> Self {
        Self { buf, offset: 0 }
    }
}

impl<'a> From<&'a TupleKey> for TupleKeyIterator<'a> {
    fn from(tk: &'a TupleKey) -> Self {
        let buf: &'a [u8] = &tk.buf;
        Self::from(buf)
    }
}

impl<'a> Iterator for TupleKeyIterator<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.buf.len() {
            None
        } else {
            let start = self.offset;
            while self.offset < self.buf.len() && self.buf[self.offset] & 0x1 != 0 {
                self.offset += 1;
            }
            if self.offset < self.buf.len() {
                self.offset += 1;
            }
            let limit = self.offset;
            Some(&self.buf[start..limit])
        }
    }
}

////////////////////////////////////////// TupleKeyParser //////////////////////////////////////////

pub struct TupleKeyParser<'a> {
    iter: TupleKeyIterator<'a>,
}

impl<'a> TupleKeyParser<'a> {
    pub fn new(tk: &'a TupleKey) -> Self {
        Self {
            iter: TupleKeyIterator::from(tk),
        }
    }

    pub fn peek_next(&self) -> Result<Option<(FieldNumber, KeyDataType, Direction)>, &'static str> {
        let elem = match self.iter.clone().next() {
            Some(elem) => elem,
            None => {
                return Ok(None);
            }
        };
        Ok(Some(
            TupleKey::unfield_number(elem).ok_or("not a valid tag")?,
        ))
    }

    pub fn parse_next(&mut self, f: FieldNumber, dir: Direction) -> Result<(), &'static str> {
        self.parse_next_tag(f, KeyDataType::unit, dir)?;
        let pad = match self.iter.next() {
            Some(pad) => pad,
            None => {
                return Err("no more elements to TupleKey");
            }
        };
        if pad.len() != 1 {
            return Err("unit struct with length != 1");
        }
        Ok(())
    }

    pub fn parse_next_with_key<E: Element>(
        &mut self,
        f: FieldNumber,
        dir: Direction,
    ) -> Result<E, &'static str> {
        // First we parse_next as normal.
        self.parse_next_tag(f, E::DATA_TYPE, dir)?;
        // Read the value
        let value = match self.iter.next() {
            Some(value) => value,
            None => {
                return Err("missing value element");
            }
        };
        if Direction::Reverse == dir {
            let mut value = value.to_vec();
            reverse_encoding(&mut value);
            E::parse_from(&value)
        } else {
            E::parse_from(value)
        }
    }

    fn parse_next_tag(
        &mut self,
        f: FieldNumber,
        ty: KeyDataType,
        dir: Direction,
    ) -> Result<(), &'static str> {
        let elem = match self.iter.next() {
            Some(elem) => elem,
            None => {
                return Err("no more elements to TupleKey");
            }
        };
        let (buf, sz) = TupleKey::field_number(f, ty, dir);
        if &buf[0..sz] != elem {
            return Err("tag does not match");
        }
        Ok(())
    }
}

////////////////////////////////////////////// Element /////////////////////////////////////////////

pub trait Element: Sized {
    const DATA_TYPE: KeyDataType;

    fn append_to(&self, key: &mut TupleKey);
    fn parse_from(buf: &[u8]) -> Result<Self, &'static str>;

    fn key_data_type(&self) -> KeyDataType {
        <Self as Element>::DATA_TYPE
    }
}

impl Element for () {
    const DATA_TYPE: KeyDataType = KeyDataType::unit;

    fn append_to(&self, key: &mut TupleKey) {
        key.append_bytes(&mut [0u8].iter().copied());
    }

    fn parse_from(buf: &[u8]) -> Result<Self, &'static str> {
        if buf.len() != 1 {
            return Err("unit not exactly 1 bytes");
        }
        Ok(())
    }
}

impl Element for u32 {
    const DATA_TYPE: KeyDataType = KeyDataType::fixed32;

    fn append_to(&self, key: &mut TupleKey) {
        key.buf.push(((self >> 24) | 1) as u8);
        key.buf.push(((self >> 17) | 1) as u8);
        key.buf.push(((self >> 10) | 1) as u8);
        key.buf.push(((self >> 3) | 1) as u8);
        key.buf.push(((self & 0xf) << 4) as u8);
    }

    fn parse_from(buf: &[u8]) -> Result<Self, &'static str> {
        if buf.len() != 5 {
            return Err("buf not exactly 5 bytes");
        }
        let mut key = 0u32;
        key |= ((buf[0] & 0xfe) as u32) << 24;
        key |= ((buf[1] & 0xfe) as u32) << 17;
        key |= ((buf[2] & 0xfe) as u32) << 10;
        key |= ((buf[3] & 0xfe) as u32) << 3;
        key |= ((buf[4] & 0xf0) as u32) >> 4;
        Ok(key)
    }
}

impl Element for u64 {
    const DATA_TYPE: KeyDataType = KeyDataType::fixed64;

    fn append_to(&self, key: &mut TupleKey) {
        key.buf.push(((self >> 56) | 1) as u8);
        key.buf.push(((self >> 49) | 1) as u8);
        key.buf.push(((self >> 42) | 1) as u8);
        key.buf.push(((self >> 35) | 1) as u8);
        key.buf.push(((self >> 28) | 1) as u8);
        key.buf.push(((self >> 21) | 1) as u8);
        key.buf.push(((self >> 14) | 1) as u8);
        key.buf.push(((self >> 7) | 1) as u8);
        key.buf.push((self | 1) as u8);
        key.buf.push(((self & 0x1) << 7) as u8);
    }

    fn parse_from(buf: &[u8]) -> Result<Self, &'static str> {
        if buf.len() != 10 {
            return Err("buf not exactly 10 bytes");
        }
        let mut key = 0u64;
        key |= ((buf[0] & 0xfe) as u64) << 56;
        key |= ((buf[1] & 0xfe) as u64) << 49;
        key |= ((buf[2] & 0xfe) as u64) << 42;
        key |= ((buf[3] & 0xfe) as u64) << 35;
        key |= ((buf[4] & 0xfe) as u64) << 28;
        key |= ((buf[5] & 0xfe) as u64) << 21;
        key |= ((buf[6] & 0xfe) as u64) << 14;
        key |= ((buf[7] & 0xfe) as u64) << 7;
        key |= (buf[8] & 0xfe) as u64;
        key |= ((buf[9] & 0x80) as u64) >> 7;
        Ok(key)
    }
}

impl Element for i32 {
    const DATA_TYPE: KeyDataType = KeyDataType::sfixed32;

    fn append_to(&self, key: &mut TupleKey) {
        let num: u32 = ordered::encode_i32(*self);
        key.buf.push(((num >> 24) | 1) as u8);
        key.buf.push(((num >> 17) | 1) as u8);
        key.buf.push(((num >> 10) | 1) as u8);
        key.buf.push(((num >> 3) | 1) as u8);
        key.buf.push(((num & 0xf) << 4) as u8);
    }

    fn parse_from(buf: &[u8]) -> Result<Self, &'static str> {
        if buf.len() != 5 {
            return Err("buf not exactly 5 bytes");
        }
        let mut key = 0u32;
        key |= ((buf[0] & 0xfe) as u32) << 24;
        key |= ((buf[1] & 0xfe) as u32) << 17;
        key |= ((buf[2] & 0xfe) as u32) << 10;
        key |= ((buf[3] & 0xfe) as u32) << 3;
        key |= ((buf[4] & 0xf0) as u32) >> 4;
        Ok(ordered::decode_i32(key))
    }
}

impl Element for i64 {
    const DATA_TYPE: KeyDataType = KeyDataType::sfixed64;

    fn append_to(&self, key: &mut TupleKey) {
        let num: u64 = ordered::encode_i64(*self);
        key.buf.push(((num >> 56) | 1) as u8);
        key.buf.push(((num >> 49) | 1) as u8);
        key.buf.push(((num >> 42) | 1) as u8);
        key.buf.push(((num >> 35) | 1) as u8);
        key.buf.push(((num >> 28) | 1) as u8);
        key.buf.push(((num >> 21) | 1) as u8);
        key.buf.push(((num >> 14) | 1) as u8);
        key.buf.push(((num >> 7) | 1) as u8);
        key.buf.push((num | 1) as u8);
        key.buf.push(((num & 0x1) << 7) as u8);
    }

    fn parse_from(buf: &[u8]) -> Result<Self, &'static str> {
        if buf.len() != 10 {
            return Err("buf not exactly 10 bytes");
        }
        let mut key = 0u64;
        key |= ((buf[0] & 0xfe) as u64) << 56;
        key |= ((buf[1] & 0xfe) as u64) << 49;
        key |= ((buf[2] & 0xfe) as u64) << 42;
        key |= ((buf[3] & 0xfe) as u64) << 35;
        key |= ((buf[4] & 0xfe) as u64) << 28;
        key |= ((buf[5] & 0xfe) as u64) << 21;
        key |= ((buf[6] & 0xfe) as u64) << 14;
        key |= ((buf[7] & 0xfe) as u64) << 7;
        key |= (buf[8] & 0xfe) as u64;
        key |= ((buf[9] & 0x80) as u64) >> 7;
        Ok(ordered::decode_i64(key))
    }
}

impl Element for String {
    const DATA_TYPE: KeyDataType = KeyDataType::string;

    fn append_to(&self, key: &mut TupleKey) {
        let iter = Iterate7BitChunks::new(self.as_bytes());
        if key.append_bytes(iter) == 0 {
            key.append_bytes(&mut [0u8].iter().copied());
        }
    }

    fn parse_from(buf: &[u8]) -> Result<Self, &'static str> {
        if buf.len() == 1 {
            return Ok(String::new());
        }
        let combiner = Combine7BitChunks::new(buf);
        String::from_utf8(combiner.collect()).map_err(|_| "invalid UTF-8 sequence")
    }
}

/////////////////////////////////////////// TypedTupleKey //////////////////////////////////////////

pub trait TypedTupleKey: TryFrom<TupleKey> + Into<TupleKey> {}

////////////////////////////////////////////// Schema //////////////////////////////////////////////

#[derive(Debug)]
pub struct Schema<T: Debug> {
    node: T,
    children: HashMap<FieldNumber, Schema<T>>,
    names: HashMap<String, FieldNumber>,
}

impl<T: Debug> Schema<T> {
    pub fn new<I: Iterator<Item = ((FieldNumber, String), Schema<T>)>>(node: T, sub: I) -> Self {
        let mut children = HashMap::new();
        let mut names = HashMap::new();
        for ((field_number, name), schema) in sub {
            names.insert(name, field_number);
            children.insert(field_number, schema);
        }
        Self {
            node,
            children,
            names,
        }
    }

    pub fn field_number(&self, name: impl AsRef<str>) -> Option<FieldNumber> {
        self.names.get(name.as_ref()).cloned()
    }

    pub fn child(&self, f: FieldNumber) -> Option<&Schema<T>> {
        self.children.get(&f)
    }

    pub fn is_terminal(&self, tk: &TupleKey) -> Result<bool, Error> {
        Ok(self.schema_for_key(tk)?.children.is_empty())
    }

    pub fn lookup(&self, tk: &TupleKey) -> Result<&T, Error> {
        Ok(&self.schema_for_key(tk)?.node)
    }

    pub fn schema_for_key<'a>(&'a self, tk: &TupleKey) -> Result<&'a Schema<T>, Error> {
        let mut tkp = TupleKeyParser::new(tk);
        let mut args = vec![];
        self.schema_for_key_recurse(&mut tkp, 0, &mut args)
    }

    pub fn args_for_key(&self, tk: &TupleKey) -> Result<Vec<String>, Error> {
        let mut tkp = TupleKeyParser::new(tk);
        let mut args = vec![];
        self.schema_for_key_recurse(&mut tkp, 0, &mut args)?;
        Ok(args)
    }

    fn schema_for_key_recurse<'a>(
        &'a self,
        tkp: &mut TupleKeyParser,
        index: usize,
        args: &mut Vec<String>,
    ) -> Result<&'a Schema<T>, Error> {
        if let Some((f, k, d)) = tkp.peek_next().map_err(Error::schema_incompatibility)? {
            let Some(name) = self.names.iter().find(|(_, v)| **v == f) else {
                return Err(Error::schema_incompatibility(format!(
                    "unknown field {f} at index {index}"
                )));
            };
            args.push(format!("--{}", name.0));
            if let Some(recurse) = self.children.get(&f) {
                match k {
                    KeyDataType::unit => {
                        tkp.parse_next(f, d)
                            .map_err(Error::schema_incompatibility)?;
                    }
                    KeyDataType::fixed32 => {
                        let v: u32 = tkp
                            .parse_next_with_key(f, d)
                            .map_err(Error::schema_incompatibility)?;
                        args.push(v.to_string());
                    }
                    KeyDataType::sfixed32 => {
                        let v: i32 = tkp
                            .parse_next_with_key(f, d)
                            .map_err(Error::schema_incompatibility)?;
                        args.push(v.to_string());
                    }
                    KeyDataType::fixed64 => {
                        let v: u64 = tkp
                            .parse_next_with_key(f, d)
                            .map_err(Error::schema_incompatibility)?;
                        args.push(v.to_string());
                    }
                    KeyDataType::sfixed64 => {
                        let v: i64 = tkp
                            .parse_next_with_key(f, d)
                            .map_err(Error::schema_incompatibility)?;
                        args.push(v.to_string());
                    }
                    KeyDataType::string => {
                        let v: String = tkp
                            .parse_next_with_key(f, d)
                            .map_err(Error::schema_incompatibility)?;
                        args.push(v.to_string());
                    }
                };
                recurse.schema_for_key_recurse(tkp, index + 1, args)
            } else {
                Err(Error::schema_incompatibility(format!(
                    "unknown field {f} at index {index}"
                )))
            }
        } else {
            Ok(self)
        }
    }
}

///////////////////////////////////////// reverse_encoding /////////////////////////////////////////

fn reverse_encoding(bytes: &mut [u8]) {
    for b in bytes.iter_mut() {
        *b = (!*b & 0xfe) | (*b & 0x1);
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tuple_key {
    use super::*;

    mod append {
        use super::*;

        #[test]
        fn two_empties() {
            let mut tk1 = TupleKey::default();
            let mut tk2 = TupleKey::default();
            tk1.append(&mut tk2);
            assert!(tk1.as_bytes().is_empty());
            assert!(tk2.is_empty());
        }

        #[test]
        fn two_triplets() {
            let mut tk1 = TupleKey::default();
            tk1.extend_with_key(FieldNumber::must(1), "A".to_owned(), Direction::Forward);
            tk1.extend_with_key(FieldNumber::must(2), "B".to_owned(), Direction::Forward);
            tk1.extend_with_key(FieldNumber::must(3), "C".to_owned(), Direction::Forward);
            let mut tk2 = TupleKey::default();
            tk2.extend_with_key(FieldNumber::must(4), "D".to_owned(), Direction::Forward);
            tk2.extend_with_key(FieldNumber::must(5), "E".to_owned(), Direction::Forward);
            tk2.extend_with_key(FieldNumber::must(6), "F".to_owned(), Direction::Forward);
            // preconditions
            assert_eq!(&[44, 65, 128, 76, 67, 0, 108, 67, 128], tk1.as_bytes());
            assert_eq!(&[140, 69, 0, 172, 69, 128, 204, 71, 0], tk2.as_bytes());
            // what we want to test
            tk1.append(&mut tk2);
            assert_eq!(
                &[44, 65, 128, 76, 67, 0, 108, 67, 128, 140, 69, 0, 172, 69, 128, 204, 71, 0,],
                tk1.as_bytes()
            );
            assert!(tk2.is_empty());
        }
    }

    mod iterator {
        use super::*;

        #[test]
        fn empty() {
            let tk1 = TupleKey::default();
            let mut iter = TupleKeyIterator::from(&tk1);
            assert_eq!(None, iter.next());
        }

        #[test]
        fn abc() {
            let mut tk1 = TupleKey::default();
            tk1.extend_with_key(FieldNumber::must(1), "A".to_string(), Direction::Forward);
            tk1.extend_with_key(FieldNumber::must(2), "B".to_string(), Direction::Forward);
            tk1.extend_with_key(FieldNumber::must(3), "C".to_string(), Direction::Forward);
            let mut iter = TupleKeyIterator::from(&tk1);

            let buf: &[u8] = &[44];
            assert_eq!(Some(buf), iter.next());
            let buf: &[u8] = &[65, 128];
            assert_eq!(Some(buf), iter.next());

            let buf: &[u8] = &[76];
            assert_eq!(Some(buf), iter.next());
            let buf: &[u8] = &[67, 0];
            assert_eq!(Some(buf), iter.next());

            let buf: &[u8] = &[108];
            assert_eq!(Some(buf), iter.next());
            let buf: &[u8] = &[67, 128];
            assert_eq!(Some(buf), iter.next());

            assert_eq!(None, iter.next());
        }

        #[test]
        fn common_prefix() {
            let mut tk1 = TupleKey::default();
            tk1.extend_with_key(FieldNumber::must(1), "A".to_string(), Direction::Forward);
            tk1.extend_with_key(FieldNumber::must(2), "B".to_string(), Direction::Forward);
            tk1.extend_with_key(FieldNumber::must(3), "C".to_string(), Direction::Forward);
            let mut tk2 = TupleKey::default();
            // (A, B, C), (), 0
            assert_eq!(
                0,
                TupleKeyIterator::number_of_elements_in_common_prefix(tk1.iter(), tk2.iter())
            );
            // (A, B, C), (A), 1
            tk2.extend_with_key(FieldNumber::must(1), "A".to_string(), Direction::Forward);
            assert_eq!(
                1,
                TupleKeyIterator::number_of_elements_in_common_prefix(tk1.iter(), tk2.iter())
            );
            // (A, B, C), (A, B), 2
            tk2.extend_with_key(FieldNumber::must(2), "B".to_string(), Direction::Forward);
            assert_eq!(
                2,
                TupleKeyIterator::number_of_elements_in_common_prefix(tk1.iter(), tk2.iter())
            );
            // (A, B, C), (A, B, C), 3
            let mut tk3 = tk2.clone();
            tk2.extend_with_key(FieldNumber::must(3), "C".to_string(), Direction::Forward);
            assert_eq!(
                3,
                TupleKeyIterator::number_of_elements_in_common_prefix(tk1.iter(), tk2.iter())
            );
            // (A, B, C, D), (A, B, D), 2
            tk3.extend_with_key(FieldNumber::must(4), "D".to_string(), Direction::Forward);
            assert_eq!(
                2,
                TupleKeyIterator::number_of_elements_in_common_prefix(tk1.iter(), tk3.iter())
            );
        }
    }

    mod properties {
        use super::*;

        proptest::proptest! {
            #[test]
            fn field_number_round_trip(f in 1..prototk::FIRST_RESERVED_FIELD_NUMBER) {
                for d in [Direction::Forward, Direction::Reverse] {
                    for v in [
                        KeyDataType::unit,
                        KeyDataType::fixed32,
                        KeyDataType::fixed64,
                        KeyDataType::sfixed32,
                        KeyDataType::sfixed64,
                        KeyDataType::string,
                    ] {
                        let (buf, sz) = TupleKey::field_number(FieldNumber::must(f), v, d);
                        let (f1, v1, d1) = TupleKey::unfield_number(&buf[..sz]).unwrap();
                        assert_eq!(f1, f);
                        assert_eq!(v1, v);
                        assert_eq!(d1, d);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod elements {
    use super::*;

    fn test_helper<E: Element + Debug + Eq>(elem: E, exp: &[u8]) {
        let mut tk = TupleKey::default();
        elem.append_to(&mut tk);
        assert_eq!(exp, tk.as_bytes());
        let got = E::parse_from(exp).unwrap();
        assert_eq!(got, elem);
    }

    #[test]
    fn to_from_unit() {
        const VALUE: () = ();
        test_helper(VALUE, &[0]);
    }

    #[test]
    fn to_from_u32() {
        const VALUE: u32 = 0x1eaff00du32;
        test_helper(
            VALUE,
            &[0b00011111, 0b01010111, 0b11111101, 0b00000001, 0b11010000],
        );
    }

    #[test]
    fn to_from_u64() {
        const VALUE: u64 = 0x1eaff00d00c0ffeeu64;
        test_helper(
            VALUE,
            &[
                0b00011111, 0b01010111, 0b11111101, 0b00000001, 0b11010001, 0b00000111, 0b00000011,
                0b11111111, 0b11101111, 0b00000000,
            ],
        );
    }

    #[test]
    fn to_from_i32() {
        const VALUE: i32 = 0x1eaff00di32;
        test_helper(
            VALUE,
            &[0b10011111, 0b01010111, 0b11111101, 0b00000001, 0b11010000],
        );
    }

    #[test]
    fn to_from_i64() {
        const VALUE: i64 = 0x1eaff00d00c0ffeei64;
        test_helper(
            VALUE,
            &[
                0b10011111, 0b01010111, 0b11111101, 0b00000001, 0b11010001, 0b00000111, 0b00000011,
                0b11111111, 0b11101111, 0b00000000,
            ],
        );
    }

    #[test]
    fn to_from_string_empty() {
        let value: String = "".to_owned();
        test_helper(value, &[0]);
    }

    #[test]
    fn to_from_string() {
        let value: String = "hello world".to_owned();
        test_helper(
            value,
            &[105, 51, 91, 141, 199, 121, 129, 239, 111, 185, 155, 141, 64],
        );
    }
}
